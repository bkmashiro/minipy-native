//! Explicit lightweight backend: bound typed expressions plus a flat resumable statement stream.
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

#[derive(Debug)]
enum Op {
    Set(usize, Expr),
    Branch(Expr, usize),
    Jump(usize),
    Start,
    Scan { buffer: usize, end: usize },
    Next(usize),
    Load(usize, Expr),
    Record(usize, Expr, Expr),
    Print(Expr, Type),
    Eval(Expr),
    Return(Expr),
}
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
                    Op::Branch(_, target) => next | live[*target],
                    Op::Jump(target) | Op::Next(target) => live[*target],
                    Op::Scan { buffer, end } => next | live[*end] | (1 << buffer),
                    Op::Load(target, _) => next & !(1 << target),
                    Op::Return(_) => 0,
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
        fn expr_bytes(e: &Expr) -> usize {
            match e {
                Expr::Binary(_, a, b) => {
                    2 * std::mem::size_of::<Expr>() + expr_bytes(a) + expr_bytes(b)
                }
                Expr::Not(e) => std::mem::size_of::<Expr>() + expr_bytes(e),
                _ => 0,
            }
        }
        std::mem::size_of::<Self>()
            + self.ops.capacity() * std::mem::size_of::<Op>()
            + self.live.capacity() * 8
            + self.inputs.capacity() * std::mem::size_of::<Type>()
            + self
                .ops
                .iter()
                .map(|op| match op {
                    Op::Set(_, e)
                    | Op::Branch(e, _)
                    | Op::Load(_, e)
                    | Op::Print(e, _)
                    | Op::Eval(e)
                    | Op::Return(e) => expr_bytes(e),
                    Op::Record(_, a, b) => expr_bytes(a) + expr_bytes(b),
                    _ => 0,
                })
                .sum::<usize>()
    }
}
fn flatten(statements: Vec<Stmt>, ops: &mut Vec<Op>) {
    for stmt in statements {
        match stmt {
            Stmt::Set(i, e) => ops.push(Op::Set(i, e)),
            Stmt::Load(i, e) => ops.push(Op::Load(i, e)),
            Stmt::Record(i, a, b) => ops.push(Op::Record(i, a, b)),
            Stmt::Print(e, ty) => ops.push(Op::Print(e, ty)),
            Stmt::Eval(e) => ops.push(Op::Eval(e)),
            Stmt::Return(e) => ops.push(Op::Return(e)),
            Stmt::If(e, yes, no) => {
                let branch = ops.len();
                ops.push(Op::Branch(e, 0));
                flatten(yes, ops);
                let jump = ops.len();
                ops.push(Op::Jump(0));
                let no_start = ops.len();
                flatten(no, ops);
                let end = ops.len();
                if let Op::Branch(_, target) = &mut ops[branch] {
                    *target = no_start;
                }
                ops[jump] = Op::Jump(end);
            }
            Stmt::For(buffer, body) => {
                ops.push(Op::Start);
                let header = ops.len();
                ops.push(Op::Scan { buffer, end: 0 });
                flatten(body, ops);
                ops.push(Op::Next(header));
                let end = ops.len();
                ops[header] = Op::Scan { buffer, end };
            }
        }
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
impl State {
    // Avoid a recursive eval call/prologue for sealed typed leaf operands.
    // Compound operands still evaluate exactly once, left-to-right.
    #[inline]
    fn operand(&self, e: &Expr) -> Result<i64, TaskError> {
        #[cfg(feature = "interpreter-profile")]
        if matches!(
            e,
            Expr::Int(_) | Expr::Bool(_) | Expr::Local(_) | Expr::Field(_)
        ) {
            crate::interpreter_profile::update(|c| c.leaf_operands += 1);
        }
        Ok(match e {
            Expr::Int(v) => *v,
            Expr::Bool(v) => i64::from(*v),
            Expr::Local(i) => self.locals[*i],
            Expr::Field(f) => {
                let row = self
                    .row
                    .as_ref()
                    .expect("sealed frontend binds fields inside scans");
                match f {
                    Field::Price => row.unit_price_cents,
                    Field::Quantity => row.quantity,
                }
            }
            _ => return self.eval(e),
        })
    }
    fn eval(&self, e: &Expr) -> Result<i64, TaskError> {
        #[cfg(feature = "interpreter-profile")]
        crate::interpreter_profile::update(|c| {
            c.expr[match e {
                Expr::Int(_) => 0,
                Expr::Bool(_) => 1,
                Expr::Local(_) => 2,
                Expr::Field(_) => 3,
                Expr::Not(_) => 4,
                Expr::Binary(..) => 5,
            }] += 1
        });
        Ok(match e {
            Expr::Int(v) => *v,
            Expr::Bool(v) => i64::from(*v),
            Expr::Local(i) => self.locals[*i],
            Expr::Field(f) => {
                let row = self
                    .row
                    .as_ref()
                    .expect("sealed frontend binds fields inside scans");
                match f {
                    Field::Price => row.unit_price_cents,
                    Field::Quantity => row.quantity,
                }
            }
            Expr::Not(e) => i64::from(self.operand(e)? == 0),
            Expr::Binary(op, a, b) => {
                let a = self.operand(a)?;
                if matches!(op, BinOp::And) && a == 0 {
                    #[cfg(feature = "interpreter-profile")]
                    crate::interpreter_profile::update(|c| c.expr[6] += 1);
                    return Ok(0);
                }
                if matches!(op, BinOp::Or) && a != 0 {
                    #[cfg(feature = "interpreter-profile")]
                    crate::interpreter_profile::update(|c| c.expr[6] += 1);
                    return Ok(1);
                }
                let b = self.operand(b)?;
                match op {
                    BinOp::Add => a.checked_add(b).ok_or(TaskError::Overflow)?,
                    BinOp::Sub => a.checked_sub(b).ok_or(TaskError::Overflow)?,
                    BinOp::Mul => a.checked_mul(b).ok_or(TaskError::Overflow)?,
                    BinOp::Eq => i64::from(a == b),
                    BinOp::Ne => i64::from(a != b),
                    BinOp::Lt => i64::from(a < b),
                    BinOp::Le => i64::from(a <= b),
                    BinOp::Gt => i64::from(a > b),
                    BinOp::Ge => i64::from(a >= b),
                    BinOp::And | BinOp::Or => i64::from(b != 0),
                }
            }
        })
    }
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
    while state.pc < unit.ops.len() {
        let op = &unit.ops[state.pc];
        #[cfg(feature = "interpreter-profile")]
        crate::interpreter_profile::update(|c| {
            c.ops[match op {
                Op::Set(..) => 0,
                Op::Eval(..) => 1,
                Op::Return(..) => 2,
                Op::Print(..) => 3,
                Op::Branch(..) => 4,
                Op::Jump(..) => 5,
                Op::Start => 6,
                Op::Scan { .. } => 7,
                Op::Next(..) => 8,
                Op::Load(..) | Op::Record(..) => 9,
            }] += 1
        });
        state.pc += 1;
        match op {
            Op::Set(i, e) => state.locals[*i] = state.eval(e)?,
            Op::Eval(e) => {
                state.eval(e)?;
            }
            Op::Return(e) => {
                state.eval(e)?;
                break;
            }
            Op::Print(e, ty) => output.append(state.eval(e)?, *ty == Type::Bool)?,
            Op::Branch(e, no) => {
                if state.eval(e)? == 0 {
                    state.pc = *no;
                }
            }
            Op::Jump(pc) => state.pc = *pc,
            Op::Start => {
                state.index = 0;
                state.chunk_left = 0;
            }
            Op::Scan { buffer, end } => {
                let rows = owners[*buffer]
                    .as_ref()
                    .expect("sealed frontend initializes scan owner")
                    .rows();
                if state.index == rows.len() {
                    state.row = None;
                    state.pc = *end;
                    continue;
                }
                if state.chunk_left == 0 {
                    #[cfg(feature = "interpreter-profile")]
                    crate::interpreter_profile::update(|c| c.fuel_reservations += 1);
                    if state.remaining == 0 {
                        return Err(TaskError::Budget);
                    }
                    let count = (rows.len() - state.index)
                        .min(limits.chunk)
                        .min(state.remaining as usize);
                    state.remaining -= count as u64;
                    state.chunk_left = count;
                }
                state.row = Some(rows[state.index]);
            }
            Op::Next(header) => {
                state.index += 1;
                state.chunk_left -= 1;
                state.pc = *header;
                let Op::Scan { buffer, .. } = unit.ops[*header] else {
                    unreachable!()
                };
                if state.chunk_left == 0
                    && state.index < owners[buffer].as_ref().unwrap().rows().len()
                {
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
            }
            Op::Load(target, batch) => {
                let batch = state.eval(batch)?;
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
                    owners[*target] = Some(quotes);
                }
            }
            Op::Record(target, batch, total) => {
                let batch = state.eval(batch)?;
                let total = state.eval(total)?;
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
                state.locals[*target] = runtime::receive(rx, &cancel).await?;
            }
        }
    }
    runtime::event(&trace, "done", 0, 0, state.remaining, state.index as u64);
    Ok(())
}
