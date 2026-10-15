//! Independent definite-write oracle across all CFG edges and turn boundaries.
use super::*;
use crate::ir::MAX_NODES;
use tape::{Dest, Operand};

fn verify(unit: &InterpreterUnit) -> usize {
    let mut incoming = vec![None; unit.ops.len() + 1];
    incoming[0] = Some([false; MAX_NODES]);
    let mut pending = vec![0];
    let mut high = 0;
    while let Some(pc) = pending.pop() {
        if pc == unit.ops.len() {
            continue;
        }
        let mut written = incoming[pc].unwrap();
        let mut read = |a: Operand| {
            if let Operand::Temp(i) = a {
                high = high.max(i + 1);
                assert!(written[i], "read before write at {pc}: {:?}", unit.ops[pc]);
            }
        };
        let dest = match unit.ops[pc] {
            Op::Copy(d, a) | Op::Abs(d, a) => {
                read(a);
                Some(d)
            }
            Op::Min(d, a, b)
            | Op::Max(d, a, b)
            | Op::Add(d, a, b)
            | Op::Sub(d, a, b)
            | Op::Mul(d, a, b)
            | Op::Eq(d, a, b)
            | Op::Ne(d, a, b)
            | Op::Lt(d, a, b)
            | Op::Le(d, a, b)
            | Op::Gt(d, a, b)
            | Op::Ge(d, a, b) => {
                read(a);
                read(b);
                Some(d)
            }
            Op::EqJump(a, b, _)
            | Op::NeJump(a, b, _)
            | Op::LtJump(a, b, _)
            | Op::LeJump(a, b, _)
            | Op::GtJump(a, b, _)
            | Op::GeJump(a, b, _)
            | Op::Record(_, a, b) => {
                read(a);
                read(b);
                None
            }
            Op::Branch(a, ..) | Op::Load(_, a) | Op::Print(a, _) => {
                read(a);
                None
            }
            Op::BufferLen(d, _) => Some(d),
            #[cfg(feature = "tool-json")]
            Op::JsonRead(d, ..) | Op::JsonString(d, ..) => Some(d),
            _ => None,
        };
        if let Some(Dest::Temp(i)) = dest {
            high = high.max(i + 1);
            written[i] = true;
        }
        // Next may yield. Every effect resumes only after its whole statement.
        let reset = matches!(unit.ops[pc], Op::Load(..) | Op::Record(..) | Op::Next(..));
        #[cfg(feature = "tool-json")]
        let reset = reset || matches!(unit.ops[pc], Op::Tool(..));
        if reset {
            written.fill(false);
        }
        let successors = match unit.ops[pc] {
            Op::Jump(t) | Op::Next(t) => vec![t],
            Op::Return => vec![],
            Op::Scan { end, .. } => vec![pc + 1, end],
            _ => unit.ops[pc]
                .branch_target()
                .map_or_else(|| vec![pc + 1], |t| vec![pc + 1, t]),
        };
        for next in successors {
            let merged = incoming[next].map_or(written, |old: [bool; MAX_NODES]| {
                std::array::from_fn(|i| old[i] && written[i])
            });
            if incoming[next] != Some(merged) {
                incoming[next] = Some(merged);
                pending.push(next);
            }
        }
    }
    assert_eq!(high, unit.temp_slots);
    high
}
fn tree(depth: usize) -> String {
    if depth == 0 {
        "(x+1)".into()
    } else {
        let s = tree(depth - 1);
        format!("({s}+{s})")
    }
}
#[test]
fn scratch_bounds_and_definite_writes() {
    let mut sources = vec![
        "print(7)".to_string(),
        "x=1\nx=(x+1)*(x+2)\nprint(x)".into(),
        "x=1\ncond=(x>0 and x+1>0) or not x<1\nif cond:\n    print(1+2)\nelse:\n    print(2+3)".into(),
        "x=1\na=tools.load_quotes(x+1)\nfor row in a:\n    x=x+(row.unit_price_cents+1)*(row.quantity+1)\nz=tools.record_total(x+1,x+2)\nprint(x+1)".into(),
    ];
    for depth in [2, 3, 4, 6] {
        sources.push(format!("x=1\nprint({})", tree(depth)));
    }
    let mut seen_small = false;
    let mut seen_large = false;
    for source in sources {
        let unit = InterpreterUnit::prepare_script(&source, &[]).unwrap();
        let slots = verify(&unit);
        seen_small |= slots <= 16;
        seen_large |= slots > 16;
        println!("CK slots={slots} ops={}", unit.ops.len());
    }
    assert!(seen_small && seen_large);
    assert!(InterpreterUnit::prepare_script(&format!("x=1\nprint({})", tree(9)), &[]).is_err());
}
#[cfg(feature = "tool-json")]
#[test]
fn scratch_json_turns() {
    use crate::tool::*;
    let tools = [ToolDeclaration {
        id: 101,
        name: "fetch".into(),
        arguments: vec![ArgumentSchema::I64("key".into())],
        result: ResultSchema::JsonTyped(vec![(vec!["n".into()], JsonLeaf::I64)]),
    }];
    let unit = InterpreterUnit::prepare_with_declarations(
        "x=1\na=tools.fetch((x+1)*(x+2))\nx=a['n']+1\nb=tools.fetch(x+2)\nprint((b['n']+1)*(x+2))",
        &[],
        &tools,
    )
    .unwrap();
    verify(&unit);
}
