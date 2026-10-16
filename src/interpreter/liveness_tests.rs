//! Test-only visit counters and independent pre-CD fixed-point oracle.
use super::*;
use std::cell::Cell;
thread_local! {
    static VISITS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}
pub(super) fn visit(json: bool) {
    VISITS.with(|v| {
        let (b, j) = v.get();
        v.set((b + usize::from(!json), j + usize::from(json)));
    });
}
fn reference_buffers(ops: &[Op]) -> (Vec<u64>, usize) {
    let mut live = vec![0; ops.len() + 1];
    let mut visits = 0;
    loop {
        let mut changed = false;
        for pc in (0..ops.len()).rev() {
            visits += 1;
            let next = live[pc + 1];
            let before = match &ops[pc] {
                op if op.branch_target().is_some() => next | live[op.branch_target().unwrap()],
                Op::Jump(t) | Op::Next(t) => live[*t],
                Op::Scan { buffer, end } => next | live[*end] | (1 << buffer),
                Op::Count(count) => next | (1 << count.buffer),
                Op::BufferLen(_, buffer) => next | (1 << buffer),
                Op::Load(target, _) => next & !(1 << target),
                Op::Return => 0,
                _ => next,
            };
            changed |= before != live[pc];
            live[pc] = before;
        }
        if !changed {
            return (live, visits);
        }
    }
}
#[cfg(feature = "tool-json")]
fn reference_json(ops: &[Op], plan: &crate::tool::Plan) -> (Vec<u32>, usize) {
    let mut live = vec![0; ops.len() + 1];
    let mut visits = 0;
    loop {
        let mut changed = false;
        for pc in (0..ops.len()).rev() {
            visits += 1;
            let next = live[pc + 1];
            let before = match &ops[pc] {
                op if op.branch_target().is_some() => next | live[op.branch_target().unwrap()],
                Op::Jump(t) | Op::Next(t) => live[*t],
                Op::Scan { end, .. } => next | live[*end],
                Op::JsonRead(_, _, root, _)
                | Op::JsonString(_, _, root, _)
                | Op::StringEq(_, _, root, _)
                | Op::PrintJsonString(_, root) => next | (1 << root),
                Op::PrintString(i) => next | plan.prints[*i].roots(),
                Op::Tool(call, _, _) => {
                    next | plan.calls[*call]
                        .string
                        .as_ref()
                        .map_or(0, crate::tool::StringSource::roots)
                }
                Op::Return => 0,
                _ => next,
            };
            changed |= before != live[pc];
            live[pc] = before;
        }
        if !changed {
            return (live, visits);
        }
    }
}
fn check(case: &str, unit: InterpreterUnit) {
    let (buffers, old_buffers) = reference_buffers(&unit.ops);
    for (pc, mask) in buffers.iter().enumerate() {
        assert_eq!(unit.buffer_mask(pc), *mask, "{case}: buffer PC {pc}");
    }
    let (buffer_visits, json_visits) = VISITS.with(|v| v.replace((0, 0)));
    if unit.buffers == 0 {
        assert_eq!(unit.live.capacity(), 0, "{case}: no buffer backing");
        assert_eq!(buffer_visits, 0, "{case}: no buffer traversal");
    } else {
        assert_eq!(unit.live.len(), unit.ops.len() + 1);
        assert!(buffer_visits > 0);
    }
    #[cfg(feature = "tool-json")]
    let old_json = if unit.json_live.is_empty() {
        0
    } else {
        let (json, visits) = reference_json(&unit.ops, unit.json.as_ref().unwrap());
        assert_eq!(unit.json_live, json, "{case}: every JSON PC mask");
        visits
    };
    #[cfg(not(feature = "tool-json"))]
    let old_json = 0;
    println!(
        "CD case={case} ops={} buffer_visits={buffer_visits} old_buffer_visits={old_buffers} json_visits={json_visits} old_json_visits={old_json}",
        unit.ops.len()
    );
    assert!(buffer_visits <= old_buffers);
    assert!(json_visits <= old_json);
}
#[test]
fn cd_buffer_masks() {
    for (case, source) in [
        ("scalar", "print(7)"),
        (
            "branch",
            "x=7\nif x>2:\n    print(x+1)\nelse:\n    print(x-1)",
        ),
        ("record", "print(tools.record_total(1,7))"),
        (
            "straight_buffer",
            "a=tools.load_quotes(1)\nb=tools.load_quotes(2)\nprint(len(a)+len(b))",
        ),
        (
            "branch_buffer",
            "a=tools.load_quotes(1)\nb=tools.load_quotes(2)\nif len(a)>0:\n    print(len(b))\nelse:\n    print(len(a))",
        ),
        (
            "loop_buffer",
            "a=tools.load_quotes(1)\nb=tools.load_quotes(2)\nx=0\nfor row in a:\n    if row.quantity>0:\n        x=x+len(b)\nprint(x)",
        ),
        (
            "branch_following_buffer",
            "a=tools.load_quotes(1)\nif len(a)>0:\n    print(1)\nprint(len(a))",
        ),
    ] {
        VISITS.with(|v| v.set((0, 0)));
        check(case, InterpreterUnit::prepare_script(source, &[]).unwrap());
    }
}
#[test]
#[should_panic(expected = "index out of bounds")]
fn nonzero_buffer_mask_keeps_pc_bounds() {
    let unit =
        InterpreterUnit::prepare_script("a=tools.load_quotes(1)\nprint(len(a))", &[]).unwrap();
    unit.buffer_mask(unit.ops.len() + 1);
}
#[cfg(feature = "tool-json")]
#[test]
fn cd_json_masks() {
    use crate::tool::{ArgumentSchema, JsonLeaf, ResultSchema, ToolDeclaration};
    let tools = [
        ToolDeclaration {
            id: 101,
            name: "fetch".into(),
            arguments: vec![ArgumentSchema::I64("key".into())],
            result: ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)]),
        },
        ToolDeclaration {
            id: 102,
            name: "send".into(),
            arguments: vec![ArgumentSchema::String("text".into())],
            result: ResultSchema::I64,
        },
    ];
    for (case, source) in [
        (
            "straight_json",
            "a=tools.fetch(1)\ntext=a['message']\nprint(text)",
        ),
        (
            "branch_json",
            "a=tools.fetch(1)\nb=tools.fetch(2)\nx=1\nif x>0:\n    print(a['message'])\nelse:\n    print(b['message'])",
        ),
        (
            "string_wait",
            "a=tools.fetch(1)\ntext=a['message']\nx=tools.send(text)\nprint(text + '!')\nprint(x)",
        ),
        (
            "loop_json",
            "a=tools.fetch(1)\ntext=a['message']\nrows=tools.load_quotes(1)\nfor row in rows:\n    if row.quantity>0:\n        print(text)\nprint(text)",
        ),
        (
            "branch_following_json",
            "a=tools.fetch(1)\nx=1\nif x>0:\n    print(1)\nprint(a['message'])",
        ),
    ] {
        VISITS.with(|v| v.set((0, 0)));
        check(
            case,
            InterpreterUnit::prepare_with_declarations(source, &[], &tools).unwrap(),
        );
    }
    // Every kind of backward control edge must retain fixed-point propagation,
    // including a self edge. These sealed-shape unit cases do not execute.
    use tape::{Dest, Operand};
    for branch in [
        Op::Jump(0),
        Op::Next(0),
        Op::Branch(Operand::Imm(1), true, 0),
        Op::EqJump(Operand::Imm(1), Operand::Imm(1), 0),
        Op::Scan { buffer: 0, end: 0 },
    ] {
        let ops = vec![
            Op::JsonString(Dest::Local(0), 0, 0, None),
            branch,
            Op::PrintJsonString(0, 1),
            Op::Return,
        ];
        let plan = crate::tool::Plan::default();
        assert_eq!(
            tape::json_liveness(&ops, &plan),
            reference_json(&ops, &plan).0
        );
    }
}
