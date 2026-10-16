#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
#[path = "../examples/di_corpus.rs"]
mod runner;
use serde_json::{Value, json};

#[tokio::test]
async fn controlled_smoke_both_backends_and_separate_memory_mode() {
    let cases: Vec<runner::Case> = include_str!("../fixtures/di/smoke.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(cases.len(), 10);
    for native in [false, true] {
        for diagnostic in [false, true] {
            for case in &cases {
                case.validate_replay().unwrap();
                let row = runner::run(case, native, diagnostic).await;
                assert_eq!(row["correct"], true, "{row}");
                assert_eq!(row["replay_matched"], true, "{row}");
                assert_eq!(row["timing"].is_null(), diagnostic);
                assert_eq!(row["memory"].is_null(), !diagnostic);
                if row["route"] != "executed" {
                    assert_eq!(row["actual"]["calls"], json!([]));
                    assert_eq!(row["whole_source_admitted"], false);
                }
                if diagnostic && row["route"] == "executed" {
                    let phases = row["memory"]["owner_phases"].as_array().unwrap();
                    assert_eq!(phases.last().unwrap()["unique_json_owner_backing_bytes"], 0);
                    assert_eq!(phases.last().unwrap()["phase"], "prepared_dropped");
                    assert!(row["memory"]["total_task_state_bytes"].is_null());
                    if case.case_id == "json_wait" {
                        assert!(phases.iter().any(|p| p["unique_json_owner_backing_bytes"].as_u64().unwrap() > 0));
                        assert!(phases.iter().any(|p| p["phase"] == "tool_wait_start"));
                    }
                }
                if !diagnostic && case.case_id == "json_wait" {
                    assert_eq!(row["timing"]["tool_waits"].as_array().unwrap().len(), 1);
                    assert!(
                        row["timing"]["tool_waits"][0]["observed_ns"]
                            .as_u64()
                            .unwrap()
                            >= 1_000_000
                    );
                }
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
async fn resource_rejection_and_runtime_mismatch_never_fallback_or_replay() {
    for native in [false, true] {
        let r = row(json!({"case_id":"resource","source":format!("tools.record_total(1, 2)\n#{}", "x".repeat(16384))}), native).await;
        assert_eq!(r["route"], "rejected");
        assert_eq!(r["actual"]["status"], "Resource");
        assert_eq!(r["actual"]["calls"], json!([]));
        let r = row(json!({"case_id":"mismatch","source":"print(7)\nx = tools.record_total(1, 2)","replies":[{"tool_id":1,"arguments":[1,3],"i64":4}]}), native).await;
        assert_eq!(r["route"], "executed");
        assert_eq!(r["actual"]["status"], "Provider");
        assert_eq!(r["actual"]["stdout"], "7\n");
        assert_eq!(
            r["actual"]["calls"],
            json!([{"tool_id":1,"arguments":[1,2]}])
        );
        assert_eq!(r["replay_matched"], false);
    }
}

#[tokio::test]
async fn malformed_json_and_numeric_two_args_use_actual_host_contract() {
    for native in [false, true] {
        let r = row(json!({"case_id":"bad_json","source":"d = tools.fetch(1, 2)\nprint(d['n'])","tools":[{"id":101,"name":"fetch","arguments":[{"name":"a","type":"i64"},{"name":"b","type":"i64"}],"result":{"kind":"json","leaves":[{"path":["n"],"type":"i64"}]}}],"replies":[{"tool_id":101,"arguments":[1,2],"json":"{\"n\":\"wrong\"}"}]}), native).await;
        assert_eq!(r["actual"]["status"], "Json");
        assert_eq!(r["route"], "executed");
        assert_eq!(r["actual"]["calls"].as_array().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn correctness_requires_complete_contract_oracle_and_expected() {
    for native in [false, true] {
        for diagnostic in [false, true] {
            for contract in [None, Some(false), Some(true)] {
                for oracle in [None, Some(false), Some(true)] {
                    for expected in [None, Some(false), Some(true)] {
                        let mut value = json!({"case_id":"eligibility","source":"print(1)"});
                        if let Some(flag) = contract {
                            value["contract_complete"] = json!(flag);
                        }
                        if let Some(flag) = oracle {
                            value["oracle_complete"] = json!(flag);
                        }
                        if let Some(matches) = expected {
                            value["expected"] = json!({"stdout":if matches {"1\n"} else {"wrong\n"},"status":"ok","calls":[]});
                        }
                        let case: runner::Case = serde_json::from_value(value.clone()).unwrap();
                        let result = runner::run(&case, native, diagnostic).await;
                        let correct = if contract == Some(true) && oracle == Some(true) {
                            json!(expected)
                        } else {
                            Value::Null
                        };
                        assert_eq!(result["correct"], correct, "{value}: {result}");
                        assert_eq!(result["actual"]["stdout"], "1\n");
                    }
                }
            }
        }
    }
}

#[test]
fn fixture_contract_is_strict_but_private_top_level_fields_are_not_executed() {
    let good = json!({"case_id":"private","source":"print(1)","provenance":{"command":"never execute this"}});
    assert!(serde_json::from_value::<runner::Case>(good.clone()).is_ok());
    for flag in ["contract_complete", "oracle_complete"] {
        let mut c = good.clone();
        c[flag] = Value::Null;
        assert!(serde_json::from_value::<runner::Case>(c.clone()).is_ok());
        c[flag] = json!(1);
        assert!(serde_json::from_value::<runner::Case>(c).is_err());
    }
    for reply in [
        json!({"tool_id":2,"arguments":[],"i64":1,"json":"{}"}),
        json!({"tool_id":2,"arguments":[],"i64":1,"wait_ms":1001}),
    ] {
        let mut c = good.clone();
        c["replies"] = json!([reply]);
        let c: runner::Case = serde_json::from_value(c).unwrap();
        assert!(c.validate_replay().is_err());
    }
    let mut c = good;
    c["tools"] = json!([{"id":2,"name":"x","arguments":[],"result":{"kind":"object_array","path":["items"],"i64_fields":["n"],"leaves":[]}}]);
    assert!(serde_json::from_value::<runner::Case>(c).is_err());
}
