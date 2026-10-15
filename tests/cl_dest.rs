//! Compact destination regression: public admission and both real executors.
use minipy_native::{
    buffer::Quote,
    codegen::CodeUnit,
    interpreter::{self, InterpreterUnit},
    ir::{MAX_LOCALS, Type},
    provider::{Delays, TestProvider},
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc};

// 32 leaves, 446 unary nodes and 31 binary nodes: 509 expression nodes.
// With module + print statement + print call this reaches MAX_NODES=512.
fn near_limit_expr() -> String {
    let mut leaves: Vec<_> = (0..32)
        .map(|i| {
            let mut s = "1".to_owned();
            for _ in 0..if i < 30 { 14 } else { 13 } {
                s = format!("abs({s})");
            }
            s
        })
        .collect();
    while leaves.len() > 1 {
        leaves = leaves
            .chunks_exact(2)
            .map(|p| format!("({}+{})", p[0], p[1]))
            .collect();
    }
    leaves.pop().unwrap()
}

#[tokio::test]
async fn admitted_frontier_locals_aliases_and_many_chunks() {
    let locals = (0..MAX_LOCALS)
        .map(|i| format!("v{i}={i}\n"))
        .collect::<String>();
    let deep = format!("print({})", near_limit_expr());
    let cases = [
        (format!("{locals}v31=v31+11\nprint(v31)"), "42\n"),
        (deep, "32\n"),
        (
            "x=3\nx=(x+1)*(x-1)+x\nif x>5 and not x<0:\n    print(x)\nelse:\n    print(0)".into(),
            "11\n",
        ),
        (
            "rows=tools.load_quotes(1)\ntotal=0\nfor row in rows:\n    total=total+(row.unit_price_cents+1)*(row.quantity+1)\nprint(total)".into(),
            "19200\n",
        ),
    ];
    let mut limits = ScriptLimits::default();
    limits.limits.chunk = 7;
    for (source, expected) in cases {
        let tape = Rc::new(InterpreterUnit::prepare_script(&source, &[]).unwrap());
        let native = Rc::new(CodeUnit::compile_script(&source, &[]).unwrap());
        let host = || {
            Arc::new(TestProvider::new(
                vec![
                    Quote {
                        unit_price_cents: 7,
                        quantity: 3
                    };
                    600
                ],
                Delays::default(),
            ))
        };
        let a =
            interpreter::execute_script(tape, host(), &[], limits, Cancellation::default(), None)
                .await;
        let b = runtime::execute_script(native, host(), &[], limits, Cancellation::default(), None)
            .await;
        assert_eq!(a.status, Ok(()), "{source}");
        assert_eq!(b.status, Ok(()), "{source}");
        assert_eq!(a.stdout, expected);
        assert_eq!(b.stdout, expected);
    }
}

#[test]
fn admission_limits_still_return_compile_errors() {
    let locals = (0..=MAX_LOCALS)
        .map(|i| format!("v{i}={i}\n"))
        .collect::<String>();
    let too_deep = format!("print(abs({}))", near_limit_expr());
    for (source, message) in [(locals, "MAX_LOCALS"), (too_deep, "MAX_NODES")] {
        let a = InterpreterUnit::prepare_script(&source, &[]).err().unwrap();
        let b = CodeUnit::compile_script(&source, &[]).err().unwrap();
        assert!(a.to_string().contains(message), "{a}");
        assert_eq!(a.to_string(), b.to_string());
    }
    let names: Vec<_> = (0..=MAX_LOCALS).map(|i| format!("i{i}")).collect();
    let inputs: Vec<_> = names.iter().map(|n| (n.as_str(), Type::I64)).collect();
    assert!(InterpreterUnit::prepare_script("print(1)", &inputs).is_err());
    assert!(CodeUnit::compile_script("print(1)", &inputs).is_err());
}

#[cfg(feature = "debug-runtime")]
#[test]
fn high_temporary_debug_step_preserves_index() {
    use interpreter::debug::Engine;
    let source = format!("print({})", near_limit_expr());
    let mut engine = Engine::default();
    let unit = engine.prepare(&source, &[]).unwrap()["unit"]
        .as_u64()
        .unwrap() as usize;
    let task = engine.create(unit, &[], ScriptLimits::default()).unwrap()["task"]
        .as_u64()
        .unwrap() as usize;
    let mut max_slot = 0;
    for _ in 0..1024 {
        let r = engine.advance(task, 1, true).unwrap();
        for e in r["events"].as_array().unwrap() {
            if let Some(writes) = e["detail"]["execution"]["scratch"].as_array() {
                for write in writes {
                    max_slot = max_slot.max(write["slot"].as_u64().unwrap());
                }
            }
        }
        if r["snapshot"]["status"] == "done" {
            assert_eq!(r["snapshot"]["stdout"], "32\n");
            assert_eq!(max_slot, 476);
            return;
        }
        assert_ne!(r["snapshot"]["status"], "error");
    }
    panic!("debug did not finish");
}

#[cfg(feature = "debug-runtime")]
#[test]
fn debug_reports_unchanged_local_destination() {
    use interpreter::debug::Engine;
    let mut engine = Engine::default();
    let unit = engine.prepare("x=1\nx=x\nprint(x)", &[]).unwrap()["unit"]
        .as_u64()
        .unwrap() as usize;
    let task = engine.create(unit, &[], ScriptLimits::default()).unwrap()["task"]
        .as_u64()
        .unwrap() as usize;
    let mut writes = 0;
    for _ in 0..8 {
        let r = engine.advance(task, 1, true).unwrap();
        for e in r["events"].as_array().unwrap() {
            if let Some(slots) = e["detail"]["execution"]["writes"].as_array() {
                for slot in slots {
                    assert_eq!(slot["slot"], 0);
                    assert_eq!(slot["value"], "1");
                    writes += 1;
                }
            }
        }
        if r["snapshot"]["status"] == "done" {
            assert_eq!(r["snapshot"]["stdout"], "1\n");
            assert_eq!(writes, 2);
            return;
        }
    }
    panic!("debug did not finish");
}
