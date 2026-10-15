#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use minipy_native::{
    codegen::CodeUnit,
    error::TaskError,
    http_provider::{Endpoint, HttpProvider, HttpService},
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{JsonLeaf, Reply, ResultSchema, ToolSchema},
};
use std::{
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
struct Fixture {
    base: String,
    connections: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<String>>>,
    seen: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    job: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.job.abort();
    }
}
async fn fixture() -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let connections = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (c, r, s, e) = (
        connections.clone(),
        requests.clone(),
        seen.clone(),
        release.clone(),
    );
    let job = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            c.fetch_add(1, Ordering::SeqCst);
            let (r, s, e) = (r.clone(), s.clone(), e.clone());
            tokio::spawn(async move {
                loop {
                    let mut header = Vec::new();
                    let mut byte = [0];
                    while !header.ends_with(b"\r\n\r\n") {
                        if stream.read_exact(&mut byte).await.is_err() {
                            return;
                        }
                        header.push(byte[0]);
                        if header.len() > 8192 {
                            return;
                        }
                    }
                    let header = String::from_utf8(header).unwrap();
                    let path = header.split_whitespace().nth(1).unwrap().to_owned();
                    r.lock().unwrap().push(header);
                    s.notify_one();
                    if path == "/blocked" {
                        e.notified().await;
                    }
                    let result=match path.as_str(){
                        "/json" | "/blocked" => stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 23\r\n\r\n{\"data\":{\"count\":10}}  ").await,
                        "/scalar" => stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n27").await,
                        "/string" => stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 19\r\n\r\n{\"message\":\"error\"}").await,
                        "/status" => stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n").await,
                        "/large" => stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\n").await,
                        "/chunks" | "/largechunks" => {
                            if stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n10\r\n{\"data\":{\"count\"\r\n").await.is_err(){return;}
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            if path=="/chunks" {stream.write_all(b"7\r\n:10}}  \r\n0\r\n\r\n").await}else{stream.write_all(b"20\r\nabcdefghijklmnopqrstuvwxyz123456\r\n0\r\n\r\n").await}
                        },
                        _=>return,
                    };
                    if result.is_err() {
                        return;
                    }
                }
            });
        }
    });
    Fixture {
        base,
        connections,
        requests,
        seen,
        release,
        job,
    }
}
fn endpoint(f: &Fixture, id: u32, path: &str, count: usize, json: bool) -> Endpoint {
    Endpoint {
        id,
        uri: format!("{}{path}", f.base).parse().unwrap(),
        arguments: count,
        json,
    }
}
#[tokio::test]
async fn real_http_pool_roundtrip_runs_both_backends_without_json_materialization() {
    let f = fixture().await;
    let service = HttpService::new(2, 1024, Duration::from_secs(2)).unwrap();
    let p = Arc::new(
        HttpProvider::new(
            service.clone(),
            vec![
                endpoint(&f, 101, "/json", 1, true),
                endpoint(&f, 102, "/scalar", 2, false),
            ],
        )
        .unwrap(),
    );
    let schemas = vec![
        ToolSchema {
            id: 101,
            name: "fetch".into(),
            arguments: vec!["batch_id".into()],
            result: ResultSchema::Json(vec![vec!["data".into(), "count".into()]]),
        },
        ToolSchema {
            id: 102,
            name: "publish".into(),
            arguments: vec!["batch_id".into(), "total".into()],
            result: ResultSchema::I64,
        },
    ];
    let source = "p = tools.fetch(batch_id)\nn = p[\"data\"][\"count\"] * 2\ntools.publish(total=n, batch_id=batch_id)\nprint(7)\n";
    for native in [false, true] {
        let r = if native {
            runtime::execute_script(
                Rc::new(
                    CodeUnit::compile_with_tools(source, &[("batch_id", Type::I64)], &schemas)
                        .unwrap(),
                ),
                p.clone(),
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await
        } else {
            interpreter::execute_script(
                Rc::new(
                    InterpreterUnit::prepare_with_tools(
                        source,
                        &[("batch_id", Type::I64)],
                        &schemas,
                    )
                    .unwrap(),
                ),
                p.clone(),
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await
        };
        assert_eq!(r.status, Ok(()));
        assert_eq!(r.stdout, "7\n");
    }
    assert_eq!(service.responses(), 4);
    assert_eq!(service.available_permits(), 2);
    assert_eq!(service.copied_bytes(), 0);
    assert_eq!(
        f.connections.load(Ordering::SeqCst),
        1,
        "all four operations reuse one pooled connection"
    );
    let requests = f.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert!(requests[1].contains("x-minipy-arg-0: 7"));
    assert!(requests[1].contains("x-minipy-arg-1: 20"));
}
#[tokio::test]
async fn real_http_json_string_contains_both_backends() {
    let f = fixture().await;
    let service = HttpService::new(1, 1024, Duration::from_secs(2)).unwrap();
    let provider = Arc::new(
        HttpProvider::new(service.clone(), vec![endpoint(&f, 101, "/string", 1, true)]).unwrap(),
    );
    let schema = [ToolSchema {
        id: 101,
        name: "fetch".into(),
        arguments: vec!["batch_id".into()],
        result: ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)]),
    }];
    let source = "data = tools.fetch(batch_id)\ntext = data[\"message\"]\nprint(\"error\" in text)";
    for native in [false, true] {
        let output = if native {
            runtime::execute_script(
                Rc::new(
                    CodeUnit::compile_with_tools(source, &[("batch_id", Type::I64)], &schema)
                        .unwrap(),
                ),
                provider.clone(),
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await
        } else {
            interpreter::execute_script(
                Rc::new(
                    InterpreterUnit::prepare_with_tools(
                        source,
                        &[("batch_id", Type::I64)],
                        &schema,
                    )
                    .unwrap(),
                ),
                provider.clone(),
                &[7],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await
        };
        assert_eq!(output.status, Ok(()));
        assert_eq!(output.stdout, "True\n");
    }
    assert_eq!(service.responses(), 2);
    assert_eq!(f.requests.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn multi_chunk_merge_is_one_copy_and_body_status_limits_are_errors() {
    let f = fixture().await;
    let service = HttpService::new(1, 32, Duration::from_secs(2)).unwrap();
    let p = HttpProvider::new(
        service.clone(),
        vec![
            endpoint(&f, 2, "/chunks", 0, true),
            endpoint(&f, 3, "/large", 0, true),
            endpoint(&f, 4, "/largechunks", 0, true),
            endpoint(&f, 5, "/status", 0, true),
        ],
    )
    .unwrap();
    let Reply::Json(bytes) = p.call_tool(2, [0, 0], 0).await.unwrap() else {
        panic!()
    };
    assert_eq!(&bytes[..], b"{\"data\":{\"count\":10}}  ");
    assert_eq!(service.copied_bytes(), bytes.len());
    assert!(matches!(
        p.call_tool(3, [0, 0], 0).await,
        Err(TaskError::Budget)
    ));
    assert!(matches!(
        p.call_tool(4, [0, 0], 0).await,
        Err(TaskError::Budget)
    ));
    assert!(matches!(
        p.call_tool(5, [0, 0], 0).await,
        Err(TaskError::Provider(_))
    ));
    assert_eq!(service.available_permits(), 1);
    assert!(!p.allows_tool(99));
    assert!(matches!(
        p.call_tool(99, [0, 0], 0).await,
        Err(TaskError::Permission)
    ));
}
#[tokio::test]
async fn cancelled_receiver_keeps_http_admission_until_real_completion() {
    let f = fixture().await;
    let service = HttpService::new(1, 1024, Duration::from_secs(2)).unwrap();
    let p = Arc::new(
        HttpProvider::new(service.clone(), vec![endpoint(&f, 2, "/blocked", 0, true)]).unwrap(),
    );
    let schema = [ToolSchema {
        id: 2,
        name: "fetch".into(),
        arguments: vec![],
        result: ResultSchema::Json(vec![vec!["data".into(), "count".into()]]),
    }];
    let cancel = Cancellation::default();
    let task = interpreter::execute_script(
        Rc::new(
            InterpreterUnit::prepare_with_tools(
                "p = tools.fetch()\nprint(p[\"data\"][\"count\"])\n",
                &[],
                &schema,
            )
            .unwrap(),
        ),
        p.clone(),
        &[],
        ScriptLimits::default(),
        cancel.clone(),
        None,
    );
    tokio::pin!(task);
    tokio::select! {r=&mut task=>panic!("early {:?}",r.status),_=f.seen.notified()=>{}}
    assert_eq!(service.available_permits(), 0);
    cancel.cancel();
    assert_eq!(task.await.status, Err(TaskError::Cancelled));
    assert_eq!(service.available_permits(), 0);
    assert!(matches!(
        p.call_tool(2, [0, 0], 0).await,
        Err(TaskError::Budget)
    ));
    f.release.notify_one();
    tokio::time::timeout(Duration::from_secs(1), async {
        while service.available_permits() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(service.responses(), 1);
    assert_eq!(f.requests.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn http_timeout_returns_permit_without_reviving_task() {
    let f = fixture().await;
    let service = HttpService::new(1, 1024, Duration::from_millis(30)).unwrap();
    let p = HttpProvider::new(service.clone(), vec![endpoint(&f, 2, "/blocked", 0, true)]).unwrap();
    assert!(matches!(
        p.call_tool(2, [0, 0], 0).await,
        Err(TaskError::Provider(_))
    ));
    assert_eq!(service.available_permits(), 1);
    f.release.notify_one();
}
