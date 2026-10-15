//! Explicit resident new-source probe, driven by tools/perf_workbench.py.
#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    error::TaskError,
    http_provider::{Endpoint, HttpProvider, HttpService},
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{
        ArgumentSchema, JsonLeaf, Reply, ResultSchema, ToolArguments, ToolDeclaration,
        ToolEnvironment,
    },
};
use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct Host;
impl Provider for Host {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        // Producer construction/import is request work, not prebuilt resident input.
        QuoteBuffer::from_quotes(
            (0..256)
                .map(|i| Quote {
                    unit_price_cents: i,
                    quantity: 2,
                })
                .collect(),
        )
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
    fn allows_tool(&self, id: u32) -> bool {
        id == 101
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        assert_eq!(id, 101);
        if matches!(args, ToolArguments::Numeric(values, _) if values[0] == -1) {
            return Ok(Reply::Json(bytes::Bytes::from_static(
                br#"{"message":"hello\nerror"}"#,
            )));
        }
        Ok(Reply::Json(bytes::Bytes::from_static(
            br#"{"message":"hello error"}"#,
        )))
    }
}
fn declarations() -> Vec<ToolDeclaration> {
    (0..16)
        .map(|i| ToolDeclaration {
            id: 101 + i,
            name: if i == 0 {
                "fetch".into()
            } else {
                format!("unused{i}")
            },
            arguments: vec![ArgumentSchema::I64("key".into())],
            result: ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)]),
        })
        .collect()
}
async fn fixture() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let job = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                loop {
                    let mut header = Vec::new();
                    let mut byte = [0];
                    while !header.ends_with(b"\r\n\r\n") {
                        if socket.read_exact(&mut byte).await.is_err() {
                            return;
                        }
                        header.push(byte[0]);
                        assert!(header.len() < 8192);
                    }
                    if socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 25\r\n\r\n{\"message\":\"hello error\"}").await.is_err() { return; }
                }
            });
        }
    });
    (base, job)
}

