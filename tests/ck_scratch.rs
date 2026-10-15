use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::Type,
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
};
use std::{rc::Rc, sync::Arc};
struct Host;
impl Provider for Host {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        tokio::task::yield_now().await;
        QuoteBuffer::from_quotes(vec![
            Quote {
                unit_price_cents: 1,
                quantity: 2
            };
            600
        ])
    }
    async fn record_total(&self, _: i64, total: i64) -> Result<i64, TaskError> {
        tokio::task::yield_now().await;
        Ok(total)
    }
}
fn tree(depth: usize) -> String {
    if depth == 0 {
        "(x+1)".into()
    } else {
        let s = tree(depth - 1);
        format!("({s}+{s})")
    }
}
#[tokio::test]
async fn small_and_fallback_scratch_match_native_across_turns() {
    for depth in [2, 3, 4, 6] {
        let expression = tree(depth);
        for source in [
            format!("print({expression})"),
            format!("if x>0:\n    print({expression})\nelse:\n    print(0)"),
            format!(
                "rows=tools.load_quotes((x+1)*(x+2))\ntotal=0\nfor row in rows:\n    total=total+{expression}+row.quantity\nr=tools.record_total(x+1,total+1)\nprint(r+1)"
            ),
        ] {
            let schema = [("x", Type::I64)];
            let unit = Rc::new(InterpreterUnit::prepare_script(&source, &schema).unwrap());
            let native = Rc::new(CodeUnit::compile_script(&source, &schema).unwrap());
            let mut limits = ScriptLimits::default();
            limits.limits.chunk = 7;
            let a = interpreter::execute_script(
                unit,
                Arc::new(Host),
                &[3],
                limits,
                Cancellation::default(),
                None,
            )
            .await;
            let b = runtime::execute_script(
                native,
                Arc::new(Host),
                &[3],
                limits,
                Cancellation::default(),
                None,
            )
            .await;
            assert_eq!(a.status, Ok(()), "{source}");
            assert_eq!(a.status, b.status);
            assert_eq!(a.stdout, b.stdout);
            let value = 4 * (1_i64 << depth);
            let expected = if source.starts_with("rows") {
                600 * (value + 2) + 2
            } else {
                value
            };
            assert_eq!(a.stdout, format!("{expected}\n"));
        }
    }
}
