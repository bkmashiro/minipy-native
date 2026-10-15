//! Diagnostic overlay only. Not part of the production or formal timing build.
use super::*;
use std::mem::{align_of, size_of};
use tape::{Dest, Operand};

#[allow(dead_code)]
#[derive(Clone, Copy)]
enum SmallDest {
    Local(u8),
    Temp(u16),
}
// Same full Op variant set and all other fields, changing only destination width.
#[allow(dead_code)]
enum SmallDestOp {
    Copy(SmallDest, Operand),
    BufferLen(SmallDest, usize),
    Abs(SmallDest, Operand),
    Min(SmallDest, Operand, Operand),
    Max(SmallDest, Operand, Operand),
    JsonRead(SmallDest, usize, usize, bool),
    JsonString(SmallDest, usize, usize, Option<usize>),
    Tool(usize, usize, [u8; 2]),
    Add(SmallDest, Operand, Operand),
    Sub(SmallDest, Operand, Operand),
    Mul(SmallDest, Operand, Operand),
    Eq(SmallDest, Operand, Operand),
    Ne(SmallDest, Operand, Operand),
    Lt(SmallDest, Operand, Operand),
    Le(SmallDest, Operand, Operand),
    Gt(SmallDest, Operand, Operand),
    Ge(SmallDest, Operand, Operand),
    EqJump(Operand, Operand, usize),
    NeJump(Operand, Operand, usize),
    LtJump(Operand, Operand, usize),
    LeJump(Operand, Operand, usize),
    GtJump(Operand, Operand, usize),
    GeJump(Operand, Operand, usize),
    Branch(Operand, bool, usize),
    Jump(usize),
    Start,
    Scan { buffer: usize, end: usize },
    Next(usize),
    Load(usize, Operand),
    Record(usize, Operand, Operand),
    Print(Operand, Type),
    PrintJsonString(usize, usize),
    PrintString(usize),
    Return,
}

