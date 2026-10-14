//! AF benchmark-only stream adapter; workload/runtime paths remain frozen.
use super::*;
use std::io::BufRead;

pub async fn serve(engine: &str) {
    let sources: Vec<_> = ["scalar", "weighted", "stateful"].map(source).into();
    let shapes = [
        ("scalar", 0),
        ("weighted", 100),
        ("stateful", 100),
        ("weighted", 10000),
        ("stateful", 100000),
    ];
    let expected_outputs: Vec<_> = shapes.iter().map(|(c, n)| expected(c, *n)).collect();
    let delay: u64 = std::env::var("MINIPY_TOOL_DELAY_US")
        .unwrap()
        .parse()
        .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<serde_json::Value>(32);
    let reader = std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let req: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
            if req["id"].as_i64().unwrap() < 0 {
                break;
            }
            if tx.blocking_send(req).is_err() {
                break;
            }
        }
    });
    let cpu = common::cpu_ns();
    common::emit(
        &serde_json::json!({"kind":"ready","pid":std::process::id(),"mono_ns":monotonic_ns(),"cpu_ns":cpu}),
    );
    let mut jobs = tokio::task::JoinSet::new();
    let mut loads = 0;
    let mut records = 0;
    while let Some(req) = rx.recv().await {
        let case = req["case"].as_str().unwrap();
        let n = req["rows"].as_u64().unwrap() as usize;
        let shape = shapes
            .iter()
            .position(|(c, r)| *c == case && *r == n)
            .unwrap();
        let index = match case {
            "scalar" => 0,
            "weighted" => 1,
            "stateful" => 2,
            _ => unreachable!(),
        };
        let source = sources[index].clone();
        let expected = expected_outputs[shape].clone();
        let engine = engine.to_owned();
        jobs.spawn_local(async move {
            let start = monotonic_ns();
            let mut provider = RequestProvider::ready(n,Duration::from_micros(delay));
            provider.observe = true;
            let provider = Arc::new(provider);
            let stats = provider.stats.clone();
            let code = prepare(&engine,&source,if index==0 {"scalar"} else if index==1 {"weighted"} else {"stateful"});
            let result = code.run(provider).await;
            let end = monotonic_ns();
            common::emit(&serde_json::json!({"kind":"result","id":req["id"],"start_ns":start,"end_ns":end,"stdout":result,"ok":result==expected}));
            (stats.loads.load(std::sync::atomic::Ordering::Relaxed),stats.records.load(std::sync::atomic::Ordering::Relaxed))
        });
        // Reap completed handles so a sustained run does not retain all results.
        while let Some(res) = jobs.try_join_next() {
            let (l, r) = res.unwrap();
            loads += l;
            records += r;
        }
    }
    while let Some(res) = jobs.join_next().await {
        let (l, r) = res.unwrap();
        loads += l;
        records += r;
    }
    reader.join().unwrap();
    common::emit(
        &serde_json::json!({"kind":"final","mono_ns":monotonic_ns(),"cpu_ns":common::cpu_ns()-cpu,"loads":loads,"records":records}),
    );
}
