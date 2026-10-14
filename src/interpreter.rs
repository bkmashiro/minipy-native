//! Explicit lightweight backend: sealed computation tape and resumable bounded turns.
//! Preparation never constructs a CodeUnit or calls Cranelift. Prepared units are immutable.
use crate::{
    buffer::{Quote, QuoteBuffer},
    error::TaskError,
    frontend,
    ir::{BinOp, CompileError, Expr, Field, MAX_LOCALS, Stmt, Type},
    provider::Provider,
    runtime::{self, Cancellation, Output, ScriptLimits, ScriptOutcome, Trace},
};
use std::{rc::Rc, sync::Arc};
use tokio::sync::oneshot;
mod tape;
use tape::{Boundary, Op, flatten};
/// Only the existing bounded frontend can construct this sealed executable.
pub struct InterpreterUnit {
    ops: Vec<Op>,
    live: Vec<u64>,
    inputs: Vec<Type>,
    buffers: usize,
}
impl InterpreterUnit {
    pub fn prepare_script(source: &str, inputs: &[(&str, Type)]) -> Result<Self, CompileError> {
        let program = frontend::parse_with_inputs(source, inputs)?;
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
        Ok(Self {
            ops,
            live,
            inputs: program.locals[..program.inputs].to_vec(),
            buffers: program.buffers,
        })
    }
    /// Logical owned preparation bytes, excluding allocator rounding and Rc header.
    pub fn prepared_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
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
        match tape::run_until_boundary(unit, &mut state, &owners, &limits, output)? {
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
                if quotes.bytes() > limits.buffer_bytes.saturating_sub(held) {
                    return Err(TaskError::Budget);
                }
                if unit.live[state.pc] & (1 << target) != 0 {
                    owners[target] = Some(quotes);
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
