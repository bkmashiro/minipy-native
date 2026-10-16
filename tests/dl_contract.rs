#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
#[path = "../examples/di_corpus.rs"]
mod runner;
use serde_json::{Value, json};

#[tokio::test]
async fn uniform_stdout_policy_counts_utf8_and_keeps_failed_print_atomic() {
    let small: runner::Case = serde_json::from_value(json!({
        "case_id":"utf8", "source":"print(7)\nprint('é界')\nx = tools.record_total(1, 2)",
        "tools":[{"id":2,"name":"unused","arguments":[],"result":{"kind":"i64"}}],
        "replies":[{"tool_id":1,"arguments":[1,2],"i64":9}]
    }))
    .unwrap();
    // 7\n = 2 bytes, é界\n = 6 bytes, not three characters.
    for native in [false, true] {
        for diagnostic in [false, true] {
            for limit in [0, 1, 2, 7, 8, 9] {
                let r = runner::run_with_stdout_bytes(&small, native, diagnostic, limit).await;
                assert_eq!(r["policy"]["stdout_bytes"], limit);
                assert_eq!(r["route"], "executed");
                assert_eq!(
                    r["actual"]["status"],
                    if limit >= 8 { "ok" } else { "Budget" }
                );
                assert_eq!(
                    r["actual"]["stdout"],
                    if limit >= 8 {
                        "7\né界\n"
                    } else if limit >= 2 {
                        "7\n"
                    } else {
                        ""
                    }
                );
                assert_eq!(
                    r["actual"]["calls"].as_array().unwrap().len(),
                    usize::from(limit >= 8)
                );
            }
        }
    }
    assert!(runner::validate_stdout_bytes(runner::MAX_STDOUT_BYTES).is_ok());
    assert!(runner::validate_stdout_bytes(runner::MAX_STDOUT_BYTES + 1).is_err());
}

#[tokio::test]
async fn document_default_is_bounded_without_changing_library_default() {
    assert_eq!(
        minipy_native::runtime::ScriptLimits::default().stdout_bytes,
        16 * 1024
    );
    assert_eq!(runner::DEFAULT_STDOUT_BYTES, 256 * 1024);
    // Dynamic replies keep source/literal limits separate from cumulative stdout.
    let text = "é".repeat(16_383) + "x"; // 32767 bytes, plus newline = 32 KiB.
    for prints in [8, 9] {
        let source = format!("d = tools.fetch()\n{}", "print(d.content)\n".repeat(prints));
        let case: runner::Case = serde_json::from_value(json!({"case_id":"document", "source":source,
            "tools":[{"id":2,"name":"fetch","arguments":[],"result":{"kind":"json","leaves":[{"path":["content"],"type":"string"}]}}],
            "replies":[{"tool_id":2,"arguments":[],"json":json!({"content":text}).to_string()}]
        })).unwrap();
        for native in [false, true] {
            for diagnostic in [false, true] {
                let r = runner::run(&case, native, diagnostic).await;
                assert_eq!(
                    r["actual"]["status"],
                    if prints == 8 { "ok" } else { "Budget" }
                );
                assert_eq!(
                    r["actual"]["stdout"].as_str().unwrap(),
                    (text.clone() + "\n").repeat(8)
                );
                assert_eq!(r["policy"]["stdout_bytes"], 262144);
                let old = runner::run_with_stdout_bytes(&case, native, diagnostic, 16384).await;
                assert_eq!(old["actual"]["status"], "Budget");
                assert_eq!(old["actual"]["stdout"], "");
            }
        }
    }
}

async fn row(value: Value, native: bool) -> Value {
    let case: runner::Case = serde_json::from_value(value).unwrap();
    case.validate_replay().unwrap();
    runner::run(&case, native, false).await
}

#[tokio::test]
async fn scalar_reassignment_is_supported_but_json_binding_is_static() {
    for native in [false, true] {
        let r = row(json!({"case_id":"scalar", "source":"n = 1\nn = 2\nb = True\nb = False\nprint(n)\nprint(b)"}), native).await;
        assert_eq!(r["actual"]["stdout"], "2\nFalse\n");
        assert_eq!(r["actual"]["status"], "ok");
        for suffix in [
            "d = tools.fetch()",
            "s = d.content\ns = d.content",
            "v = d\nv = d",
        ] {
            let r = row(json!({"case_id":"static", "source":format!("d = tools.fetch()\n{suffix}"),
                "tools":[{"id":2,"name":"fetch","arguments":[],"result":{"kind":"json","leaves":[{"path":["content"],"type":"string"}]}}]
            }), native).await;
            assert_eq!(r["actual"]["status"], "Unsupported");
            assert_eq!(
                r["error"]["reason"],
                "JSON view name conflicts with an existing/reserved binding"
            );
            assert_eq!(r["actual"]["calls"], json!([]));
        }
    }
}

#[tokio::test]
async fn explicit_quota_preserves_permission_and_precancellation() {
    use minipy_native::{
        codegen::CodeUnit,
        interpreter::{self, InterpreterUnit},
        provider::{Delays, TestProvider},
        runtime::{self, Cancellation, ScriptLimits},
    };
    use std::{rc::Rc, sync::Arc};
    for native in [false, true] {
        for cancelled in [false, true] {
            let source = "print(7)\nx = tools.record_total(1, 2)";
            let provider = Arc::new(TestProvider::new(vec![], Delays::default()));
            let cancellation = Cancellation::default();
            if cancelled {
                cancellation.cancel();
            }
            let limits = ScriptLimits {
                stdout_bytes: 262144,
                allow_print: false,
                ..ScriptLimits::default()
            };
            let out = if native {
                runtime::execute_script(
                    Rc::new(CodeUnit::compile_script(source, &[]).unwrap()),
                    provider.clone(),
                    &[],
                    limits,
                    cancellation,
                    None,
                )
                .await
            } else {
                interpreter::execute_script(
                    Rc::new(InterpreterUnit::prepare_script(source, &[]).unwrap()),
                    provider.clone(),
                    &[],
                    limits,
                    cancellation,
                    None,
                )
                .await
            };
            assert_eq!(
                out.status,
                Err(if cancelled {
                    minipy_native::error::TaskError::Cancelled
                } else {
                    minipy_native::error::TaskError::Permission
                })
            );
            assert!(out.stdout.is_empty());
            assert_eq!(provider.record_count(), 0);
        }
    }
}
