//! Localhost end-to-end qualification, not a production network benchmark.
#[cfg(not(feature = "tool-json"))]
fn main() {
    eprintln!("requires --features tool-json");
}
#[cfg(feature = "tool-json")]
fn main() {
    probe::run();
}
#[cfg(feature = "tool-json")]
mod probe {
    use minipy_native::{
        codegen::CodeUnit,
        http_provider::{Endpoint, HttpProvider, HttpService},
        interpreter::{self, InterpreterUnit},
        ir::Type,
        runtime::{self, Cancellation, ScriptLimits},
        tool::{ResultSchema, ToolSchema},
    };
    use std::{
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    pub fn run() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let connections = Arc::new(AtomicUsize::new(0));
            let count = connections.clone();
            // Same-runtime synthetic local peer; construction is outside request timing.
            let server = tokio::spawn(async move {
                while let Ok((mut stream, _)) = listener.accept().await {
                    count.fetch_add(1, Ordering::Relaxed);
                    stream.set_nodelay(true).unwrap();
                    tokio::spawn(async move {
                        loop {
                            let mut header = Vec::new();
                            while !header.ends_with(b"\r\n\r\n") {
                                let mut b = [0];
                                if stream.read_exact(&mut b).await.is_err() { return; }
                                header.push(b[0]);
                                assert!(header.len() < 8192);
                            }
                            let text = std::str::from_utf8(&header).unwrap();
                            let body = if text.starts_with("GET /json ") {
                                br#"{"data":{"count":10}}"#.as_slice()
                            } else {
                                assert!(text.starts_with("GET /scalar "));
                                assert!(text.contains("x-minipy-arg-0: 7"));
                                assert!(text.contains("x-minipy-arg-1: 20"));
                                b"27".as_slice()
                            };
                            let mut reply = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
                            reply.extend_from_slice(body);
                            if stream.write_all(&reply).await.is_err() { return; }
                        }
                    });
                }
            });
            let service = HttpService::new(2, 1024, Duration::from_secs(2)).unwrap();
            let p = Arc::new(HttpProvider::new(service.clone(), vec![
                Endpoint { id:101, uri:format!("{base}/json").parse().unwrap(), arguments:1, json:true },
                Endpoint { id:102, uri:format!("{base}/scalar").parse().unwrap(), arguments:2, json:false },
            ]).unwrap());
            let schemas = vec![
                ToolSchema { id:101, name:"fetch".into(), arguments:vec!["batch_id".into()],
                    result:ResultSchema::Json(vec![vec!["data".into(),"count".into()]]) },
                ToolSchema { id:102, name:"publish".into(), arguments:vec!["batch_id".into(),"total".into()], result:ResultSchema::I64 },
            ];
            let inputs = &[("batch_id", Type::I64)];
            println!("syntax,engine,lifecycle,rep,iterations,total_ns,prepared_bytes,code_bytes,responses,connections,merge_copied_bytes,checksum");
            for direct in [false, true] {
                let source = if direct {
                    "p = tools.fetch(batch_id)\nprint(tools.publish(total=p[\"data\"][\"count\"] * 2, batch_id=batch_id))\n"
                } else {
                    "p = tools.fetch(batch_id)\nr = tools.publish(total=p[\"data\"][\"count\"] * 2, batch_id=batch_id)\nprint(r)\n"
                };
                for native in [false, true] {
                    let nu = native.then(|| Rc::new(CodeUnit::compile_with_tools(source, inputs, &schemas).unwrap()));
                    let tu = (!native).then(|| Rc::new(InterpreterUnit::prepare_with_tools(source, inputs, &schemas).unwrap()));
                    for lifecycle in ["source_cold", "prepared"] {
                        for rep in 0..3 {
                            let before = service.responses();
                            // Warm one request, including pool establishment, outside the batch.
                            let mut sum = 0;
                            let mut start = Instant::now();
                            let n = if lifecycle == "source_cold" { 20 } else { 100 };
                            for i in 0..=n {
                                if i == 1 { start = Instant::now(); }
                                let r = if native {
                                    let unit = if lifecycle == "source_cold" { Rc::new(CodeUnit::compile_with_tools(source, inputs, &schemas).unwrap()) } else { nu.as_ref().unwrap().clone() };
                                    runtime::execute_script(unit, p.clone(), &[7], ScriptLimits::default(), Cancellation::default(), None).await
                                } else {
                                    let unit = if lifecycle == "source_cold" { Rc::new(InterpreterUnit::prepare_with_tools(source, inputs, &schemas).unwrap()) } else { tu.as_ref().unwrap().clone() };
                                    interpreter::execute_script(unit, p.clone(), &[7], ScriptLimits::default(), Cancellation::default(), None).await
                                };
                                assert_eq!(r.status, Ok(()));
                                assert_eq!(r.stdout, "27\n");
                                if i > 0 { sum += r.stdout.bytes().map(u64::from).sum::<u64>(); }
                            }
                            let elapsed = start.elapsed().as_nanos();
                            assert_eq!(service.responses() - before, 2*(n+1));
                            assert_eq!(service.available_permits(), 2);
                            println!("{},{},{lifecycle},{rep},{n},{elapsed},{},{},{},{},{},{sum}",
                                if direct {"direct"} else {"assigned"}, if native {"native"} else {"tape"},
                                tu.as_ref().map_or(0, |u| u.prepared_bytes()), nu.as_ref().map_or(0, |u| u.code_bytes),
                                service.responses()-before, connections.load(Ordering::Relaxed), service.copied_bytes());
                        }
                    }
                }
            }
            assert_eq!(connections.load(Ordering::Relaxed), 1);
            server.abort();
        });
    }
}
