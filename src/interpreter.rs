//! Explicit lightweight backend: sealed computation tape and resumable bounded turns.
//! Preparation never constructs a CodeUnit or calls Cranelift. Prepared units are immutable.
#[cfg(not(target_arch = "wasm32"))]
use crate::provider::Provider;
#[cfg(not(target_arch = "wasm32"))]
use crate::runtime::{ScriptOutcome, Trace};
use crate::{
    buffer::{Quote, QuoteBuffer},
    error::TaskError,
    frontend,
    ir::{BinOp, CompileError, Expr, Field, MAX_LOCALS, Stmt, Type},
    runtime::{self, Cancellation, Output, ScriptLimits},
};
use std::rc::Rc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::oneshot;
#[cfg(feature = "debug-runtime")]
pub mod debug;
mod tape;
use tape::{Boundary, Op, flatten};
/// Only the existing bounded frontend can construct this sealed executable.
pub struct InterpreterUnit {
    ops: Vec<Op>,
    live: Vec<u64>,
    inputs: Vec<Type>,
    buffers: usize,
    #[cfg(feature = "tool-json")]
    json: Option<Rc<crate::tool::Plan>>,
    #[cfg(feature = "tool-json")]
    json_live: Vec<u32>,
}
impl InterpreterUnit {
    pub fn prepare_script(source: &str, inputs: &[(&str, Type)]) -> Result<Self, CompileError> {
        let program = frontend::parse_with_inputs(source, inputs)?;
        Self::from_program(program)
    }
    #[cfg(feature = "tool-json")]
    pub fn prepare_with_tools(
        source: &str,
        inputs: &[(&str, Type)],
        tools: &[crate::tool::ToolSchema],
    ) -> Result<Self, CompileError> {
        Self::from_program(frontend::parse_with_tools(source, inputs, tools)?)
    }
    fn from_program(program: crate::ir::Program) -> Result<Self, CompileError> {
        let mut ops = Vec::new();
        flatten(program.statements, &mut ops);
        // Buffer liveness at each PC, including loop backedges. No extra scalar
        // analysis is needed: slots already reside in the task's fixed local array.
        let mut live = vec![0; ops.len() + 1];
        loop {
            let mut changed = false;
            for pc in (0..ops.len()).rev() {
                let next = live[pc + 1];
                let before = match &ops[pc] {
                    op if op.branch_target().is_some() => next | live[op.branch_target().unwrap()],
                    Op::Jump(target) | Op::Next(target) => live[*target],
                    Op::Scan { buffer, end } => next | live[*end] | (1 << buffer),
                    Op::Load(target, _) => next & !(1 << target),
                    Op::Return => 0,
                    _ => next,
                };
                changed |= before != live[pc];
                live[pc] = before;
            }
            if !changed {
                break;
            }
        }
        #[cfg(feature = "tool-json")]
        let json_live = if program.json.roots > 0 {
            tape::json_liveness(&ops)
        } else {
            Vec::new()
        };
        Ok(Self {
            #[cfg(feature = "tool-json")]
            json: (!program.json.calls.is_empty()).then(|| Rc::new(program.json)),
            #[cfg(feature = "tool-json")]
            json_live,
            ops,
            live,
            inputs: program.locals[..program.inputs].to_vec(),
            buffers: program.buffers,
        })
    }
    /// Logical owned preparation bytes, excluding allocator rounding and Rc header.
    pub fn prepared_bytes(&self) -> usize {
        #[cfg(feature = "tool-json")]
        let json_bytes =
            self.json.as_ref().map_or(0, |p| p.owned_bytes()) + self.json_live.capacity() * 4;
        #[cfg(not(feature = "tool-json"))]
        let json_bytes = 0;
        json_bytes
            + std::mem::size_of::<Self>()
            + self.ops.capacity() * std::mem::size_of::<Op>()
            + self.live.capacity() * 8
            + self.inputs.capacity() * std::mem::size_of::<Type>()
    }
}
struct State {
    locals: [i64; MAX_LOCALS],
    pc: usize,
    index: usize,
    chunk_left: usize,
    remaining: u64,
    row: Option<Quote>,
}

