//! Separate event-count build. These counters are not performance evidence.
use super::*;
struct RowsHost(usize);
impl Provider for RowsHost {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        QuoteBuffer::from_quotes(
            (0..self.0)
                .map(|i| Quote {
                    unit_price_cents: i as i64,
                    quantity: 2,
                })
                .collect(),
        )
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}
#[test]
#[ignore = "separate CI dynamic counts, never formal timing"]
fn ci_loop_counts() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for rows in [256usize, 4096, 65536] {
        for (name, source, inputs) in super::ci_probe::cases()
            .into_iter()
            .filter(|(name, _, _)| matches!(*name, "quote" | "madd_loop"))
        {
            let unit = Rc::new(InterpreterUnit::prepare_script(source, &inputs).unwrap());
            let host = Arc::new(RowsHost(rows));
            crate::interpreter_profile::reset();
            let result = rt.block_on(execute_script(
                unit,
                host.clone(),
                &[],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            ));
            let counts = crate::interpreter_profile::take();
            let n = rows as u64;
            let expected = if name == "quote" {
                n * (n - 1)
            } else {
                3 * n * (n + 1) / 2
            };
            assert!(result.status.is_ok());
            assert_eq!(result.stdout, format!("{expected}\n"));
            assert_eq!(
                counts.tape_dispatches,
                if name == "quote" {
                    4 * n + 6
                } else {
                    6 * n + 6
                }
            );
            assert_eq!(counts.fuel_reservations, n / 256);
            assert_eq!(counts.yields, n / 256 - 1);
            println!(
                "CI_COUNTS {}",
                serde_json::json!({"case":name,"rows":rows,"row_payload_bytes":16*rows,
                "dispatches":counts.tape_dispatches,"fuel_reservations":counts.fuel_reservations,"yields":counts.yields})
            );
            let code = Rc::new(crate::codegen::CodeUnit::compile_script(source, &inputs).unwrap());
            let result = rt.block_on(runtime::execute_script(
                code,
                host,
                &[],
                ScriptLimits::default(),
                Cancellation::default(),
                None,
            ));
            assert!(result.status.is_ok());
            assert_eq!(result.stdout, format!("{expected}\n"));
        }
    }
}
