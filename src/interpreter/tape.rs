//! Sealed linear computation lowering. Scratch values exist only on a synchronous turn.
use super::*;
use crate::ir::MAX_NODES;

#[test]
#[ignore = "explicit serial release preparation/layout diagnostic"]
fn bx_diagnostic() {
    use std::{hint::black_box, time::Instant};
    for (case, source) in [
        ("unmatched", "x=7\nx=x+3\nprint(x)"),
        ("short", "x=7\nx=x*3+x\nprint(x)"),
        (
            "loop",
            "rows=tools.load_quotes(1)\ntotal=0\nfor row in rows:\n    total=total+(row.unit_price_cents+1)*(row.quantity+1)\nprint(total)",
        ),
    ] {
        let unit = InterpreterUnit::prepare_script(source, &[]).unwrap();
        println!(
            "BX layout {case} stride={} ops={} capacity={} prepared_bytes={} unit={} state={} scratch={}",
            std::mem::size_of::<Op>(),
            unit.ops.len(),
            unit.ops.capacity(),
            unit.prepared_bytes(),
            std::mem::size_of::<InterpreterUnit>(),
            std::mem::size_of::<State>(),
            std::mem::size_of::<[i64; MAX_NODES]>()
        );
        println!("BX tape {case} {:?}", unit.ops);
        for batch in 0..9 {
            let start = Instant::now();
            for _ in 0..1000 {
                black_box(InterpreterUnit::prepare_script(black_box(source), &[]).unwrap());
            }
            println!(
                "BX prepare {case} batch={batch} count=1000 ns={}",
                start.elapsed().as_nanos()
            );
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Operand {
    Imm(i64),
    Local(usize),
    Field(Field),
    Temp(usize),
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Dest {
    Local(u8),
    Temp(u16),
}
#[derive(Debug)]
pub(super) struct Count {
    pub(super) buffer: usize,
    field: Field,
    threshold: Operand,
    accumulator: usize,
}
impl Count {
    // One admitted pure shape. No source text/Tool identity or retained IR.
    fn recognize(buffer: usize, body: &[Stmt]) -> Option<Self> {
        // The instruction debugger must retain the original single-op trace.
        if cfg!(feature = "debug-runtime") {
            return None;
        }
        let [Stmt::If(Expr::Binary(BinOp::Gt, field, threshold), yes, no)] = body else {
            return None;
        };
        let Expr::Field(field) = **field else {
            return None;
        };
        let [Stmt::Set(accumulator, Expr::Binary(BinOp::Add, lhs, rhs))] = yes.as_slice() else {
            return None;
        };
        if !no.is_empty()
            || !matches!(**lhs, Expr::Local(i) if i == *accumulator)
            || !matches!(**rhs, Expr::Int(1))
        {
            return None;
        }
        let threshold = match **threshold {
            Expr::Int(v) => Operand::Imm(v),
            Expr::Local(i) if i != *accumulator => Operand::Local(i),
            _ => return None,
        };
        Some(Self {
            buffer,
            field,
            threshold,
            accumulator: *accumulator,
        })
    }
}

// Checked sequential increment preserves the first overflowing row. Inspect the
// actual release machine code instead of assuming LLVM leaves this scalar.
#[inline(never)]
fn scalar_count<const QUANTITY: bool>(
    rows: &[Quote],
    threshold: i64,
    count: &mut i64,
) -> Result<(), usize> {
    for (i, row) in rows.iter().enumerate() {
        let value = if QUANTITY {
            row.quantity
        } else {
            row.unit_price_cents
        };
        if value > threshold {
            let Some(next) = count.checked_add(1) else {
                return Err(i);
            };
            *count = next;
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(super) enum Op {
    Copy(Dest, Operand),
    BufferLen(Dest, usize),
    Abs(Dest, Operand),
    Min(Dest, Operand, Operand),
    Max(Dest, Operand, Operand),
    #[cfg(feature = "tool-json")]
    JsonRead(Dest, usize, usize, bool),
    #[cfg(feature = "tool-json")]
    JsonString(Dest, usize, usize, Option<usize>),
    #[cfg(feature = "tool-json")]
    StringEq(Dest, usize, usize, usize),
    #[cfg(feature = "tool-json")]
    Tool(usize, usize, [u8; 2]),
    Add(Dest, Operand, Operand),
    Sub(Dest, Operand, Operand),
    Mul(Dest, Operand, Operand),
    Eq(Dest, Operand, Operand),
    Ne(Dest, Operand, Operand),
    Lt(Dest, Operand, Operand),
    Le(Dest, Operand, Operand),
    Gt(Dest, Operand, Operand),
    Ge(Dest, Operand, Operand),
    // Comparison consumers branch directly, without writing a temporary bool.
    EqJump(Operand, Operand, usize),
    NeJump(Operand, Operand, usize),
    LtJump(Operand, Operand, usize),
    LeJump(Operand, Operand, usize),
    GtJump(Operand, Operand, usize),
    GeJump(Operand, Operand, usize),
    Branch(Operand, bool, usize),
    Jump(usize),
    Start,
    Count(Count),
    Scan {
        buffer: usize,
        end: usize,
    },
    Next(usize),
    Load(usize, Operand),
    Record(usize, Operand, Operand),
    Print(Operand, Type),
    #[cfg(feature = "tool-json")]
    PrintJsonString(usize, usize),
    #[cfg(feature = "tool-json")]
    PrintString(usize),
    Return,
}
impl Op {
    #[cfg(feature = "debug-runtime")]
    pub(super) fn destination(&self) -> Option<Dest> {
        match self {
            #[cfg(feature = "tool-json")]
            Self::JsonRead(d, ..) => Some(*d),
            #[cfg(feature = "tool-json")]
            Self::JsonString(d, ..) | Self::StringEq(d, ..) => Some(*d),
            Self::BufferLen(d, _)
            | Self::Abs(d, _)
            | Self::Min(d, _, _)
            | Self::Max(d, _, _)
            | Self::Copy(d, _)
            | Self::Add(d, _, _)
            | Self::Sub(d, _, _)
            | Self::Mul(d, _, _)
            | Self::Eq(d, _, _)
            | Self::Ne(d, _, _)
            | Self::Lt(d, _, _)
            | Self::Le(d, _, _)
            | Self::Gt(d, _, _)
            | Self::Ge(d, _, _) => Some(*d),
            _ => None,
        }
    }
    pub(super) fn target(&mut self) -> &mut usize {
        match self {
            Self::EqJump(_, _, t)
            | Self::NeJump(_, _, t)
            | Self::LtJump(_, _, t)
            | Self::LeJump(_, _, t)
            | Self::GtJump(_, _, t)
            | Self::GeJump(_, _, t)
            | Self::Branch(_, _, t)
            | Self::Jump(t) => t,
            _ => unreachable!(),
        }
    }
    pub(super) fn branch_target(&self) -> Option<usize> {
        match self {
            Self::EqJump(_, _, t)
            | Self::NeJump(_, _, t)
            | Self::LtJump(_, _, t)
            | Self::LeJump(_, _, t)
            | Self::GtJump(_, _, t)
            | Self::GeJump(_, _, t)
            | Self::Branch(_, _, t) => Some(*t),
            _ => None,
        }
    }
}
struct Lower<'a> {
    ops: &'a mut Vec<Op>,
    next: usize,
}
impl Lower<'_> {
    fn temp(&mut self) -> Dest {
        let i = self.next;
        self.next += 1;
        // Each scratch slot originates at a bounded admitted expression node.
        assert!(self.next <= MAX_NODES);
        Dest::Temp(u16::try_from(i).expect("admitted temporary index fits u16"))
    }
    fn value(&mut self, e: Expr) -> Operand {
        match e {
            Expr::Int(v) => Operand::Imm(v),
            Expr::Bool(v) => Operand::Imm(i64::from(v)),
            Expr::Local(i) => Operand::Local(i),
            Expr::Field(f) => Operand::Field(f),
            e => {
                let dest = self.temp();
                self.assign(e, dest);
                let Dest::Temp(i) = dest else { unreachable!() };
                Operand::Temp(usize::from(i))
            }
        }
    }
    fn assign(&mut self, e: Expr, dest: Dest) {
        match e {
            Expr::BufferLen(i) => self.ops.push(Op::BufferLen(dest, i)),
            Expr::Abs(e) => {
                let a = self.value(*e);
                self.ops.push(Op::Abs(dest, a));
            }
            Expr::Min(a, b) => {
                let a = self.value(*a);
                let b = self.value(*b);
                self.ops.push(Op::Min(dest, a, b));
            }
            Expr::Max(a, b) => {
                let a = self.value(*a);
                let b = self.value(*b);
                self.ops.push(Op::Max(dest, a, b));
            }
            #[cfg(feature = "tool-json")]
            Expr::JsonI64 { slot, root } | Expr::JsonView { slot, root } => self.ops.push(
                Op::JsonRead(dest, slot, root, matches!(e, Expr::JsonView { .. })),
            ),
            #[cfg(feature = "tool-json")]
            Expr::JsonString { slot, root } => {
                self.ops.push(Op::JsonString(dest, slot, root, None))
            }
            #[cfg(feature = "tool-json")]
            Expr::Contains { slot, root, needle } => {
                self.ops
                    .push(Op::JsonString(dest, slot, root, Some(needle)))
            }
            #[cfg(feature = "tool-json")]
            Expr::StringEq {
                slot,
                root,
                literal,
            } => self.ops.push(Op::StringEq(dest, slot, root, literal)),
            Expr::Binary(op, a, b) if !matches!(op, BinOp::And | BinOp::Or) => {
                let a = self.value(*a);
                let b = self.value(*b);
                self.ops.push(match op {
                    BinOp::Add => Op::Add(dest, a, b),
                    BinOp::Sub => Op::Sub(dest, a, b),
                    BinOp::Mul => Op::Mul(dest, a, b),
                    BinOp::Eq => Op::Eq(dest, a, b),
                    BinOp::Ne => Op::Ne(dest, a, b),
                    BinOp::Lt => Op::Lt(dest, a, b),
                    BinOp::Le => Op::Le(dest, a, b),
                    BinOp::Gt => Op::Gt(dest, a, b),
                    BinOp::Ge => Op::Ge(dest, a, b),
                    _ => unreachable!(),
                });
            }
            e @ (Expr::Binary(..) | Expr::Not(_)) => {
                // Do not write the final local until the entire RHS has been read.
                let false_edges = self.predicate(e, false);
                self.ops.push(Op::Copy(dest, Operand::Imm(1)));
                let jump = self.ops.len();
                self.ops.push(Op::Jump(0));
                self.patch(false_edges);
                self.ops.push(Op::Copy(dest, Operand::Imm(0)));
                *self.ops[jump].target() = self.ops.len();
            }
            e => {
                let src = self.value(e);
                self.ops.push(Op::Copy(dest, src));
            }
        }
    }
    fn patch(&mut self, edges: Vec<usize>) {
        let target = self.ops.len();
        for i in edges {
            *self.ops[i].target() = target;
        }
    }
    fn predicate(&mut self, e: Expr, on: bool) -> Vec<usize> {
        match e {
            Expr::Not(e) => self.predicate(*e, !on),
            Expr::Binary(op @ (BinOp::And | BinOp::Or), a, b) => {
                let short = matches!(op, BinOp::Or);
                if on == short {
                    let mut edges = self.predicate(*a, on);
                    edges.extend(self.predicate(*b, on));
                    edges
                } else {
                    let skip = self.predicate(*a, !on);
                    let edges = self.predicate(*b, on);
                    self.patch(skip);
                    edges
                }
            }
            Expr::Binary(
                op @ (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge),
                a,
                b,
            ) => {
                let a = self.value(*a);
                let b = self.value(*b);
                let op = if on {
                    op
                } else {
                    match op {
                        BinOp::Eq => BinOp::Ne,
                        BinOp::Ne => BinOp::Eq,
                        BinOp::Lt => BinOp::Ge,
                        BinOp::Le => BinOp::Gt,
                        BinOp::Gt => BinOp::Le,
                        BinOp::Ge => BinOp::Lt,
                        _ => unreachable!(),
                    }
                };
                let i = self.ops.len();
                self.ops.push(match op {
                    BinOp::Eq => Op::EqJump(a, b, 0),
                    BinOp::Ne => Op::NeJump(a, b, 0),
                    BinOp::Lt => Op::LtJump(a, b, 0),
                    BinOp::Le => Op::LeJump(a, b, 0),
                    BinOp::Gt => Op::GtJump(a, b, 0),
                    BinOp::Ge => Op::GeJump(a, b, 0),
                    _ => unreachable!(),
                });
                vec![i]
            }
            e => {
                let a = self.value(e);
                let i = self.ops.len();
                self.ops.push(Op::Branch(a, on, 0));
                vec![i]
            }
        }
    }
    fn statements(&mut self, statements: Vec<Stmt>) {
        for stmt in statements {
            self.next = 0;
            match stmt {
                Stmt::Set(i, e) => self.assign(
                    e,
                    Dest::Local(u8::try_from(i).expect("admitted local index fits u8")),
                ),
                Stmt::Eval(e) => {
                    self.value(e);
                }
                Stmt::Return(e) => {
                    self.value(e);
                    self.ops.push(Op::Return);
                }
                Stmt::Print(e, t) => {
                    let e = self.value(e);
                    self.ops.push(Op::Print(e, t));
                }
                #[cfg(feature = "tool-json")]
                Stmt::PrintJsonString { slot, root } => {
                    self.ops.push(Op::PrintJsonString(slot, root));
                }
                #[cfg(feature = "tool-json")]
                Stmt::PrintString { plan, .. } => self.ops.push(Op::PrintString(plan)),
                Stmt::Load(i, e) => {
                    let e = self.value(e);
                    self.ops.push(Op::Load(i, e));
                }
                #[cfg(feature = "tool-json")]
                Stmt::Tool {
                    call, target, args, ..
                } => {
                    let mut slots = [MAX_LOCALS as u8; 2];
                    for (i, e) in args.into_iter().enumerate() {
                        let Expr::Local(slot) = e else {
                            unreachable!("frontend binds arguments after source-order evaluation")
                        };
                        slots[i] = slot as u8;
                    }
                    self.ops.push(Op::Tool(call, target, slots));
                }
                Stmt::Record(i, a, b) => {
                    let a = self.value(a);
                    let b = self.value(b);
                    self.ops.push(Op::Record(i, a, b));
                }
                Stmt::If(e, yes, no) => {
                    let edges = self.predicate(e, false);
                    self.statements(yes);
                    let jump = self.ops.len();
                    self.ops.push(Op::Jump(0));
                    self.patch(edges);
                    self.statements(no);
                    *self.ops[jump].target() = self.ops.len();
                }
                Stmt::For(buffer, body) => {
                    self.ops.push(Op::Start);
                    if let Some(count) = Count::recognize(buffer, &body) {
                        self.ops.push(Op::Count(count));
                        continue;
                    }
                    let header = self.ops.len();
                    self.ops.push(Op::Scan { buffer, end: 0 });
                    self.statements(body);
                    self.ops.push(Op::Next(header));
                    let end = self.ops.len();
                    self.ops[header] = Op::Scan { buffer, end };
                }
            }
        }
    }
}
pub(super) fn flatten(statements: Vec<Stmt>, ops: &mut Vec<Op>) {
    Lower { ops, next: 0 }.statements(statements);
}

#[test]
fn count_admission_and_layout() {
    assert_eq!(std::mem::size_of::<Op>(), 48);
    assert_eq!(std::mem::size_of::<Count>(), 40);
    for (body, matched) in [
        ("if row.quantity>threshold:\n        count=count+1", true),
        ("if row.unit_price_cents>0:\n        count=count+1", true),
        ("if row.quantity>count:\n        count=count+1", false),
        ("if row.quantity>threshold:\n        count=count+2", false),
        ("if row.quantity+1>threshold:\n        count=count+1", false),
        (
            "if row.quantity>threshold:\n        count=count+1\n    threshold=threshold+1",
            false,
        ),
        (
            "if row.quantity>threshold:\n        count=count+1\n    else:\n        count=count-1",
            false,
        ),
    ] {
        let source = format!(
            "rows=tools.load_quotes(1)\nthreshold=0\ncount=0\nfor row in rows:\n    {body}\nprint(count)"
        );
        let unit = InterpreterUnit::prepare_script(&source, &[]).unwrap();
        let hits = unit
            .ops
            .iter()
            .filter(|op| matches!(op, Op::Count(_)))
            .count();
        assert_eq!(
            hits,
            usize::from(matched && !cfg!(feature = "debug-runtime"))
        );
    }
}

pub(super) enum Boundary {
    Load(usize, i64),
    Record(usize, i64, i64),
    #[cfg(feature = "tool-json")]
    Tool(usize, usize, [i64; 2]),
    Yield,
    Done,
    #[cfg(feature = "debug-runtime")]
    Step,
}
#[cfg(feature = "tool-json")]
pub(super) fn json_liveness(ops: &[Op], plan: &crate::tool::Plan) -> Vec<u32> {
    let mut live = vec![0; ops.len() + 1];
    // As for buffer owners, forward-only tape is solved by the first pass.
    let mut backward = false;
    loop {
        let mut changed = false;
        for pc in (0..ops.len()).rev() {
            #[cfg(test)]
            liveness_tests::visit(true);
            let next = live[pc + 1];
            let before = match &ops[pc] {
                op if op.branch_target().is_some() => {
                    let target = op.branch_target().unwrap();
                    backward |= target <= pc;
                    next | live[target]
                }
                Op::Jump(t) | Op::Next(t) => {
                    backward |= *t <= pc;
                    live[*t]
                }
                Op::Scan { end, .. } => {
                    backward |= *end <= pc;
                    next | live[*end]
                }
                Op::JsonRead(_, _, root, _) => next | (1 << root),
                Op::JsonString(_, _, root, _) | Op::StringEq(_, _, root, _) => next | (1 << root),
                Op::PrintJsonString(_, root) => next | (1 << root),
                Op::PrintString(index) => next | plan.prints[*index].roots(),
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
        if !changed || !backward {
            break;
        }
    }
    live
}
#[inline]
fn read(a: Operand, state: &State, temps: &[i64; MAX_NODES]) -> i64 {
    match a {
        Operand::Imm(v) => v,
        Operand::Local(i) => state.locals[i],
        Operand::Temp(i) => temps[i],
        Operand::Field(f) => {
            let row = state.row.as_ref().expect("sealed fields inside scans");
            match f {
                Field::Price => row.unit_price_cents,
                Field::Quantity => row.quantity,
            }
        }
    }
}
#[inline]
fn write(dest: Dest, v: i64, state: &mut State, temps: &mut [i64; MAX_NODES]) {
    match dest {
        Dest::Local(i) => state.locals[usize::from(i)] = v,
        Dest::Temp(i) => temps[usize::from(i)] = v,
    }
}
// This is deliberately synchronous: no scratch array is captured in a waiting future.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn run_until_boundary(
    unit: &InterpreterUnit,
    state: &mut State,
    owners: &[Option<QuoteBuffer>],
    limits: &runtime::Limits,
    output: &mut Output,
) -> Result<Boundary, TaskError> {
    let mut temps = [0; MAX_NODES];
    run::<false>(unit, state, owners, limits, output, &mut temps)
}
// Constant specialization removes step accounting from production dispatch.
pub(super) fn run<const STEP: bool>(
    unit: &InterpreterUnit,
    state: &mut State,
    owners: &[Option<QuoteBuffer>],
    limits: &runtime::Limits,
    output: &mut Output,
    temps: &mut [i64; MAX_NODES],
) -> Result<Boundary, TaskError> {
    while state.pc < unit.ops.len() {
        let op = &unit.ops[state.pc];
        state.pc += 1;
        #[cfg(feature = "interpreter-profile")]
        crate::interpreter_profile::update(|c| c.tape_dispatches += 1);
        macro_rules! calc {
            ($d:expr,$a:expr,$b:expr,$method:ident) => {{
                let a = read(*$a, state, temps);
                let b = read(*$b, state, temps);
                let v = a.$method(b).ok_or(TaskError::Overflow)?;
                write(*$d, v, state, temps);
            }};
        }
        macro_rules! cmp { ($d:expr,$a:expr,$b:expr,$operator:tt) => {{ let v=i64::from(read(*$a,state,temps) $operator read(*$b,state,temps)); write(*$d,v,state,temps); }} }
        macro_rules! jump { ($a:expr,$b:expr,$target:expr,$operator:tt) => { if read(*$a,state,temps) $operator read(*$b,state,temps) {state.pc=*$target;} } }
        match op {
            Op::BufferLen(d, i) => write(
                *d,
                owners[*i]
                    .as_ref()
                    .expect("sealed buffer owner")
                    .rows()
                    .len() as i64,
                state,
                temps,
            ),
            Op::Abs(d, a) => {
                let v = read(*a, state, temps)
                    .checked_abs()
                    .ok_or(TaskError::Overflow)?;
                write(*d, v, state, temps);
            }
            Op::Min(d, a, b) => {
                let v = read(*a, state, temps).min(read(*b, state, temps));
                write(*d, v, state, temps);
            }
            Op::Max(d, a, b) => {
                let v = read(*a, state, temps).max(read(*b, state, temps));
                write(*d, v, state, temps);
            }
            #[cfg(feature = "tool-json")]
            Op::JsonRead(d, slot, _, view) => {
                let json = output.json.as_mut().expect("sealed JSON plan");
                let v = if *view {
                    json.view(*slot)?
                } else {
                    json.integer(*slot)?
                };
                write(*d, v, state, temps);
            }
            #[cfg(feature = "tool-json")]
            Op::JsonString(d, slot, _, needle) => {
                let json = output.json.as_mut().expect("sealed JSON plan");
                let v = if let Some(needle) = needle {
                    json.contains(*slot, *needle)?
                } else {
                    json.validate_string(*slot)?
                };
                write(*d, v, state, temps);
            }
            #[cfg(feature = "tool-json")]
            Op::StringEq(d, slot, _, literal) => {
                let v = output
                    .json
                    .as_mut()
                    .expect("sealed JSON plan")
                    .equals(*slot, *literal)?;
                write(*d, v, state, temps);
            }
            #[cfg(feature = "tool-json")]
            Op::Tool(call, target, args) => {
                return Ok(Boundary::Tool(
                    *call,
                    *target,
                    args.map(|i| {
                        if (i as usize) < MAX_LOCALS {
                            state.locals[i as usize]
                        } else {
                            0
                        }
                    }),
                ));
            }
            Op::Copy(d, a) => {
                let v = read(*a, state, temps);
                write(*d, v, state, temps);
            }
            Op::Add(d, a, b) => calc!(d, a, b, checked_add),
            Op::Sub(d, a, b) => calc!(d, a, b, checked_sub),
            Op::Mul(d, a, b) => calc!(d, a, b, checked_mul),
            Op::Eq(d, a, b) => cmp!(d,a,b,==),
            Op::Ne(d, a, b) => cmp!(d,a,b,!=),
            Op::Lt(d, a, b) => cmp!(d,a,b,<),
            Op::Le(d, a, b) => cmp!(d,a,b,<=),
            Op::Gt(d, a, b) => cmp!(d,a,b,>),
            Op::Ge(d, a, b) => cmp!(d,a,b,>=),
            Op::EqJump(a, b, t) => jump!(a,b,t,==),
            Op::NeJump(a, b, t) => jump!(a,b,t,!=),
            Op::LtJump(a, b, t) => jump!(a,b,t,<),
            Op::LeJump(a, b, t) => jump!(a,b,t,<=),
            Op::GtJump(a, b, t) => jump!(a,b,t,>),
            Op::GeJump(a, b, t) => jump!(a,b,t,>=),
            Op::Branch(a, on, t) => {
                if (read(*a, state, temps) != 0) == *on {
                    state.pc = *t;
                }
            }
            Op::Jump(t) => state.pc = *t,
            Op::Print(a, t) => output.append(read(*a, state, temps), *t == Type::Bool)?,
            #[cfg(feature = "tool-json")]
            Op::PrintJsonString(slot, _) => output.append_json_string(*slot)?,
            #[cfg(feature = "tool-json")]
            Op::PrintString(plan) => output
                .append_string_source(&unit.json.as_ref().unwrap().prints[*plan], &state.locals)?,
            Op::Return => return Ok(Boundary::Done),
            Op::Load(i, a) => return Ok(Boundary::Load(*i, read(*a, state, temps))),
            Op::Record(i, a, b) => {
                return Ok(Boundary::Record(
                    *i,
                    read(*a, state, temps),
                    read(*b, state, temps),
                ));
            }
            Op::Start => {
                state.index = 0;
                state.chunk_left = 0;
            }
            Op::Count(count) => {
                let rows = owners[count.buffer]
                    .as_ref()
                    .expect("sealed count owner")
                    .rows();
                if state.index == rows.len() {
                    state.row = None;
                    continue;
                }
                #[cfg(feature = "interpreter-profile")]
                crate::interpreter_profile::update(|c| c.fuel_reservations += 1);
                if state.remaining == 0 {
                    return Err(TaskError::Budget);
                }
                let length = (rows.len() - state.index)
                    .min(limits.chunk)
                    .min(state.remaining as usize);
                state.remaining -= length as u64;
                state.chunk_left = length;
                let threshold = read(count.threshold, state, temps);
                let chunk = &rows[state.index..state.index + length];
                let result = match count.field {
                    Field::Price => scalar_count::<false>(
                        chunk,
                        threshold,
                        &mut state.locals[count.accumulator],
                    ),
                    Field::Quantity => {
                        scalar_count::<true>(chunk, threshold, &mut state.locals[count.accumulator])
                    }
                };
                if let Err(completed) = result {
                    state.index += completed;
                    state.chunk_left -= completed;
                    state.row = Some(rows[state.index]);
                    return Err(TaskError::Overflow);
                }
                state.index += length;
                state.chunk_left = 0;
                state.row = Some(rows[state.index - 1]);
                // Resume at this descriptor, using the ordinary fairness/cancel path.
                state.pc -= 1;
                if state.index < rows.len() {
                    return Ok(Boundary::Yield);
                }
            }
            Op::Scan { buffer, end } => {
                let rows = owners[*buffer].as_ref().expect("sealed scan owner").rows();
                if state.index == rows.len() {
                    state.row = None;
                    state.pc = *end;
                    #[cfg(feature = "debug-runtime")]
                    if STEP {
                        return Ok(Boundary::Step);
                    }
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
                    return Ok(Boundary::Yield);
                }
            }
        }
        #[cfg(feature = "debug-runtime")]
        if STEP {
            return Ok(Boundary::Step);
        }
    }
    Ok(Boundary::Done)
}
