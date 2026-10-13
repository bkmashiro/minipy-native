//! Bounded rerun of the parameterized helper and contemporaneous controls.
use super::*;
use typed_helpers::{Field, sum_product_where_gt_le};

fn runner() -> Result<MontyRun, String> {
    MontyRun::new(
        "handle = load_quotes(batch_id)\ntotal = sum_product_where_gt_le(handle, 1, 0, 0, price_limit, 0, 1)\nrecord_total(batch_id, total)\n".into(),
        "generic.py",
        vec!["batch_id".into(), "price_limit".into(), "load_quotes".into(),
             "sum_product_where_gt_le".into(), "record_total".into()],
        CompileOptions::default(),
    ).map_err(|e| e.to_string())
}

fn field(index: i64) -> Result<Field, String> {
    match index {
        0 => Ok(Field::Price),
        1 => Ok(Field::Quantity),
        _ => Err("unknown Quote/v1 field".into()),
    }
}

fn execute(runner: MontyRun, n: usize) -> Result<i64, String> {
    let args = vec![
        MontyObject::int(BATCH),
        MontyObject::int(LIMIT),
        MontyObject::function("load_quotes", None),
        MontyObject::function("sum_product_where_gt_le", None),
        MontyObject::function("record_total", None),
    ];
    let p = runner
        .start(args, ResourceTracker::default(), PrintWriter::Disabled)
        .map_err(|e| e.to_string())?;
    let RunProgress::FunctionCall(load) = p else {
        return Err("expected load".into());
    };
    if load.function_name != "load_quotes"
        || load.args.arg(0).and_then(|v| v.as_int()) != Some(BATCH)
    {
        return Err("invalid load request".into());
    }
    let buffer = QuoteBuffer::from_quotes(input(n)).map_err(|e| e.to_string())?;
    let p = load
        .resume(MontyObject::int(1), PrintWriter::Disabled)
        .map_err(|e| e.to_string())?;
    let RunProgress::FunctionCall(call) = p else {
        return Err("expected helper".into());
    };
    if call.function_name != "sum_product_where_gt_le" {
        return Err("unknown helper".into());
    }
    let arg = |i| {
        call.args
            .arg(i)
            .and_then(|v| v.as_int())
            .ok_or_else(|| "invalid helper argument".to_string())
    };
    if arg(0)? != 1 {
        return Err("unauthorized buffer handle".into());
    }
    // Every choice comes from the actual Monty call; no case/fixture dispatch.
    let total = sum_product_where_gt_le(
        buffer.rows(),
        field(arg(1)?)?,
        arg(2)?,
        field(arg(3)?)?,
        arg(4)?,
        field(arg(5)?)?,
        field(arg(6)?)?,
    )
    .map_err(|e| e.to_string())?;
    drop(buffer);
    let p = call
        .resume(MontyObject::int(total), PrintWriter::Disabled)
        .map_err(|e| e.to_string())?;
    let RunProgress::FunctionCall(record) = p else {
        return Err("expected record".into());
    };
    if record.function_name != "record_total"
        || record.args.arg(0).and_then(|v| v.as_int()) != Some(BATCH)
        || record.args.arg(1).and_then(|v| v.as_int()) != Some(total)
    {
        return Err("invalid record request".into());
    }
    let receipt = total.checked_add(BATCH).ok_or("receipt overflow")?;
    let p = record
        .resume(MontyObject::int(receipt), PrintWriter::Disabled)
        .map_err(|e| e.to_string())?;
    let RunProgress::Complete(v) = p else {
        return Err("expected completion".into());
    };
    v.as_ref().as_int().ok_or_else(|| "invalid receipt".into())
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_current_thread().build()?;
    let fixtures = std::env::var_os("MINIPY_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures"));
    let source = std::fs::read_to_string(fixtures.join("weighted_scan.py"))?;
    let code = Rc::new(CodeUnit::compile(&source)?);
    let generic_warm = runner()?;
    let monty_warm = super::runner(false)?;
    println!(
        "# generic=v1,opt=speed,provider=ready,stateful_branch=unsupported; fixture floor remains historical"
    );
    println!(
        "case,rows,engine,mode,chunk,sample,compile_ns,conversion_ns,execute_cleanup_ns,total_ns,code_bytes,receipt"
    );
    for n in ROW_COUNTS {
        let receipt = expected(&input(n), false);
        for mode in ["cold", "warm"] {
            for i in 0..ITERS {
                // Interleave engine cells, not a repeat of the unrelated fairness campaign.
                for engine in ["monty_generic_helper", "monty", "native256", "native4096"] {
                    let start = Instant::now();
                    let t = Instant::now();
                    let (compile, conversion, execution, bytes, r, chunk) = match engine {
                        "monty_generic_helper" => {
                            let runner = if mode == "cold" {
                                runner()?
                            } else {
                                generic_warm.clone()
                            };
                            let compile = t.elapsed().as_nanos();
                            let t = Instant::now();
                            let r = execute(runner, n).map_err(std::io::Error::other)?;
                            (compile, 0, t.elapsed().as_nanos(), 0, r, 0)
                        }
                        "monty" => {
                            let runner = if mode == "cold" {
                                super::runner(false)?
                            } else {
                                monty_warm.clone()
                            };
                            let compile = t.elapsed().as_nanos();
                            let t = Instant::now();
                            let rows = input(n);
                            let (args, quotes, _) = monty_input(&rows);
                            drop(rows);
                            let conversion = t.elapsed().as_nanos();
                            let t = Instant::now();
                            let (r, calls) =
                                run_monty(runner, args, quotes).map_err(std::io::Error::other)?;
                            let execution = t.elapsed().as_nanos();
                            assert_eq!(calls, 2);
                            (compile, conversion, execution, 0, r, 0)
                        }
                        _ => {
                            let code = if mode == "cold" {
                                Rc::new(CodeUnit::compile(&source)?)
                            } else {
                                code.clone()
                            };
                            let compile = t.elapsed().as_nanos();
                            let bytes = code.code_bytes;
                            let chunk = if engine == "native256" { 256 } else { 4096 };
                            let t = Instant::now();
                            let provider = Arc::new(ImmediateProvider {
                                source: Mutex::new(Some(QuoteBuffer::from_quotes(input(n))?)),
                            });
                            let conversion = t.elapsed().as_nanos();
                            let t = Instant::now();
                            let r = rt.block_on(runtime::execute(
                                code,
                                provider,
                                BATCH,
                                LIMIT,
                                Limits {
                                    chunk,
                                    ..Limits::default()
                                },
                                Cancellation::default(),
                                None,
                            ))?;
                            (compile, conversion, t.elapsed().as_nanos(), bytes, r, chunk)
                        }
                    };
                    let total = start.elapsed().as_nanos();
                    assert_eq!(r, receipt);
                    let engine = if chunk == 0 { engine } else { "native" };
                    println!(
                        "weighted_scan,{n},{engine},{mode},{chunk},{i},{compile},{conversion},{execution},{total},{bytes},{r}"
                    );
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monty_generic_call_matches_oracle() {
        for n in ROW_COUNTS {
            assert_eq!(
                execute(runner().unwrap(), n).unwrap(),
                expected(&input(n), false)
            );
        }
        assert!(field(2).is_err());
    }
}
