//! AO admission evidence only. No new language or kernel implementation.
#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
use minipy_native::{
    codegen::CodeUnit,
    interpreter::InterpreterUnit,
    ir::Type,
    tool::{ResultSchema, ToolSchema},
};

#[test]
fn kernel_candidates_need_new_value_or_iteration_contracts() {
    let tools = [ToolSchema {
        id: 101,
        name: "fetch".into(),
        arguments: vec!["batch_id".into()],
        result: ResultSchema::Json(vec![
            vec!["data".into(), "a".into()],
            vec!["data".into(), "b".into()],
        ]),
    }];
    let inputs = [("batch_id", Type::I64)];
    for (label, body, admitted) in [
        (
            "multi_i64",
            "p = tools.fetch(batch_id)\nprint(p[\"data\"][\"a\"] + p[\"data\"][\"b\"])\n",
            true,
        ),
        (
            "raw_subview_alias",
            "p = tools.fetch(batch_id)\nd = p[\"data\"]\nprint(d[\"a\"])\n",
            true,
        ),
        (
            "raw_subview_output",
            "p = tools.fetch(batch_id)\nprint(p[\"data\"])\n",
            false,
        ),
        (
            "json_encode",
            "p = tools.fetch(batch_id)\nprint(json.dumps(p[\"data\"]))\n",
            false,
        ),
        ("contains", "print(\"a\" in \"payload\")\n", false),
        (
            "startswith",
            "print(\"payload\".startswith(\"p\"))\n",
            false,
        ),
        ("split", "print(\"a,b\".split(\",\"))\n", false),
        ("join", "print(\",\".join([\"a\", \"b\"]))\n", false),
        (
            "filter_project",
            "p = tools.fetch(batch_id)\nx = [r[\"a\"] for r in p[\"data\"] if r[\"b\"] > 0]\n",
            false,
        ),
        (
            "any",
            "q = tools.load_quotes(batch_id)\nprint(any(r.quantity > 0 for r in q))\n",
            false,
        ),
        (
            "all",
            "q = tools.load_quotes(batch_id)\nprint(all(r.quantity > 0 for r in q))\n",
            false,
        ),
        (
            "find",
            "q = tools.load_quotes(batch_id)\nx = next((r for r in q if r.quantity > 0), None)\n",
            false,
        ),
        (
            "count",
            "q = tools.load_quotes(batch_id)\nprint(len([r for r in q if r.quantity > 0]))\n",
            false,
        ),
        (
            "sum",
            "q = tools.load_quotes(batch_id)\nprint(sum(r.quantity for r in q))\n",
            false,
        ),
    ] {
        let tape = InterpreterUnit::prepare_with_tools(body, &inputs, &tools);
        let native = CodeUnit::compile_with_tools(body, &inputs, &tools);
        assert_eq!(tape.is_ok(), admitted, "tape: {label}");
        assert_eq!(native.is_ok(), admitted, "native: {label}");
        println!("{label}: admitted={admitted} (tape and native)");
    }
}