pub(crate) fn environment() -> crate::tool::ToolEnvironment {
    use crate::tool::*;
    ToolEnvironment::new(
        (0..16)
            .map(|i| ToolDeclaration {
                id: 101 + i,
                name: if i == 0 {
                    "fetch".into()
                } else {
                    format!("unused{i}")
                },
                arguments: if i == 1 {
                    vec![ArgumentSchema::String("text".into())]
                } else {
                    vec![ArgumentSchema::I64("key".into())]
                },
                result: if i == 1 {
                    ResultSchema::I64
                } else {
                    ResultSchema::JsonTyped(vec![(vec!["message".into()], JsonLeaf::String)])
                },
            })
            .collect(),
    )
    .unwrap()
}
pub(crate) fn cases() -> Vec<(&'static str, &'static str, Vec<(&'static str, Type)>)> {
    vec![
        ("scalar", "print(7+1)", vec![]),
        (
            "bound_tree",
            "print((a+1)*(b-1)+a)",
            vec![("a", Type::I64), ("b", Type::I64)],
        ),
        ("madd_short", "x=7\nx=x*3+x\nprint(x)", vec![]),
        (
            "quote",
            "rows=tools.load_quotes(1)\ntotal=0\nfor row in rows:\n    total=total+row.unit_price_cents*row.quantity\nprint(total)",
            vec![],
        ),
        (
            "madd_loop",
            "rows=tools.load_quotes(1)\ntotal=0\nfor row in rows:\n    total=total+(row.unit_price_cents+1)*(row.quantity+1)\nprint(total)",
            vec![],
        ),
        (
            "ready",
            "data=tools.fetch(7)\nalias=data\ntext=alias['message']\nprint('error' in text)\nprint(text)",
            vec![],
        ),
        (
            "branch",
            "x=7\nif x>2:\n    print(x+1)\nelse:\n    print(x-1)",
            vec![],
        ),
        (
            "joined_sink",
            "n=7\ndata=tools.fetch(n)\ntext=data['message']\nprint('['+text+'/'+text+']')",
            vec![],
        ),
        (
            "async_string",
            "data=tools.fetch(7)\ntext=data['message']\nr=tools.unused1(text)\nprint(r)",
            vec![],
        ),
    ]
}
#[test]
#[ignore = "explicit separate CI layout and tape diagnostic"]
fn ci_layout() {
    use std::hint::black_box;
    println!(
        "CI_LAYOUT {}",
        serde_json::json!({
            "op":size_of::<Op>(),"op_align":align_of::<Op>(),"operand":size_of::<Operand>(),
            "dest":size_of::<Dest>(),"small_dest":size_of::<SmallDest>(),"small_dest_op":size_of::<SmallDestOp>(),
            "unit":size_of::<InterpreterUnit>(),"state":size_of::<State>(),"scratch":size_of::<[i64;crate::ir::MAX_NODES]>(),
            "native_frame":size_of::<crate::codegen::Frame>(),"view":size_of::<crate::codegen::View>(),
            "output":size_of::<Output>(),"json_state":size_of::<crate::json_view::State>(),
            "max_locals":MAX_LOCALS,"max_nodes":crate::ir::MAX_NODES,
        })
    );
    let env = environment();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for (name, source, inputs) in cases() {
        let unit =
            Rc::new(InterpreterUnit::prepare_with_environment(source, &inputs, &env).unwrap());
        println!(
            "CI_CASE {}",
            serde_json::json!({"name":name,"source":source,"ops":unit.ops.len(),"capacity":unit.ops.capacity(),
            "prepared_bytes":unit.prepared_bytes(),"live_len":unit.live.len(),"live_capacity":unit.live.capacity(),
            "json_live_len":unit.json_live.len(),"json_live_capacity":unit.json_live.capacity(),"buffers":unit.buffers})
        );
        println!("CI_TAPE {name} {:?}", unit.ops);
        let host = Arc::new(DiagnosticHost);
        let cancel = Cancellation::default();
        let values: Vec<i64> = inputs.iter().map(|_| 7).collect();
        let future = execute_script(
            unit,
            host.clone(),
            &values,
            ScriptLimits::default(),
            cancel.clone(),
            None,
        );
        println!(
            "CI_FUTURE {name} tape {}",
            std::mem::size_of_val(black_box(&future))
        );
        let outcome = rt.block_on(future);
        assert!(outcome.status.is_ok(), "{name}: {:?}", outcome.status);
        let expected = match name {
            "scalar" | "branch" => "8\n",
            "bound_tree" => "55\n",
            "madd_short" => "28\n",
            "quote" => "65280\n",
            "madd_loop" => "98688\n",
            "ready" => "True\nhello error\n",
            "joined_sink" => "[hello error/hello error]\n",
            "async_string" => "7\n",
            _ => unreachable!(),
        };
        assert_eq!(outcome.stdout, expected);
        let code = Rc::new(
            crate::codegen::CodeUnit::compile_with_environment(source, &inputs, &env).unwrap(),
        );
        println!("CI_NATIVE_SIZE {name} {}", code.code_bytes);
        let native =
            runtime::execute_script(code, host, &values, ScriptLimits::default(), cancel, None);
        println!(
            "CI_FUTURE {name} native {}",
            std::mem::size_of_val(black_box(&native))
        );
        let outcome = rt.block_on(native);
        assert!(outcome.status.is_ok(), "{name}: {:?}", outcome.status);
        assert_eq!(outcome.stdout, expected);
        println!("CI_EXECUTED {name} both status=Ok stdout=exact");
    }
}

struct DiagnosticHost;
impl Provider for DiagnosticHost {
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        QuoteBuffer::from_quotes(
            (0..256)
                .map(|i| Quote {
                    unit_price_cents: i,
                    quantity: 2,
                })
                .collect(),
        )
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
    fn allows_tool(&self, id: u32) -> bool {
        id == 101 || id == 102
    }
    async fn call_bound_tool(
        &self,
        id: u32,
        args: crate::tool::ToolArguments,
    ) -> Result<crate::tool::Reply, TaskError> {
        if id == 102 {
            let crate::tool::ToolArguments::String(s) = args else {
                panic!("bound string")
            };
            assert_eq!(s.as_str(), "hello error");
            tokio::task::yield_now().await;
            Ok(crate::tool::Reply::I64(7))
        } else {
            assert_eq!(id, 101);
            Ok(crate::tool::Reply::Json(bytes::Bytes::from_static(
                br#"{"message":"hello error"}"#,
            )))
        }
    }
}