#[test]
#[ignore = "explicit serial release experiment under perf lock"]
fn resident_request() {
    let backend = std::env::var("PW_BACKEND").unwrap();
    let case = std::env::var("PW_CASE").unwrap();
    let entry = std::env::var("PW_ENTRY").unwrap();
    assert!(["native", "tape"].contains(&backend.as_str()));
    assert!(["legacy", "registered"].contains(&entry.as_str()));
    let reps: usize = std::env::var("PW_REPS").unwrap().parse().unwrap();
    assert!((1..=1000).contains(&reps));
    let setup = Instant::now();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let tools = declarations();
    let environment = ToolEnvironment::new(tools.clone()).unwrap();
    let registration_ns = setup.elapsed().as_nanos();
    let (uri, server) = rt.block_on(fixture());
    let service = HttpService::new(1, 4096, Duration::from_secs(2)).unwrap();
    let http = Arc::new(
        HttpProvider::new(
            service.clone(),
            vec![Endpoint {
                id: 101,
                uri: uri.parse().unwrap(),
                arguments: 1,
                json: true,
            }],
        )
        .unwrap(),
    );
    let ready = Arc::new(Host);
    let schema = [("input_0", Type::I64), ("input_1", Type::I64)];
    let inputs = if case.ends_with("_inputs") {
        &schema[..]
    } else {
        &[]
    };
    let values = if inputs.is_empty() {
        &[][..]
    } else {
        &[7, 9][..]
    };
    let setup_ns = setup.elapsed().as_nanos();
    let diagnostic = cfg!(feature = "compiler-profile");
    println!(
        "PW {}",
        serde_json::json!({"kind":"setup", "ns":setup_ns,
        "registration_and_runtime_ns":registration_ns, "diagnostic":diagnostic})
    );
    let mut rows = Vec::with_capacity(reps + 1);
    for sample in 0..=reps {
        // Source construction is caller work, outside the stated source-to-release boundary.
        // Different literal each time, no prepared-code or result reuse.
        let n = sample + 1;
        let (source, expected) = match case.as_str() {
            "scalar" => (format!("print({n} + 1)"), format!("{}\n", n + 1)),
            "scalar_inputs" => (
                format!("print(input_0 + input_1 + {n})"),
                format!("{}\n", n + 16),
            ),
            "quote" => (
                format!(
                    "rows = tools.load_quotes({n})\ntotal = 0\nfor row in rows:\n    total = total + row.unit_price_cents * row.quantity\nprint(total)"
                ),
                "65280\n".into(),
            ),
            "plain_sink" | "escaped_sink" | "joined_sink" | "fstring_sink" | "escaped_join" => {
                let print = match case.as_str() {
                    "plain_sink" | "escaped_sink" => "print(text)",
                    "joined_sink" | "escaped_join" => "print('[' + text + '/' + text + ']')",
                    _ => "print(f'[{text}/{n}/{True}]')",
                };
                let escaped = matches!(case.as_str(), "escaped_sink" | "escaped_join");
                let text = if escaped {
                    "hello\nerror"
                } else {
                    "hello error"
                };
                let expected = match case.as_str() {
                    "joined_sink" | "escaped_join" => format!("[{text}/{text}]\n"),
                    "fstring_sink" => format!("[{text}/{n}/True]\n"),
                    _ => format!("{text}\n"),
                };
                (
                    format!(
                        "n={n}\ndata=tools.fetch({})\ntext=data['message']\n{print}",
                        if escaped { -1 } else { n as i64 }
                    ),
                    expected,
                )
            }
            "ready" | "loopback" => (
                format!(
                    "data = tools.fetch({n})\nalias = data\ntext = alias['message']\nprint('error' in text)\nprint(text)"
                ),
                "True\nhello error\n".into(),
            ),
            "ready_inputs" => (
                format!(
                    "data = tools.fetch(input_0 + input_1 + {n})\nalias = data\ntext = alias['message']\nprint('error' in text)\nprint(text)"
                ),
                "True\nhello error\n".into(),
            ),
            _ => panic!("unknown case"),
        };
        #[cfg(feature = "compiler-profile")]
        minipy_native::compiler_profile::reset(false);
        let start = Instant::now();
        // Only diagnostic builds have phase timers. Default records one E2E interval.
        #[cfg(feature = "compiler-profile")]
        let prepare_ns;
        let outcome = if backend == "native" {
            let code = Rc::new(
                if entry == "registered" {
                    CodeUnit::compile_with_environment(&source, inputs, &environment)
                } else {
                    CodeUnit::compile_with_declarations(&source, inputs, &tools)
                }
                .unwrap(),
            );
            #[cfg(feature = "compiler-profile")]
            {
                prepare_ns = start.elapsed().as_nanos();
            }
            if case == "loopback" {
                rt.block_on(runtime::execute_script(
                    code,
                    http.clone(),
                    values,
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                ))
            } else {
                rt.block_on(runtime::execute_script(
                    code,
                    ready.clone(),
                    values,
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                ))
            }
        } else {
            let code = Rc::new(
                if entry == "registered" {
                    InterpreterUnit::prepare_with_environment(&source, inputs, &environment)
                } else {
                    InterpreterUnit::prepare_with_declarations(&source, inputs, &tools)
                }
                .unwrap(),
            );
            #[cfg(feature = "compiler-profile")]
            {
                prepare_ns = start.elapsed().as_nanos();
            }
            if case == "loopback" {
                rt.block_on(interpreter::execute_script(
                    code,
                    http.clone(),
                    values,
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                ))
            } else {
                rt.block_on(interpreter::execute_script(
                    code,
                    ready.clone(),
                    values,
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                ))
            }
        };
        #[cfg(feature = "compiler-profile")]
        let execute_ns = start.elapsed().as_nanos() - prepare_ns;
        // Keep tiny output/status checks inside E2E so output can be released before stop.
        assert_eq!(outcome.status, Ok(()));
        assert_eq!(outcome.stdout, expected);
        #[cfg(feature = "compiler-profile")]
        let cleanup = Instant::now();
        drop(outcome);
        #[cfg(feature = "compiler-profile")]
        let cleanup_ns = cleanup.elapsed().as_nanos();
        let ns = start.elapsed().as_nanos();
        let row = serde_json::json!({"kind":"request", "sample":sample, "ns":ns});
        #[cfg(feature = "compiler-profile")]
        let row = {
            let mut row = row;
            row["prepare_ns"] = serde_json::json!(prepare_ns);
            row["execute_including_code_release_ns"] = serde_json::json!(execute_ns);
            row["output_release_ns"] = serde_json::json!(cleanup_ns);
            let p = minipy_native::compiler_profile::take();
            row["compiler"] = serde_json::json!({"parse_ns":p.parse_ns, "type_ns":p.type_ns,
                "frontend_ns":p.frontend_ns, "setup_ns":p.setup_ns, "lowering_ns":p.lowering_ns,
                "backend_ns":p.backend_ns, "finalize_ns":p.finalize_ns, "cleanup_ns":p.cleanup_ns});
            row
        };
        rows.push(row);
    }
    if case == "loopback" {
        assert_eq!(service.responses(), reps + 1);
        assert_eq!(service.available_permits(), 1);
    }
    for row in rows {
        println!("PW {row}");
    }
    server.abort();
}
