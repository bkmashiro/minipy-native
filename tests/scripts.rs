use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    error::TaskError,
    ir::Type,
    provider::{Delays, Provider, TestProvider},
    runtime::{self, Cancellation, ScriptLimits, ScriptOutcome, Trace},
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};
fn rows() -> Vec<Quote> {
    vec![
        Quote {
            unit_price_cents: 12,
            quantity: 3,
        },
        Quote {
            unit_price_cents: 800,
            quantity: 1,
        },
        Quote {
            unit_price_cents: -1,
            quantity: 2,
        },
    ]
}
async fn run(
    source: &str,
    values: &[i64],
    schema: &[(&str, Type)],
    limits: ScriptLimits,
) -> (ScriptOutcome, Arc<TestProvider>) {
    let code = Rc::new(CodeUnit::compile_script(source, schema).unwrap());
    let p = Arc::new(TestProvider::new(rows(), Delays::default()));
    let out = runtime::execute_script(
        code,
        p.clone(),
        values,
        limits,
        Cancellation::default(),
        None,
    )
    .await;
    (out, p)
}
#[tokio::test]
async fn scalar_script_private_stdout_and_no_implicit_echo() {
    for source in ["", "True", "1 + 2", "x = 3\nx > 0"] {
        let (out, p) = run(source, &[], &[], ScriptLimits::default()).await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, "");
        assert_eq!(p.load_count(), 0);
        assert_eq!(p.record_count(), 0);
    }
    let (out, p) = run("tools.load_quotes(1)", &[], &[], ScriptLimits::default()).await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "");
    assert_eq!(p.load_count(), 1);
    let (out, _) = run(
        "answer = input * 2 + 1\nprint(answer)\nprint(answer > 0)\nprint(-9223372036854775808)",
        &[20],
        &[("input", Type::I64)],
        ScriptLimits::default(),
    )
    .await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "41\nTrue\n-9223372036854775808\n");
    let (out, _) = run(
        "print(flag)",
        &[2],
        &[("flag", Type::Bool)],
        ScriptLimits::default(),
    )
    .await;
    assert_eq!(out.status, Err(TaskError::Input));
    assert_eq!(out.stdout, "");
}
const SCAN: &str = "quotes = tools.load_quotes(batch_id)\ntotal = 0\nfor quote in quotes:\n    if quote.quantity > 0 and quote.unit_price_cents <= price_limit:\n        amount = quote.unit_price_cents * quote.quantity\n        total += amount\n";
const INPUTS: &[(&str, Type)] = &[("batch_id", Type::I64), ("price_limit", Type::I64)];
#[tokio::test]
async fn nested_predicates_match_value_consumers_and_join_locals() {
    type Oracle = fn(bool, bool, bool) -> bool;
    let expressions: &[(&str, Oracle)] = &[
        ("a and b", |a, b, _| a && b),
        ("a or b", |a, b, _| a || b),
        ("not (a and (b or not c))", |a, b, c| !(a && (b || !c))),
        ("(a or b) and (not a or c)", |a, b, c| (a || b) && (!a || c)),
        ("not (not a or (b and not c))", |a, b, c| !(!a || (b && !c))),
    ];
    for (expr, oracle) in expressions {
        let source = format!(
            "value = {expr}\nprint(value)\nprint({expr})\nif {expr}:\n    x = 11\nelse:\n    x = 23\nif not ({expr}):\n    x += 2\nelse:\n    x += 3\nprint(x)"
        );
        let code = Rc::new(
            CodeUnit::compile_script(
                &source,
                &[("a", Type::Bool), ("b", Type::Bool), ("c", Type::Bool)],
            )
            .unwrap(),
        );
        for bits in 0..8 {
            let inputs = [bits & 1, (bits >> 1) & 1, (bits >> 2) & 1];
            let value = oracle(inputs[0] != 0, inputs[1] != 0, inputs[2] != 0);
            let p = Arc::new(TestProvider::new(rows(), Delays::default()));
            let out = runtime::execute_script(
                code.clone(),
                p.clone(),
                &inputs,
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            )
            .await;
            assert_eq!(out.status, Ok(()), "{expr} {inputs:?}");
            let text = if value { "True" } else { "False" };
            assert_eq!(
                out.stdout,
                format!("{text}\n{text}\n{}\n", if value { 14 } else { 25 })
            );
            assert_eq!(p.record_count(), 0);
        }
    }
}
#[tokio::test]
async fn predicate_short_circuit_preserves_overflow_and_output_order() {
    let schema = &[("x", Type::I64), ("a", Type::Bool)];
    for (expr, a, expected) in [
        ("a and x + 1 > 0", 0, Some(false)),
        ("a or x + 1 > 0", 1, Some(true)),
        ("not (a and (x + 1 > 0 or a))", 0, Some(true)),
        ("a and x + 1 > 0", 1, None),
        ("a or x + 1 > 0", 0, None),
        ("x + 1 > 0 and a", 0, None),
        ("x + 1 > 0 or a", 1, None),
    ] {
        // All three consumers must evaluate the same operand prefix.
        for statement in [
            format!("if {expr}:\n    print(True)\nelse:\n    print(False)"),
            format!("v = {expr}\nprint(v)"),
            format!("print({expr})"),
        ] {
            let source = format!("print(7)\n{statement}\ntools.record_total(1, 2)");
            let (out, p) = run(&source, &[i64::MAX, a], schema, ScriptLimits::default()).await;
            if let Some(value) = expected {
                assert_eq!(out.status, Ok(()), "{statement}");
                assert_eq!(out.stdout, if value { "7\nTrue\n" } else { "7\nFalse\n" });
                assert_eq!(p.record_count(), 1);
            } else {
                assert_eq!(out.status, Err(TaskError::Overflow), "{statement}");
                assert_eq!(out.stdout, "7\n");
                assert_eq!(p.record_count(), 0);
            }
        }
    }
}
#[tokio::test]
async fn nested_predicates_preserve_chunk_carry_and_tool_join_order() {
    let source = "quotes = tools.load_quotes(4)\ntotal = 0\nprevious = 0\nfor q in quotes:\n    if not (q.quantity <= 0 or (q.unit_price_cents > 700 and previous > 0)):\n        total += q.quantity\n    else:\n        total -= 1\n    previous = q.quantity\nif total > 0 and not (previous < 0 or total < previous):\n    receipt = tools.record_total(5, total)\nelse:\n    receipt = tools.record_total(6, previous)\nprint(total)\nprint(receipt)\ntools.record_total(receipt, total)";
    let p = Arc::new(Ordered {
        log: Mutex::new(vec![]),
    });
    let mut limits = ScriptLimits::default();
    limits.limits.chunk = 1;
    let out = runtime::execute_script(
        Rc::new(CodeUnit::compile_script(source, &[]).unwrap()),
        p.clone(),
        &[],
        limits,
        Cancellation::default(),
        None,
    )
    .await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "4\n9\n");
    assert_eq!(
        *p.log.lock().unwrap(),
        vec![('l', 4, 0), ('r', 5, 4), ('r', 9, 4)]
    );
}
#[tokio::test]
async fn real_scan_without_record_and_then_record_receipt() {
    let (out, p) = run(
        "print(tools.record_total(1,2))",
        &[],
        &[],
        ScriptLimits::default(),
    )
    .await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "3\n");
    assert_eq!(p.record_count(), 1);
    let (out, p) = run(
        &format!("{SCAN}print(total)"),
        &[7, 700],
        INPUTS,
        ScriptLimits::default(),
    )
    .await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "34\n");
    assert_eq!(p.load_count(), 1);
    assert_eq!(p.record_count(), 0);
    let (out, p) = run(
        &format!("{SCAN}receipt = tools.record_total(batch_id, total)\nprint(receipt)"),
        &[7, 700],
        INPUTS,
        ScriptLimits::default(),
    )
    .await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "41\n");
    assert_eq!(p.load_count(), 1);
    assert_eq!(p.record_count(), 1);
}
#[tokio::test]
async fn ordered_two_loops_and_tools_preserve_locals_and_total_fuel() {
    let source = "first = tools.load_quotes(seed + 1)\nacc = 0\nfor item in first:\n    amount = item.quantity * item.unit_price_cents\n    acc += amount\nreceipt = tools.record_total(seed, acc)\nsecond = tools.load_quotes(receipt)\nfor row in second:\n    value = row.quantity\n    acc += value\nprint(acc)\nreceipt = tools.record_total(seed + 1, acc)\nprint(receipt)";
    let mut options = ScriptLimits::default();
    options.limits.chunk = 1;
    let (out, p) = run(source, &[7], &[("seed", Type::I64)], options).await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "840\n848\n");
    assert_eq!(p.load_count(), 2);
    assert_eq!(p.record_count(), 2);
    options.limits.rows = 5;
    let (out, p) = run(source, &[7], &[("seed", Type::I64)], options).await;
    assert_eq!(out.status, Err(TaskError::Budget));
    assert_eq!(out.stdout, "");
    assert_eq!(p.record_count(), 1);
    options.limits.rows = 6;
    options.tool_calls = 3;
    let (out, p) = run(source, &[7], &[("seed", Type::I64)], options).await;
    assert_eq!(out.status, Err(TaskError::Budget));
    assert_eq!(out.stdout, "840\n");
    assert_eq!(p.record_count(), 1);
}
#[tokio::test]
async fn branch_tool_resumption_and_loop_print_do_not_duplicate() {
    for (flag, expected) in [(0, "4\n1\n2\n3\n"), (1, "8\n1\n2\n3\n")] {
        let source = "if enabled:\n    x = tools.record_total(3, 5)\nelse:\n    x = tools.record_total(3, 1)\nprint(x)\nbatch = tools.load_quotes(x)\ncounter = 0\nfor row in batch:\n    counter += 1\n    if enabled:\n        amount = counter\n    else:\n        amount = counter\n    print(amount)";
        let mut limits = ScriptLimits::default();
        limits.limits.chunk = 1;
        let (out, p) = run(source, &[flag], &[("enabled", Type::Bool)], limits).await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, expected);
        assert_eq!(p.record_count(), 1);
    }
}
#[tokio::test]
async fn failures_keep_earlier_stdout_and_prevent_later_effects() {
    let (out, p) = run(
        "print(7)\n9223372036854775807 + 1\ntools.record_total(1, 2)",
        &[],
        &[],
        ScriptLimits::default(),
    )
    .await;
    assert_eq!(out.status, Err(TaskError::Overflow));
    assert_eq!(out.stdout, "7\n");
    assert_eq!(p.record_count(), 0);
    let mut limits = ScriptLimits {
        stdout_bytes: 2,
        ..ScriptLimits::default()
    };
    let (out, p) = run(
        "print(7)\nprint(8)\ntools.record_total(1, 2)",
        &[],
        &[],
        limits,
    )
    .await;
    assert_eq!(out.status, Err(TaskError::Budget));
    assert_eq!(out.stdout, "7\n");
    assert_eq!(p.record_count(), 0);
    limits.allow_print = false;
    let (out, _) = run("print(True)", &[], &[], limits).await;
    assert_eq!(out.status, Err(TaskError::Permission));
    assert_eq!(out.stdout, "");
    limits = ScriptLimits::default();
    limits.limits.allow_record = false;
    let (out, p) = run("print(3)\ntools.record_total(1, 2)", &[], &[], limits).await;
    assert_eq!(out.status, Err(TaskError::Permission));
    assert_eq!(out.stdout, "3\n");
    assert_eq!(p.record_count(), 0);
    let code = Rc::new(
        CodeUnit::compile_script("print(3)\nx = tools.record_total(1, 2)\nprint(x)", &[]).unwrap(),
    );
    let p = Arc::new(TestProvider::new(rows(), Delays::default()).with_failures(false, true));
    let out = runtime::execute_script(
        code,
        p,
        &[],
        ScriptLimits::default(),
        Cancellation::default(),
        None,
    )
    .await;
    assert!(matches!(out.status, Err(TaskError::Provider(_))));
    assert_eq!(out.stdout, "3\n");
}
struct Ordered {
    log: Mutex<Vec<(char, i64, i64)>>,
}
impl Provider for Ordered {
    async fn load_quotes(&self, b: i64) -> Result<QuoteBuffer, TaskError> {
        self.log.lock().unwrap().push(('l', b, 0));
        tokio::time::sleep(Duration::from_millis(1)).await;
        QuoteBuffer::from_quotes(rows())
    }
    async fn record_total(&self, b: i64, t: i64) -> Result<i64, TaskError> {
        self.log.lock().unwrap().push(('r', b, t));
        tokio::time::sleep(Duration::from_millis(1)).await;
        Ok(b + t)
    }
}
#[tokio::test]
async fn delayed_order_code_lifetime_and_last_use_buffer_release() {
    let source = format!(
        "{SCAN}receipt = tools.record_total(batch_id, total)\nprint(receipt)\nother = tools.record_total(receipt, 1)\nprint(other)"
    );
    let code = Rc::new(CodeUnit::compile_script(&source, INPUTS).unwrap());
    let weak = Rc::downgrade(&code);
    let p = Arc::new(TestProvider::new(
        rows(),
        Delays {
            load: Duration::from_millis(2),
            record: Duration::from_millis(20),
        },
    ));
    let trace: Trace = Rc::default();
    let task = runtime::execute_script(
        code,
        p.clone(),
        &[7, 700],
        ScriptLimits::default(),
        Cancellation::default(),
        Some(trace.clone()),
    );
    tokio::pin!(task);
    tokio::time::timeout(Duration::from_secs(2), async {
        while !trace.borrow().iter().any(|e| e.phase == "record_wait") {
            tokio::select! { _=&mut task => panic!("must suspend"), _=tokio::task::yield_now()=>{} }
        }
    })
    .await
    .unwrap();
    assert!(weak.upgrade().is_some());
    assert!(p.last_rows().unwrap().upgrade().is_none());
    assert_eq!(
        trace
            .borrow()
            .iter()
            .find(|e| e.phase == "record_wait")
            .unwrap()
            .buffer_bytes,
        0
    );
    let out = task.await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "41\n42\n");
    assert!(weak.upgrade().is_none());
    let code=Rc::new(CodeUnit::compile_script("a = tools.load_quotes(4)\nx = 0\nfor q in a:\n    x += q.quantity\nb = tools.record_total(5, x)\nc = tools.load_quotes(b)\nprint(b)",&[]).unwrap());
    let p = Arc::new(Ordered {
        log: Mutex::new(vec![]),
    });
    let out = runtime::execute_script(
        code,
        p.clone(),
        &[],
        ScriptLimits::default(),
        Cancellation::default(),
        None,
    )
    .await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "11\n");
    assert_eq!(
        *p.log.lock().unwrap(),
        vec![('l', 4, 0), ('r', 5, 6), ('l', 11, 0)]
    );
}
#[tokio::test]
async fn cancelling_after_print_during_record_keeps_output_and_discards_late_completion() {
    let code = Rc::new(
        CodeUnit::compile_script(
            "print(7)\nx = tools.record_total(1, 2)\nprint(x)\ntools.record_total(2, 3)",
            &[],
        )
        .unwrap(),
    );
    let p = Arc::new(TestProvider::new(
        rows(),
        Delays {
            load: Duration::ZERO,
            record: Duration::from_millis(50),
        },
    ));
    let cancel = Cancellation::default();
    let task = runtime::execute_script(
        code,
        p.clone(),
        &[],
        ScriptLimits::default(),
        cancel.clone(),
        None,
    );
    tokio::pin!(task);
    tokio::select! { _=&mut task=>panic!("must wait"), _=tokio::time::sleep(Duration::from_millis(5))=>{} }
    cancel.cancel();
    let out = task.await;
    assert_eq!(out.status, Err(TaskError::Cancelled));
    assert_eq!(out.stdout, "7\n");
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(p.record_count(), 1);
}
#[tokio::test]
async fn compilation_of_different_sources_does_not_reuse_semantics_or_diagnostics() {
    // Same compiling thread: different locals, CFG/Tool imports and errors.
    for i in 1..16 {
        let source = format!("x={i}\nif x > 0:\n    y=x*2\nelse:\n    y=0\nprint(y)");
        let (out, _) = run(&source, &[], &[], ScriptLimits::default()).await;
        assert_eq!(out.status, Ok(()));
        assert_eq!(out.stdout, format!("{}\n", i * 2));
        assert!(
            CodeUnit::compile_with_opt(include_str!("../fixtures/weighted_scan.py"), "invalid")
                .is_err()
        );
        let (out, _) = run("print(True)", &[], &[], ScriptLimits::default()).await;
        assert_eq!(out.stdout, "True\n");
    }
}
#[tokio::test]
async fn future_scans_keep_owners_across_tools_and_live_bytes_are_aggregate() {
    let source = "a=tools.load_quotes(1)\nb=tools.record_total(1,2)\nx=0\nfor q in a:\n    x+=q.quantity\nc=tools.load_quotes(b)\nfor q in c:\n    x+=q.quantity\nprint(x)\ntools.record_total(1,x)";
    let code = Rc::new(CodeUnit::compile_script(source, &[]).unwrap());
    let p = Arc::new(TestProvider::new(rows(), Delays::default()));
    let trace: Trace = Rc::default();
    let out = runtime::execute_script(
        code,
        p,
        &[],
        ScriptLimits::default(),
        Cancellation::default(),
        Some(trace.clone()),
    )
    .await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "12\n");
    let held: Vec<_> = trace
        .borrow()
        .iter()
        .filter(|e| e.phase == "record_wait")
        .map(|e| e.buffer_bytes)
        .collect();
    assert_eq!(held, vec![48, 0]);
    let source = "a=tools.load_quotes(1)\nb=tools.load_quotes(2)\nx=0\nfor q in a:\n    x+=q.quantity\nfor q in b:\n    x+=q.quantity\nprint(x)";
    let mut limits = ScriptLimits::default();
    limits.limits.buffer_bytes = 48;
    let (out, p) = run(source, &[], &[], limits).await;
    assert_eq!(out.status, Err(TaskError::Budget));
    assert_eq!(out.stdout, "");
    assert_eq!(p.load_count(), 2);
    let code=Rc::new(CodeUnit::compile_script("a=tools.load_quotes(1)\nx=7\nfor q in a:\n    amount=q.quantity\n    x+=amount\nprint(x)",&[]).unwrap());
    let p = Arc::new(TestProvider::new(vec![], Delays::default()));
    let out = runtime::execute_script(
        code,
        p,
        &[],
        ScriptLimits::default(),
        Cancellation::default(),
        None,
    )
    .await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "7\n");
}
#[tokio::test]
async fn scalar_values_cross_tools_and_chunk_backedges_only_when_live() {
    let source = "x=40\ny=tools.record_total(1,2)\nprint(x+y)\na=tools.load_quotes(y)\ntotal=1\nprevious=0\nfor q in a:\n    value=q.quantity\n    if value>previous:\n        total+=value\n    previous=value\nreceipt=tools.record_total(y,total)\nprint(total)\nprint(previous)\nprint(receipt)";
    let mut limits = ScriptLimits::default();
    limits.limits.chunk = 1;
    let (out, _) = run(source, &[], &[], limits).await;
    assert_eq!(out.status, Ok(()));
    assert_eq!(out.stdout, "43\n6\n2\n9\n");
}
#[test]
fn frontend_named_safety_bounds_and_rejections() {
    let compile = |s: &str| CodeUnit::compile_script(s, &[]);
    assert!(compile(&" ".repeat(16 * 1024 + 1)).is_err());
    assert!(compile(&format!("x={}0{}", "(".repeat(33), ")".repeat(33))).is_err());
    assert!(compile(&"x=1\n".repeat(513)).is_err());
    assert!(compile(&(0..33).map(|i| format!("v{i}=1\n")).collect::<String>()).is_err());
    for s in [
        "print('hi')",
        "print(1,2)",
        "print(1,end=2)",
        "await print(1)",
        "import os",
        "def f():\n    return 1",
        "while True:\n    print(1)",
        "a=tools.load_quotes(True)",
        "a=tools.load_quotes(1)\na=tools.load_quotes(2)",
        "a=tools.load_quotes(1)\nb=a",
        "if True:\n    a=tools.load_quotes(1)",
        "a=tools.load_quotes(1)\nfor q in a:\n    x=q.quantity\nprint(x)",
        "a=tools.load_quotes(1)\nfor q in a:\n    if q.quantity>0:\n        x=1\n    print(x)",
        "a=tools.load_quotes(1)\nfor q in a:\n    for r in a:\n        print(1)",
        "tools.record_total(1,True)",
        "async def main(batch_id: i64, price_limit: i64) -> bool:\n    return True",
    ] {
        assert!(compile(s).is_err(), "accepted {s}");
    }
}
