//! Isolate driver wakeup vs deferred yield. Timing diagnostic, no MiniPy work.
fn main() {
    let mut b = tokio::runtime::Builder::new_current_thread();
    #[cfg(feature = "tool-json")]
    if std::env::args().any(|a| a == "--io") {
        b.enable_all();
    }
    b.enable_time();
    let rt = b.build().unwrap();
    println!("detached,yields,rep,iterations,total_ns");
    rt.block_on(async {
        for (detached, yields) in [(0, 0), (0, 3), (2, 0), (2, 3)] {
            for rep in 0..3 {
                let start = std::time::Instant::now();
                for _ in 0..1000 {
                    if detached > 0 {
                        handoff().await;
                    }
                    for _ in 0..yields {
                        tokio::task::yield_now().await;
                    }
                    if detached > 0 {
                        handoff().await;
                    }
                }
                println!(
                    "{detached},{yields},{rep},1000,{}",
                    start.elapsed().as_nanos()
                );
            }
        }
    });
}
async fn handoff() {
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move { tx.send(()).unwrap() });
    rx.await.unwrap();
}
