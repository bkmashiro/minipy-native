#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use minipy_native::{
    http_provider::{Endpoint, HttpProvider, HttpService},
    provider::Provider,
    tool::Reply,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
    time::Instant,
};

struct CountingAllocator;
static TRACK: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REQUESTED_BYTES: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            REQUESTED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            REQUESTED_BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

fn old_single(bytes: bytes::Bytes) -> bytes::Bytes {
    let mut chunks = Vec::new();
    chunks.push(bytes);
    chunks.pop().unwrap()
}

fn lazy_single(bytes: bytes::Bytes) -> bytes::Bytes {
    bytes
}

fn allocation_cost(f: impl FnOnce()) -> (usize, usize) {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    REQUESTED_BYTES.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::SeqCst);
    f();
    TRACK.store(false, Ordering::SeqCst);
    (
        ALLOCATIONS.load(Ordering::Relaxed),
        REQUESTED_BYTES.load(Ordering::Relaxed),
    )
}

use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpListener};

#[tokio::test]
#[ignore = "bounded local measurement; run serially with the perf lock"]
async fn measure_loopback_single_chunk() {
    const SAMPLES: usize = 500;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let body = "x".repeat(256);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    let response = Arc::<[u8]>::from(response.into_bytes());
    let server = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let response = response.clone();
            tokio::spawn(async move {
                loop {
                    let mut request = Vec::new();
                    let mut byte = [0u8; 1];
                    while !request.ends_with(b"\r\n\r\n") {
                        if stream.read_exact(&mut byte).await.is_err() {
                            return;
                        }
                        request.push(byte[0]);
                    }
                    if stream.write_all(&response).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    let service = HttpService::new(1, 1024, Duration::from_secs(2)).unwrap();
    let provider = HttpProvider::new(
        service.clone(),
        vec![Endpoint {
            id: 2,
            uri: format!("http://{addr}/").parse().unwrap(),
            arguments: 0,
            json: true,
        }],
    )
    .unwrap();
    let mut ns = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        match provider.call_tool(2, [0, 0], 0).await.unwrap() {
            Reply::Json(bytes) => assert_eq!(bytes.len(), 256),
            _ => panic!("unexpected reply"),
        }
        ns.push(start.elapsed().as_nanos() as u64);
    }
    let raw = ns.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
    ns.sort_unstable();
    println!(
        "loopback_single_chunk variant={} samples={SAMPLES} median_ns={} p10_ns={} p90_ns={} copied_bytes={} responses={} raw_ns={raw}",
        std::env::var("HTTP_VARIANT").unwrap_or_else(|_| "unspecified".into()),
        ns[SAMPLES / 2],
        ns[SAMPLES / 10],
        ns[SAMPLES * 9 / 10],
        service.copied_bytes(),
        service.responses(),
    );
    assert_eq!(service.responses(), SAMPLES);
    assert_eq!(service.copied_bytes(), 0);
    assert_eq!(service.available_permits(), 1);
    server.abort();
}

#[test]
fn singleton_collection_allocation_shape() {
    let baseline = allocation_cost(|| {
        let bytes = bytes::Bytes::from_static(b"single");
        assert_eq!(old_single(bytes).as_ref(), b"single");
    });
    let candidate = allocation_cost(|| {
        let bytes = bytes::Bytes::from_static(b"single");
        assert_eq!(lazy_single(bytes).as_ref(), b"single");
    });
    println!(
        "singleton_collector baseline_allocs={} baseline_requested_bytes={} candidate_allocs={} candidate_requested_bytes={}",
        baseline.0, baseline.1, candidate.0, candidate.1
    );
    assert_eq!(baseline.0, 1);
    assert_eq!(candidate, (0, 0));
}
