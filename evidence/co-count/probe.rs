//! Temporarily included inside interpreter, never in a formal timing binary.
use super::*;
use std::mem::{size_of, size_of_val};
#[test]
#[ignore = "explicit serial CO diagnostic"]
fn co_probe() {
    println!("CO_LAYOUT op={} operand={} unit={} state={} scratch={}", size_of::<Op>(), size_of::<tape::Operand>(), size_of::<InterpreterUnit>(), size_of::<State>(), size_of::<[i64; crate::ir::MAX_NODES]>());
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    for (name, body, expected) in [
        ("price", "if row.unit_price_cents>threshold:\n        count=count+1", 300),
        ("quantity", "if row.quantity>threshold:\n        count=count+1", 300),
        ("alias", "if row.quantity>count:\n        count=count+1", 1),
        ("unmatched", "count=count+row.quantity", 0),
    ] {
        let source = format!("rows=tools.load_quotes(1)\ncount=0\nthreshold=0\nfor row in rows:\n    {body}\nprint(count)");
        let unit = Rc::new(InterpreterUnit::prepare_script(&source, &[]).unwrap());
        println!("CO_CASE {name} ops={} capacity={} prepared={} live_capacity={} tape={:?}",unit.ops.len(),unit.ops.capacity(),unit.prepared_bytes(),unit.live.capacity(),unit.ops);
        for chunk in [7, 256] {
            let rows: Vec<_> = (0..600).map(|i| Quote {unit_price_cents: i%2, quantity: if i%2 == 0 {-1} else {1}}).collect();
            let provider = Arc::new(crate::provider::TestProvider::new(rows, crate::provider::Delays::default()));
            let mut options = ScriptLimits::default();
            options.limits.chunk = chunk;
            crate::interpreter_profile::reset();
            let future = execute_script(unit.clone(), provider.clone(), &[], options, Cancellation::default(), None);
            println!("CO_FUTURE {name} tape={}", size_of_val(&future));
            let result = rt.block_on(future);
            assert_eq!(result.status, Ok(()));
            assert_eq!(result.stdout, format!("{expected}\n"));
            let c = crate::interpreter_profile::take();
            println!("CO_COUNTS {name} rows=600 chunk={chunk} dispatches={} fuel={} yields={}",c.tape_dispatches,c.fuel_reservations,c.yields);
            let native = Rc::new(crate::codegen::CodeUnit::compile_script(&source, &[]).unwrap());
            let future = runtime::execute_script(native, provider, &[], options, Cancellation::default(), None);
            println!("CO_FUTURE {name} native={}",size_of_val(&future));
            let result = rt.block_on(future);
            assert_eq!(result.status, Ok(()));
            assert_eq!(result.stdout, format!("{expected}\n"));
        }
    }
}
