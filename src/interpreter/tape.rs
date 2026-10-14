//! Sealed linear computation lowering. Scratch values exist only on a synchronous turn.
use super::*;
use crate::ir::MAX_NODES;

#[derive(Clone, Copy, Debug)]
pub(super) enum Operand {
    Imm(i64),
    Local(usize),
    Field(Field),
    Temp(usize),
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Dest {
    Local(usize),
    Temp(usize),
}
#[derive(Debug)]
pub(super) enum Op {
    Copy(Dest, Operand),
    #[cfg(feature = "tool-json")]
    JsonRead(Dest, usize, usize, bool),
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
    Scan {
        buffer: usize,
        end: usize,
    },
    Next(usize),
    Load(usize, Operand),
    Record(usize, Operand, Operand),
    Print(Operand, Type),
    Return,
}
impl Op {
    #[cfg(feature = "debug-runtime")]
    pub(super) fn destination(&self) -> Option<Dest> {
        match self {
            #[cfg(feature = "tool-json")]
            Self::JsonRead(d, ..) => Some(*d),
            Self::Copy(d, _)
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
        Dest::Temp(i)
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
                Operand::Temp(i)
            }
        }
    }
    fn assign(&mut self, e: Expr, dest: Dest) {
        match e {
            #[cfg(feature = "tool-json")]
            Expr::JsonI64 { slot, root } | Expr::JsonView { slot, root } => self.ops.push(
                Op::JsonRead(dest, slot, root, matches!(e, Expr::JsonView { .. })),
            ),
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
                Stmt::Set(i, e) => self.assign(e, Dest::Local(i)),
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
pub(super) fn json_liveness(ops: &[Op]) -> Vec<u32> {
    let mut live = vec![0; ops.len() + 1];
    loop {
        let mut changed = false;
        for pc in (0..ops.len()).rev() {
            let next = live[pc + 1];
            let before = match &ops[pc] {
                op if op.branch_target().is_some() => next | live[op.branch_target().unwrap()],
                Op::Jump(t) | Op::Next(t) => live[*t],
                Op::Scan { end, .. } => next | live[*end],
                Op::JsonRead(_, _, root, _) => next | (1 << root),
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
        Dest::Local(i) => state.locals[i] = v,
        Dest::Temp(i) => temps[i] = v,
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