fn release(owners: &mut [Option<QuoteBuffer>], live: u64) -> usize {
    let mut held = 0;
    for (i, owner) in owners.iter_mut().enumerate() {
        if live & (1 << i) == 0 {
            *owner = None;
        }
        if let Some(owner) = owner {
            held += owner.bytes();
        }
    }
    held
}
/// Same script outcome, limits, typed provider and detached-operation cancellation
/// contract as native execution. One task, no OS thread or per-row object tree.
#[cfg(not(target_arch = "wasm32"))]
pub async fn execute_script<P: Provider>(
    unit: Rc<InterpreterUnit>,
    provider: Arc<P>,
    inputs: &[i64],
    options: ScriptLimits,
    cancel: Cancellation,
    trace: Option<Trace>,
) -> ScriptOutcome {
    let mut output = Output::new(options.stdout_bytes, options.allow_print);
    let status = run(&unit, provider, inputs, options, cancel, trace, &mut output).await;
    ScriptOutcome {
        status,
        stdout: output.stdout,
    }
}
#[cfg(not(target_arch = "wasm32"))]
async fn run<P: Provider>(
    unit: &InterpreterUnit,
    provider: Arc<P>,
    inputs: &[i64],
    options: ScriptLimits,
    cancel: Cancellation,
    trace: Option<Trace>,
    output: &mut Output,
) -> Result<(), TaskError> {
    runtime::check(&cancel)?;
    if inputs.len() != unit.inputs.len()
        || inputs
            .iter()
            .zip(&unit.inputs)
            .any(|(v, t)| *t == Type::Bool && ![0, 1].contains(v))
    {
        return Err(TaskError::Input);
    }
    #[cfg(feature = "tool-json")]
    if let Some(plan) = &unit.json
        && plan.roots > 0
    {
        output.json = Some(Box::new(crate::json_view::State::new(plan.clone())));
    }
    let limits = options.limits;
    if limits.chunk == 0 || limits.chunk > 4096 {
        return Err(TaskError::Budget);
    }
    let mut state = State {
        locals: [0; MAX_LOCALS],
        pc: 0,
        index: 0,
        chunk_left: 0,
        remaining: limits.rows,
        row: None,
    };
    state.locals[..inputs.len()].copy_from_slice(inputs);
    let mut owners: Vec<Option<QuoteBuffer>> = (0..unit.buffers).map(|_| None).collect();
    let mut calls = options.tool_calls;
    // Cancellation is checked at the same bounded turns as native: entry, Tool,
    // and chunk yield. Scalar dispatch remains bounded by frontend admission.
    loop {
        let boundary = tape::run_until_boundary(unit, &mut state, &owners, &limits, output)?;
        #[cfg(feature = "tool-json")]
        if let Some(json) = &mut output.json {
            json.release(unit.json_live[state.pc]);
        }
        match boundary {
            #[cfg(feature = "debug-runtime")]
            Boundary::Step => unreachable!("production specialization never stops per instruction"),
            Boundary::Done => break,
            Boundary::Yield => {
                let held = release(&mut owners, unit.live[state.pc]);
                #[cfg(feature = "interpreter-profile")]
                crate::interpreter_profile::update(|c| c.yields += 1);
                runtime::event(
                    &trace,
                    "yield",
                    held,
                    std::mem::size_of::<State>(),
                    state.remaining,
                    state.index as u64,
                );
                tokio::task::yield_now().await;
                runtime::check(&cancel)?;
            }
            Boundary::Load(target, batch) => {
                let held = release(&mut owners, unit.live[state.pc]);
                runtime::authorize_tool(&cancel, limits.allow_load, &mut calls)?;
                runtime::event(
                    &trace,
                    "load_wait",
                    held,
                    std::mem::size_of::<State>(),
                    state.remaining,
                    state.index as u64,
                );
                let p = provider.clone();
                let (tx, rx) = oneshot::channel();
                tokio::spawn(async move {
                    let _ = tx.send(p.load_quotes(batch).await);
                });
                let quotes = runtime::receive(rx, &cancel).await?;
                #[cfg(feature = "tool-json")]
                let held = held + output.json.as_ref().map_or(0, |j| j.held());
                if quotes.bytes() > limits.buffer_bytes.saturating_sub(held) {
                    return Err(TaskError::Budget);
                }
                if unit.live[state.pc] & (1 << target) != 0 {
                    owners[target] = Some(quotes);
                }
            }
            #[cfg(feature = "tool-json")]
            Boundary::Tool(call, target, args) => {
                let held = release(&mut owners, unit.live[state.pc]);
                let call = &unit.json.as_ref().expect("sealed Tool plan").calls[call];
                runtime::authorize_tool(&cancel, provider.allows_tool(call.id), &mut calls)?;
                let (id, count, is_json) = (call.id, call.arguments, call.json);
                let p = provider.clone();
                let (tx, rx) = oneshot::channel();
                tokio::spawn(async move {
                    let _ = tx.send(p.call_tool(id, args, count).await);
                });
                match (is_json, runtime::receive(rx, &cancel).await?) {
                    (true, crate::tool::Reply::Json(bytes)) => {
                        let json = output.json.as_mut().expect("sealed JSON plan");
                        json.install(target, bytes, limits.buffer_bytes.saturating_sub(held))?;
                        json.release(unit.json_live[state.pc]);
                    }
                    (false, crate::tool::Reply::I64(v)) => state.locals[target] = v,
                    _ => return Err(TaskError::Json("Tool reply does not match declaration")),
                }
            }
            Boundary::Record(target, batch, total) => {
                let held = release(&mut owners, unit.live[state.pc]);
                runtime::authorize_tool(&cancel, limits.allow_record, &mut calls)?;
                runtime::event(
                    &trace,
                    "record_wait",
                    held,
                    std::mem::size_of::<State>(),
                    state.remaining,
                    state.index as u64,
                );
                let p = provider.clone();
                let (tx, rx) = oneshot::channel();
                tokio::spawn(async move {
                    let _ = tx.send(p.record_total(batch, total).await);
                });
                state.locals[target] = runtime::receive(rx, &cancel).await?;
            }
        }
    }
    runtime::event(&trace, "done", 0, 0, state.remaining, state.index as u64);
    Ok(())
}
