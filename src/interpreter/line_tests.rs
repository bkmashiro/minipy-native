//! Integrated single-instruction tape/debug dispatch, not a scanner analogue.
use super::*;
use crate::tool::{JsonLeaf, ObjectArraySchema, ResultSchema, ToolDeclaration};
#[test]
fn debug_lines_break_and_sequential_array() {
    let declarations = [
        ToolDeclaration {
            id: 201,
            name: "log".into(),
            arguments: vec![],
            result: ResultSchema::JsonTyped(vec![(vec!["content".into()], JsonLeaf::String)]),
        },
        ToolDeclaration {
            id: 202,
            name: "items".into(),
            arguments: vec![],
            result: ResultSchema::JsonObjectArray(
                ObjectArraySchema {
                    path: vec!["items".into()],
                    i64_fields: vec!["n".into()],
                }
                .into(),
            ),
        },
    ];
    let source = "data=tools.log()\nn=0\nfor line in data.content.splitlines():\n    if 'WARN' in line and line.startswith('WARN'):\n        print(line)\n        n+=1\n        break\nitems=tools.items()\nfor row in items['items']:\n    n+=row['n']\n    break\nfor other in data.content.splitlines():\n    n+=1\nprint(n)";
    let unit = InterpreterUnit::prepare_with_declarations(source, &[], &declarations).unwrap();
    let mut expected = None;
    for step in [false, true] {
        let mut state = State {
            locals: [0; MAX_LOCALS],
            pc: 0,
            index: 0,
            chunk_left: 0,
            remaining: 100,
            row: None,
        };
        let mut output = Output::new(1024, true);
        output.json = Some(Box::new(crate::json_view::State::new(
            unit.json.as_ref().unwrap().clone(),
            1024,
            10000,
        )));
        let mut temps = [0; crate::ir::MAX_NODES];
        let mut limits = ScriptLimits::default();
        limits.limits.chunk = 1;
        let mut destinations = 0;
        let mut yields = 0;
        for _ in 0..200 {
            if step && unit.ops.get(state.pc).and_then(Op::destination).is_some() {
                destinations += 1;
            }
            let boundary = if step {
                tape::run::<true>(
                    &unit,
                    &mut state,
                    &[],
                    &limits.limits,
                    &mut output,
                    &mut temps,
                )
            } else {
                tape::run::<false>(
                    &unit,
                    &mut state,
                    &[],
                    &limits.limits,
                    &mut output,
                    &mut temps,
                )
            }
            .unwrap();
            match boundary {
                Boundary::Step => {}
                Boundary::Yield => {
                    yields += 1;
                    output
                        .json
                        .as_mut()
                        .unwrap()
                        .release(unit.json_live[state.pc]);
                }
                Boundary::Tool(call, target, _) => {
                    output
                        .json
                        .as_mut()
                        .unwrap()
                        .release(unit.json_live[state.pc]);
                    let raw = if unit.json.as_ref().unwrap().calls[call].id == 201 {
                        br#"{"content":"no\nWARN:x\nlast"}"#.as_slice()
                    } else {
                        br#"{"items":[{"n":7},{"n":9}]}"#.as_slice()
                    };
                    output
                        .json
                        .as_mut()
                        .unwrap()
                        .install(target, bytes::Bytes::copy_from_slice(raw), 1024)
                        .unwrap();
                }
                Boundary::Done => break,
                _ => panic!("unexpected boundary"),
            }
        }
        assert_eq!(state.pc, unit.ops.len());
        assert_eq!(output.stdout, "WARN:x\n11\n");
        assert!(yields >= 3);
        if step {
            assert!(destinations > 0);
        }
        if let Some(expected) = &expected {
            assert_eq!(&output.stdout, expected);
        } else {
            expected = Some(output.stdout);
        }
    }
    println!(
        "DR layout Op={} tape_State={} json_State={} Frame={} Output={} Stmt={} Expr={} StringSource={} Unit={} prepared={}",
        std::mem::size_of::<Op>(),
        std::mem::size_of::<State>(),
        std::mem::size_of::<crate::json_view::State>(),
        std::mem::size_of::<crate::codegen::Frame>(),
        std::mem::size_of::<Output>(),
        std::mem::size_of::<Stmt>(),
        std::mem::size_of::<Expr>(),
        std::mem::size_of::<crate::tool::StringSource>(),
        std::mem::size_of::<InterpreterUnit>(),
        unit.prepared_bytes()
    );
}
