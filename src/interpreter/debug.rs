//! Opt-in debugger over the same sealed tape dispatcher. No browser dependency.
use super::*;
use crate::ir::MAX_NODES;
use serde_json::{Value, json};

pub const MAX_TASKS: usize = 8;
pub const MAX_UNITS: usize = 16;
pub const MAX_BATCH: usize = 2048;

#[derive(Clone, Copy)]
enum Pending {
    Load { target: usize, id: u64 },
    Record { target: usize, id: u64 },
}
struct Frame {
    state: State,
    temps: [i64; MAX_NODES],
}
struct Task {
    unit: Rc<InterpreterUnit>,
    frame: Box<Frame>,
    owners: Vec<Option<QuoteBuffer>>,
    output: Output,
    options: ScriptLimits,
    cancel: Cancellation,
    calls: u64,
    pending: Option<Pending>,
    status: &'static str,
    error: Option<String>,
}
impl Task {
    fn stop(&mut self, status: &'static str, error: Option<String>) {
        self.status = status;
        self.error = error;
        self.pending = None;
        self.owners.iter_mut().for_each(|o| *o = None);
        self.frame.state.row = None;
    }
    fn snapshot(&self) -> Value {
        let s = &self.frame.state;
        json!({"status":self.status,"error":self.error,"pc":s.pc,
            "index":s.index,"remaining":s.remaining.to_string(),
            "locals":s.locals.map(|x|x.to_string()),"stdout":self.output.stdout,
            "frame":{"offset":(&*self.frame as *const Frame) as usize,
                "bytes":std::mem::size_of::<Frame>(),"locals_offset":s.locals.as_ptr() as usize},
            "tape":{"offset":self.unit.ops.as_ptr() as usize,
                "bytes":self.unit.ops.len()*std::mem::size_of::<Op>(),"stride":std::mem::size_of::<Op>()},
            "buffers":self.owners.iter().enumerate().filter_map(|(slot,o)|o.as_ref().map(|b|
                json!({"slot":slot,"offset":b.rows().as_ptr() as usize,"bytes":b.bytes(),"rows":b.rows().len(),
                    "preview":b.rows().iter().take(4).map(|q|[q.unit_price_cents.to_string(),q.quantity.to_string()]).collect::<Vec<_>>()}))).collect::<Vec<_>>()})
    }
}
#[derive(Default)]
pub struct Engine {
    units: Vec<Rc<InterpreterUnit>>,
    tasks: Vec<Task>,
    sequence: u64,
}
impl Engine {
    pub fn prepare(&mut self, source: &str, inputs: &[(&str, Type)]) -> Result<Value, String> {
        if self.units.len() == MAX_UNITS {
            return Err("prepared unit limit".into());
        }
        let unit =
            Rc::new(InterpreterUnit::prepare_script(source, inputs).map_err(|e| e.to_string())?);
        let id = self.units.len();
        let tape = unit
            .ops
            .iter()
            .enumerate()
            .map(|(pc, op)| json!({"pc":pc,"instruction":format!("{op:?}")}))
            .collect::<Vec<_>>();
        self.units.push(unit);
        Ok(json!({"unit":id,"tape":tape}))
    }
    pub fn create(
        &mut self,
        unit: usize,
        inputs: &[i64],
        options: ScriptLimits,
    ) -> Result<Value, String> {
        if self.tasks.len() == MAX_TASKS {
            return Err("task limit; reset to reclaim task slots".into());
        }
        let unit = self.units.get(unit).ok_or("unknown unit")?.clone();
        if inputs.len() != unit.inputs.len()
            || inputs
                .iter()
                .zip(&unit.inputs)
                .any(|(v, t)| *t == Type::Bool && ![0, 1].contains(v))
        {
            return Err("Input".into());
        }
        if options.limits.chunk == 0
            || options.limits.chunk > 4096
            || options.limits.rows > 1_000_000
            || options.limits.buffer_bytes > 16_000_000
            || options.stdout_bytes > 16384
            || options.tool_calls > 512
        {
            return Err("Budget".into());
        }
        let mut state = State {
            locals: [0; MAX_LOCALS],
            pc: 0,
            index: 0,
            chunk_left: 0,
            remaining: options.limits.rows,
            row: None,
        };
        state.locals[..inputs.len()].copy_from_slice(inputs);
        let owners = (0..unit.buffers).map(|_| None).collect();
        let task = self.tasks.len();
        self.tasks.push(Task {
            unit,
            frame: Box::new(Frame {
                state,
                temps: [0; MAX_NODES],
            }),
            owners,
            output: Output::new(options.stdout_bytes, options.allow_print),
            options,
            cancel: Cancellation::default(),
            calls: options.tool_calls,
            pending: None,
            status: "ready",
            error: None,
        });
        Ok(json!({"task":task,"snapshot":self.tasks[task].snapshot()}))
    }
    fn event(&mut self, task: usize, kind: &str, detail: Value) -> Value {
        self.sequence += 1;
        json!({"id":self.sequence,"task":task,"kind":kind,"detail":detail})
    }
    pub fn snapshot(&self, task: usize) -> Result<Value, String> {
        Ok(self.tasks.get(task).ok_or("unknown task")?.snapshot())
    }
    pub fn cancel(&mut self, task: usize) -> Result<Value, String> {
        let t = self.tasks.get_mut(task).ok_or("unknown task")?;
        if matches!(t.status, "done" | "error" | "cancelled") {
            return Ok(json!({"events":[],"snapshot":t.snapshot()}));
        }
        t.cancel.cancel();
        t.stop("cancelled", Some("Cancelled".into()));
        let event = self.event(task, "cancel", json!({}));
        Ok(json!({"events":[event],"snapshot":self.snapshot(task)?}))
    }
    pub fn advance(&mut self, task: usize, count: usize, trace: bool) -> Result<Value, String> {
        if count == 0 || count > MAX_BATCH {
            return Err("batch must be 1..2048".into());
        }
        if task >= self.tasks.len() {
            return Err("unknown task".into());
        }
        let mut events = Vec::new();
        for _ in 0..if trace { count } else { 1 } {
            let t = &mut self.tasks[task];
            if t.status != "ready" {
                break;
            }
            let s = &t.frame.state;
            let pc = s.pc;
            let dest = if trace {
                t.unit.ops.get(pc).and_then(Op::destination)
            } else {
                None
            };
            let locals = s.locals;
            let temps = t.frame.temps;
            let out_len = t.output.stdout.len();
            let read = if let Some(Op::Scan { buffer, .. }) = t.unit.ops.get(pc) {
                t.owners[*buffer].as_ref().filter(|b|s.index<b.rows().len()).map(|b|
                    json!({"slot":buffer,"offset":b.rows().as_ptr() as usize+s.index*16,"bytes":16,"index":s.index}))
            } else {
                None
            };
            let f = &mut *t.frame;
            let result = if trace {
                tape::run::<true>(
                    &t.unit,
                    &mut f.state,
                    &t.owners,
                    &t.options.limits,
                    &mut t.output,
                    &mut f.temps,
                )
            } else {
                tape::run::<false>(
                    &t.unit,
                    &mut f.state,
                    &t.owners,
                    &t.options.limits,
                    &mut t.output,
                    &mut f.temps,
                )
            };
            let writes = f.state.locals.iter().zip(locals).enumerate().filter(|(slot, (a,b))|**a!=*b || (result.is_ok() && matches!(dest,Some(tape::Dest::Local(i)) if usize::from(i)==*slot)))
                .map(|(slot,(v,_))|json!({"slot":slot,"value":v.to_string(),"offset":f.state.locals.as_ptr() as usize+slot*8,"bytes":8})).collect::<Vec<_>>();
            let scratch = f.temps.iter().zip(temps).enumerate().filter(|(slot, (a,b))|**a!=*b || (result.is_ok() && matches!(dest,Some(tape::Dest::Temp(i)) if usize::from(i)==*slot)))
                .map(|(slot,(v,_))|json!({"slot":slot,"value":v.to_string(),"offset":f.temps.as_ptr() as usize+slot*8,"bytes":8})).collect::<Vec<_>>();
            let detail = json!({"pc":pc,"next_pc":f.state.pc,"writes":writes,"scratch":scratch,"read":if trace {read} else {None},
                "granularity":if trace {"instruction"} else {"boundary"},
                "instruction":if trace && pc<t.unit.ops.len() {Some(json!({"offset":t.unit.ops.as_ptr() as usize+pc*std::mem::size_of::<Op>(),"bytes":std::mem::size_of::<Op>()}))} else {None},
                "stdout":&t.output.stdout[out_len..]});
            let (kind, extra) = match result {
                #[cfg(feature = "tool-json")]
                Ok(Boundary::Tool(..)) => {
                    unreachable!("debug Engine only prepares legacy Tool schemas")
                }
                Ok(Boundary::Step) => ("instruction", Value::Null),
                Ok(Boundary::Done) => {
                    t.stop("done", None);
                    ("done", Value::Null)
                }
                Ok(Boundary::Yield) => {
                    release(&mut t.owners, t.unit.buffer_mask(t.frame.state.pc));
                    ("yield", Value::Null)
                }
                Ok(Boundary::Load(target, batch)) => {
                    let held = release(&mut t.owners, t.unit.buffer_mask(t.frame.state.pc));
                    match runtime::authorize_tool(
                        &t.cancel,
                        t.options.limits.allow_load,
                        &mut t.calls,
                    ) {
                        Ok(()) => {
                            let id = self.sequence + 1;
                            t.pending = Some(Pending::Load { target, id });
                            t.status = "load_wait";
                            (
                                "load_wait",
                                json!({"request":id,"batch":batch.to_string(),"available_bytes":t.options.limits.buffer_bytes.saturating_sub(held)}),
                            )
                        }
                        Err(e) => {
                            t.stop("error", Some(e.to_string()));
                            ("error", Value::Null)
                        }
                    }
                }
                Ok(Boundary::Record(target, batch, total)) => {
                    release(&mut t.owners, t.unit.buffer_mask(t.frame.state.pc));
                    match runtime::authorize_tool(
                        &t.cancel,
                        t.options.limits.allow_record,
                        &mut t.calls,
                    ) {
                        Ok(()) => {
                            let id = self.sequence + 1;
                            t.pending = Some(Pending::Record { target, id });
                            t.status = "record_wait";
                            (
                                "record_wait",
                                json!({"request":id,"batch":batch.to_string(),"total":total.to_string()}),
                            )
                        }
                        Err(e) => {
                            t.stop("error", Some(e.to_string()));
                            ("error", Value::Null)
                        }
                    }
                }
                Err(e) => {
                    t.stop("error", Some(e.to_string()));
                    ("error", Value::Null)
                }
            };
            events.push(self.event(task, kind, json!({"execution":detail,"tool":extra})));
            if kind != "instruction" {
                break;
            }
        }
        Ok(json!({"events":events,"snapshot":self.snapshot(task)?}))
    }
    /// Host completion token identifies a single operation. Cancelled task ignores
    /// late completion; host operation itself remains detached and may finish.
    pub fn complete_fixture(
        &mut self,
        task: usize,
        request: u64,
        count: usize,
        quote: Quote,
    ) -> Result<Value, String> {
        let t = self.tasks.get_mut(task).ok_or("unknown task")?;
        if t.status == "cancelled" {
            return self.complete(task, request, None, None, None);
        }
        let Some(Pending::Load { id, .. }) = t.pending else {
            return Err("task is not waiting for load".into());
        };
        if request != id {
            return Err("stale operation token".into());
        }
        let held = t
            .owners
            .iter()
            .flatten()
            .map(QuoteBuffer::bytes)
            .sum::<usize>();
        if count > 1_000_000 || count > t.options.limits.buffer_bytes.saturating_sub(held) / 16 {
            t.stop("error", Some(TaskError::Budget.to_string()));
            let e = self.event(task, "tool_resume", json!({"request":request}));
            return Ok(json!({"events":[e],"snapshot":self.snapshot(task)?}));
        }
        self.complete(task, request, Some(vec![quote; count]), None, None)
    }
    pub fn complete(
        &mut self,
        task: usize,
        request: u64,
        rows: Option<Vec<Quote>>,
        receipt: Option<i64>,
        error: Option<String>,
    ) -> Result<Value, String> {
        let t = self.tasks.get_mut(task).ok_or("unknown task")?;
        if t.status == "cancelled" {
            return Ok(json!({"ignored":true,"snapshot":t.snapshot()}));
        }
        let pending = t.pending.ok_or("task is not waiting")?;
        let id = match pending {
            Pending::Load { id, .. } | Pending::Record { id, .. } => id,
        };
        if request != id {
            return Err("stale operation token".into());
        }
        let status = if let Some(error) = error {
            Err(TaskError::Provider(error))
        } else {
            match pending {
                Pending::Load { target, .. } => {
                    let rows = rows.ok_or("load requires rows")?;
                    let held = t
                        .owners
                        .iter()
                        .flatten()
                        .map(QuoteBuffer::bytes)
                        .sum::<usize>();
                    if rows.len() > t.options.limits.buffer_bytes.saturating_sub(held) / 16 {
                        Err(TaskError::Budget)
                    } else {
                        let b = QuoteBuffer::from_quotes(rows).map_err(|e| e.to_string())?;
                        if t.unit.buffer_mask(t.frame.state.pc) & (1 << target) != 0 {
                            t.owners[target] = Some(b);
                        }
                        Ok(())
                    }
                }
                Pending::Record { target, .. } => {
                    t.frame.state.locals[target] = receipt.ok_or("record requires receipt")?;
                    Ok(())
                }
            }
        };
        t.pending = None;
        match status {
            Ok(()) => t.status = "ready",
            Err(e) => t.stop("error", Some(e.to_string())),
        }
        let writes = match pending {
            Pending::Record { target, .. } if t.status == "ready" => vec![
                json!({"slot":target,"value":t.frame.state.locals[target].to_string(),"offset":t.frame.state.locals.as_ptr() as usize+target*8,"bytes":8}),
            ],
            _ => Vec::new(),
        };
        let e = self.event(
            task,
            "tool_resume",
            json!({"request":request,"writes":writes}),
        );
        Ok(json!({"events":[e],"snapshot":self.snapshot(task)?}))
    }
}
