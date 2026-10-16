//! Thread-local iterative task dispatcher with owned, detached I/O operations.
use crate::error::TaskError;
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    buffer::QuoteBuffer,
    codegen::{CodeUnit, Frame, View},
    provider::Provider,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::Notify;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::oneshot;
#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    notify: Notify,
}
#[derive(Clone, Default)]
pub struct Cancellation(Arc<CancelState>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        self.0.notify.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }
    #[cfg(not(target_arch = "wasm32"))]
    async fn cancelled(&self) {
        let notification = self.0.notify.notified();
        tokio::pin!(notification);
        // Register before checking the flag so concurrent cancellation cannot be lost.
        notification.as_mut().enable();
        if !self.is_cancelled() {
            notification.await;
        }
    }
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub rows: u64,
    pub chunk: usize,
    pub buffer_bytes: usize,
    pub allow_load: bool,
    pub allow_record: bool,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            rows: 1_000_000,
            chunk: 256,
            buffer_bytes: 16_000_000,
            allow_load: true,
            allow_record: true,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Event {
    pub phase: &'static str,
    pub buffer_bytes: usize,
    pub frame_bytes: usize,
    pub remaining: u64,
    pub index: u64,
}
pub type Trace = Rc<RefCell<Vec<Event>>>;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn event(
    trace: &Option<Trace>,
    phase: &'static str,
    buffer_bytes: usize,
    frame_bytes: usize,
    remaining: u64,
    index: u64,
) {
    if let Some(t) = trace {
        t.borrow_mut().push(Event {
            phase,
            buffer_bytes,
            frame_bytes,
            remaining,
            index,
        });
    }
}
pub(crate) fn check(cancel: &Cancellation) -> Result<(), TaskError> {
    if cancel.is_cancelled() {
        Err(TaskError::Cancelled)
    } else {
        Ok(())
    }
}
pub(crate) fn authorize_tool(
    cancel: &Cancellation,
    allowed: bool,
    calls: &mut u64,
) -> Result<(), TaskError> {
    check(cancel)?;
    if !allowed {
        return Err(TaskError::Permission);
    }
    if *calls == 0 {
        return Err(TaskError::Budget);
    }
    *calls -= 1;
    Ok(())
}
/// Futures own all required arguments. Dropping the receiver does not drop the operation.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn receive<T>(
    rx: oneshot::Receiver<Result<T, TaskError>>,
    cancel: &Cancellation,
) -> Result<T, TaskError> {
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(TaskError::Cancelled),
        result = rx => result.map_err(|_| TaskError::Provider("operation lost".into()))?,
    };
    check(cancel)?;
    result
}
#[cfg(not(target_arch = "wasm32"))]
pub async fn execute<P: Provider>(
    code: Rc<CodeUnit>,
    provider: Arc<P>,
    batch: i64,
    price_limit: i64,
    limits: Limits,
    cancel: Cancellation,
    trace: Option<Trace>,
) -> Result<i64, TaskError> {
    if !code.legacy {
        return Err(TaskError::Input);
    }
    let mut output = Output::new(0, false);
    run(
        code,
        provider,
        &[batch, price_limit],
        ScriptLimits {
            limits,
            ..ScriptLimits::default()
        },
        cancel,
        trace,
        &mut output,
    )
    .await
}

/// Additional script bounds. A Tool invocation does not reset scan fuel.
#[derive(Clone, Copy)]
pub struct ScriptLimits {
    pub limits: Limits,
    pub tool_calls: u64,
    pub stdout_bytes: usize,
    pub allow_print: bool,
    /// Cumulative UTF-8 validation, JSON escape decode and contains scan bytes.
    pub string_scan_bytes: usize,
}
impl Default for ScriptLimits {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            tool_calls: 512,
            stdout_bytes: 16 * 1024,
            allow_print: true,
            string_scan_bytes: 1024 * 1024,
        }
    }
}
pub struct ScriptOutcome {
    pub status: Result<(), TaskError>,
    pub stdout: String,
}
pub(crate) struct Output {
    pub(crate) stdout: String,
    max: usize,
    allowed: bool,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) error: Option<TaskError>,
    #[cfg(feature = "tool-json")]
    pub(crate) json: Option<Box<crate::json_view::State>>,
}
impl Output {
    #[cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
    pub(crate) fn append_plan(&mut self, index: usize, locals: &[i64]) -> Result<(), TaskError> {
        let plan = self.json.as_ref().expect("string plan").plan();
        self.append_string_source(&plan.prints[index], locals)
    }
    pub(crate) fn new(max: usize, allowed: bool) -> Self {
        Self {
            stdout: String::new(),
            max,
            allowed,
            #[cfg(not(target_arch = "wasm32"))]
            error: None,
            #[cfg(feature = "tool-json")]
            json: None,
        }
    }
}
/// Fixed native ABI, task-private output only. No I/O, callback dispatch or panic
/// on user data. One atomic line append: a rejected line leaves earlier output intact.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) unsafe extern "C" fn native_print(output: *mut Output, value: i64, boolean: i64) -> i64 {
    // Only generated code calls this with its dispatcher-owned live Output.
    let output = unsafe { &mut *output };
    match output.append(value, boolean != 0) {
        Ok(()) => 0,
        Err(error) => {
            output.error = Some(error);
            1
        }
    }
}
#[cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
pub(crate) unsafe extern "C" fn native_print_plan(
    output: *mut Output,
    frame: *const Frame,
    index: i64,
) -> i64 {
    let output = unsafe { &mut *output };
    let frame = unsafe { &*frame };
    match output.append_plan(index as usize, &frame.locals) {
        Ok(()) => 0,
        Err(e) => {
            output.error = Some(e);
            1
        }
    }
}
// Only runtime-validated StringValues enter this private consumer representation.
// Scalars are formatted on the stack until the final consumer owns the bytes.
#[cfg(feature = "tool-json")]
enum SinkPart {
    Text(crate::tool::StringValue),
    Scalar { bytes: [u8; 20], len: usize },
}
#[cfg(feature = "tool-json")]
impl SinkPart {
    fn scalar(value: i64, boolean: bool) -> Self {
        use std::io::Write;
        let mut bytes = [0u8; 20];
        let mut remaining = bytes.as_mut_slice();
        if boolean {
            remaining
                .write_all(if value == 0 { b"False" } else { b"True" })
                .unwrap();
        } else {
            // Every signed i64, including MIN, fits in 20 decimal ASCII bytes.
            write!(remaining, "{value}").unwrap();
        }
        let len = 20 - remaining.len();
        Self::Scalar { bytes, len }
    }
    fn len(&self) -> usize {
        use crate::tool::StringValue as V;
        match self {
            Self::Scalar { len, .. } => *len,
            Self::Text(V::Literal(s)) => s.len(),
            Self::Text(V::Decoded(s)) => s.len(),
            Self::Text(V::Joined(s)) => s.len(),
            Self::Text(V::Json { range, .. }) => range.len(),
        }
    }
    fn as_str(&self) -> &str {
        match self {
            Self::Scalar { bytes, len } => std::str::from_utf8(&bytes[..*len]).unwrap(),
            Self::Text(value) => value.as_str(),
        }
    }
}
impl Output {
    #[cfg(feature = "tool-json")]
    pub(crate) fn string_argument_source(
        &mut self,
        source: &crate::tool::StringSource,
        locals: &[i64],
        live: u32,
    ) -> Result<crate::tool::StringValue, TaskError> {
        use crate::tool::{StringSource as S, StringValue as V};
        match source {
            S::Json { slot, .. } => self
                .json
                .as_mut()
                .expect("JSON plan")
                .string_argument(*slot, live),
            S::JsonGetLiteral { slot, default, .. } => {
                let json = self.json.as_mut().expect("JSON plan");
                if json.get_present(*slot)? {
                    json.string_argument(*slot, live)
                } else {
                    Ok(V::Literal(default.clone()))
                }
            }
            _ => self.string_value(source, locals),
        }
    }
    #[cfg(feature = "tool-json")]
    fn sink_part(
        &mut self,
        source: &crate::tool::StringSource,
        locals: &[i64],
    ) -> Result<SinkPart, TaskError> {
        match source {
            crate::tool::StringSource::Scalar { slot, boolean } => {
                Ok(SinkPart::scalar(locals[*slot], *boolean))
            }
            crate::tool::StringSource::JsonInteger { slot, .. } => {
                let value = self.json.as_mut().expect("JSON plan").integer(*slot)?;
                Ok(SinkPart::scalar(value, false))
            }
            _ => self.string_value(source, locals).map(SinkPart::Text),
        }
    }
    #[cfg(feature = "tool-json")]
    pub(crate) fn string_value(
        &mut self,
        source: &crate::tool::StringSource,
        locals: &[i64],
    ) -> Result<crate::tool::StringValue, TaskError> {
        use crate::tool::{StringSource as S, StringValue as V};
        match source {
            S::Literal(s) => Ok(V::Literal(s.clone())),
            S::Json { slot, .. } => self.json.as_mut().expect("JSON plan").string_value(*slot),
            S::JsonGetLiteral { slot, default, .. } => {
                let json = self.json.as_mut().expect("JSON plan");
                if json.get_present(*slot)? {
                    json.string_value(*slot)
                } else {
                    Ok(V::Literal(default.clone()))
                }
            }
            S::JsonInteger { .. } => Ok(V::Joined(
                self.sink_part(source, locals)?.as_str().to_owned(),
            )),
            S::Scalar { slot, boolean } => Ok(V::Joined(if *boolean {
                if locals[*slot] == 0 {
                    "False".to_owned()
                } else {
                    "True".to_owned()
                }
            } else {
                locals[*slot].to_string()
            })),
            S::Joined(parts) => {
                let mut values: [Option<SinkPart>; 32] = std::array::from_fn(|_| None);
                let mut len = 0usize;
                for (i, part) in parts.iter().enumerate() {
                    values[i] = Some(self.sink_part(part, locals)?);
                    len = len
                        .checked_add(values[i].as_ref().unwrap().len())
                        .ok_or(TaskError::Budget)?;
                    if len > 64 * 1024 {
                        return Err(TaskError::Budget);
                    }
                }
                if len > self.json.as_ref().map_or(usize::MAX, |j| j.available()) {
                    return Err(TaskError::Budget);
                }
                let mut text = String::new();
                text.try_reserve_exact(len).map_err(|_| TaskError::Budget)?;
                for value in values.iter().take(parts.len()).flatten() {
                    text.push_str(value.as_str());
                }
                Ok(V::Joined(text))
            }
        }
    }
    #[cfg(feature = "tool-json")]
    pub(crate) fn append_string_source(
        &mut self,
        source: &crate::tool::StringSource,
        locals: &[i64],
    ) -> Result<(), TaskError> {
        use crate::tool::StringSource as S;
        let parts = match source {
            crate::tool::StringSource::Joined(parts) => parts.as_slice(),
            other => std::slice::from_ref(other),
        };
        let mut values: [Option<SinkPart>; 32] = std::array::from_fn(|_| None);
        let mut len = 1usize;
        let mut present = 0u32;
        for (i, part) in parts.iter().enumerate() {
            // Preserve left-to-right validation, scan charging and size failures.
            // Only scalars need temporary storage. Text stays in its current owner.
            let size = match part {
                S::Literal(s) => s.len(),
                S::Json { slot, .. } => {
                    let json = self.json.as_mut().expect("JSON plan");
                    json.validate_string(*slot)?;
                    json.prepared_string(*slot).len()
                }
                S::JsonGetLiteral { slot, default, .. } => {
                    let json = self.json.as_mut().expect("JSON plan");
                    if json.get_present(*slot)? {
                        present |= 1 << i;
                        json.validate_string(*slot)?;
                        json.prepared_string(*slot).len()
                    } else {
                        default.len()
                    }
                }
                S::Scalar { slot, boolean } => {
                    let value = SinkPart::scalar(locals[*slot], *boolean);
                    let size = value.len();
                    values[i] = Some(value);
                    size
                }
                S::JsonInteger { .. } => {
                    let value = self.sink_part(part, locals)?;
                    let size = value.len();
                    values[i] = Some(value);
                    size
                }
                S::Joined(_) => unreachable!("flattened sink plan"),
            };
            len = len.checked_add(size).ok_or(TaskError::Budget)?;
            if len > 64 * 1024 + 1 {
                return Err(TaskError::Budget);
            }
        }
        if !self.allowed {
            return Err(TaskError::Permission);
        }
        if len > self.max.saturating_sub(self.stdout.len()) {
            return Err(TaskError::Budget);
        }
        let required = self.stdout.len() + len;
        if required > self.stdout.capacity() {
            let capacity = required
                .checked_next_power_of_two()
                .unwrap_or(self.max)
                .min(self.max);
            self.stdout
                .try_reserve_exact(capacity - self.stdout.len())
                .map_err(|_| TaskError::Budget)?;
        }
        for (i, part) in parts.iter().enumerate() {
            let text = match part {
                S::Literal(s) => s.as_ref(),
                S::Json { slot, .. } => self.json.as_ref().unwrap().prepared_string(*slot),
                S::JsonGetLiteral { slot, default, .. } => {
                    if present & (1 << i) != 0 {
                        self.json.as_ref().unwrap().prepared_string(*slot)
                    } else {
                        default.as_ref()
                    }
                }
                S::Scalar { .. } | S::JsonInteger { .. } => values[i].as_ref().unwrap().as_str(),
                S::Joined(_) => unreachable!("flattened sink plan"),
            };
            self.stdout.push_str(text);
        }
        self.stdout.push('\n');
        Ok(())
    }
    pub(crate) fn append(&mut self, value: i64, boolean: bool) -> Result<(), TaskError> {
        let mut bytes = [0u8; 21];
        bytes[20] = b'\n';
        let line = if boolean {
            if value == 0 { "False\n" } else { "True\n" }
        } else {
            let mut n = value.unsigned_abs();
            let mut position = 20;
            loop {
                position -= 1;
                bytes[position] = b'0' + (n % 10) as u8;
                n /= 10;
                if n == 0 {
                    break;
                }
            }
            if value < 0 {
                position -= 1;
                bytes[position] = b'-';
            }
            // The stack buffer contains only ASCII digits, minus and newline.
            unsafe { std::str::from_utf8_unchecked(&bytes[position..]) }
        };
        self.append_line(line)
    }
    #[cfg(feature = "tool-json")]
    pub(crate) fn append_json_string(&mut self, slot: usize) -> Result<(), TaskError> {
        // Evaluate and validate the leaf before testing print authority.
        let json = self.json.as_mut().expect("sealed JSON plan");
        json.validate_string(slot)?;
        let line = json.string(slot)?;
        Self::append_parts(&mut self.stdout, self.max, self.allowed, line, true)
    }
    fn append_line(&mut self, line: &str) -> Result<(), TaskError> {
        Self::append_parts(&mut self.stdout, self.max, self.allowed, line, false)
    }
    fn append_parts(
        stdout: &mut String,
        max: usize,
        allowed: bool,
        line: &str,
        newline: bool,
    ) -> Result<(), TaskError> {
        if !allowed {
            return Err(TaskError::Permission);
        }
        let length = line.len() + usize::from(newline);
        if length > max.saturating_sub(stdout.len()) {
            return Err(TaskError::Budget);
        }
        let required = stdout.len() + length;
        if required > stdout.capacity() {
            // Amortize repeated line appends without letting capacity exceed the
            // task's explicit output bound. Small single-line output stays small.
            let capacity = required.checked_next_power_of_two().unwrap_or(max).min(max);
            if stdout.try_reserve_exact(capacity - stdout.len()).is_err() {
                return Err(TaskError::Budget);
            }
        }
        stdout.push_str(line);
        if newline {
            stdout.push('\n');
        }
        Ok(())
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub async fn execute_script<P: Provider>(
    code: Rc<CodeUnit>,
    provider: Arc<P>,
    inputs: &[i64],
    limits: ScriptLimits,
    cancel: Cancellation,
    trace: Option<Trace>,
) -> ScriptOutcome {
    let mut output = Output::new(limits.stdout_bytes, limits.allow_print);
    let status = run(code, provider, inputs, limits, cancel, trace, &mut output)
        .await
        .map(|_| ());
    ScriptOutcome {
        status,
        stdout: output.stdout,
    }
}
#[allow(clippy::too_many_arguments)]
#[cfg(not(target_arch = "wasm32"))]
async fn run<P: Provider>(
    code: Rc<CodeUnit>,
    provider: Arc<P>,
    inputs: &[i64],
    options: ScriptLimits,
    cancel: Cancellation,
    trace: Option<Trace>,
    output: &mut Output,
) -> Result<i64, TaskError> {
    check(&cancel)?;
    if inputs.len() != code.inputs.len()
        || inputs
            .iter()
            .zip(&code.inputs)
            .any(|(v, ty)| *ty == crate::ir::Type::Bool && ![0, 1].contains(v))
    {
        return Err(TaskError::Input);
    }
    let limits = options.limits;
    if limits.chunk == 0 || limits.chunk > 4096 {
        return Err(TaskError::Budget);
    }
    #[cfg(feature = "tool-json")]
    if let Some(plan) = &code.json
        && (plan.roots > 0 || !plan.prints.is_empty())
    {
        output.json = Some(Box::new(crate::json_view::State::new(
            plan.clone(),
            limits.buffer_bytes,
            options.string_scan_bytes,
        )));
    }
    let mut calls = options.tool_calls;
    let mut owners: Vec<Option<QuoteBuffer>> = (0..code.buffers).map(|_| None).collect();
    let mut views = vec![View::default(); code.buffers];
    let mut frame = Box::new(Frame::new(inputs, limits.rows, limits.chunk));
    loop {
        check(&cancel)?;
        #[cfg(feature = "tool-json")]
        if code
            .json
            .as_ref()
            .is_some_and(|p| p.string_paths || p.array_paths)
            && let Some(json) = &mut output.json
        {
            json.set_external(owners.iter().flatten().map(QuoteBuffer::bytes).sum());
        }
        let status = code.resume(&mut frame, &views, output);
        let boundary = matches!(status, 0 | 2 | 3 | 4 | 7);
        #[cfg(feature = "tool-json")]
        let boundary = boundary || status >= 8;
        if boundary {
            for (i, owner) in owners.iter_mut().enumerate() {
                if frame.live & (1u64 << i) == 0 {
                    *owner = None;
                    views[i] = View::default();
                }
            }
        }
        let held: usize = owners.iter().flatten().map(QuoteBuffer::bytes).sum();
        match status {
            0 => {
                event(&trace, "done", 0, 0, frame.remaining, frame.index);
                return Ok(frame.total);
            }
            1 => return Err(TaskError::Overflow),
            5 => return Err(TaskError::Budget),
            6 => return Err(output.error.take().unwrap_or(TaskError::Budget)),
            4 => {
                event(
                    &trace,
                    "yield",
                    held,
                    std::mem::size_of::<Frame>(),
                    frame.remaining,
                    frame.index,
                );
                tokio::task::yield_now().await;
            }
            2 | 3 | 7 => {
                authorize_tool(
                    &cancel,
                    if status == 2 {
                        limits.allow_load
                    } else {
                        limits.allow_record
                    },
                    &mut calls,
                )?;
                let batch = frame.batch;
                let total = frame.total;
                let target = frame.target as usize;
                if status == 7 {
                    let remaining = frame.remaining;
                    let index = frame.index;
                    // Proven terminal record: no remaining native continuation.
                    drop(frame);
                    drop(owners);
                    drop(views);
                    drop(code);
                    event(&trace, "record_wait", 0, 0, remaining, index);
                    let (tx, rx) = oneshot::channel();
                    tokio::spawn(async move {
                        let _ = tx.send(provider.record_total(batch, total).await);
                    });
                    let receipt = receive(rx, &cancel).await?;
                    event(&trace, "done", 0, 0, remaining, index);
                    return Ok(receipt);
                }
                let p = provider.clone();
                if status == 2 {
                    event(
                        &trace,
                        "load_wait",
                        held,
                        std::mem::size_of::<Frame>(),
                        frame.remaining,
                        frame.index,
                    );
                    let (tx, rx) = oneshot::channel();
                    tokio::spawn(async move {
                        let _ = tx.send(p.load_quotes(batch).await);
                    });
                    let quotes = receive(rx, &cancel).await?;
                    #[cfg(feature = "tool-json")]
                    let held = held + output.json.as_ref().map_or(0, |j| j.held());
                    if quotes.bytes() > limits.buffer_bytes.saturating_sub(held) {
                        return Err(TaskError::Budget);
                    }
                    if frame.live & (1u64 << target) != 0 {
                        views[target] = View {
                            ptr: quotes.rows().as_ptr(),
                            len: quotes.rows().len() as u64,
                        };
                        owners[target] = Some(quotes);
                    }
                } else {
                    event(
                        &trace,
                        "record_wait",
                        held,
                        std::mem::size_of::<Frame>(),
                        frame.remaining,
                        frame.index,
                    );
                    let (tx, rx) = oneshot::channel();
                    tokio::spawn(async move {
                        let _ = tx.send(p.record_total(batch, total).await);
                    });
                    frame.locals[target] = receive(rx, &cancel).await?;
                }
            }
            #[cfg(feature = "tool-json")]
            status if status >= 8 => {
                let call = code
                    .json
                    .as_ref()
                    .ok_or(TaskError::Input)?
                    .calls
                    .get((status - 8) as usize)
                    .ok_or(TaskError::Input)?;
                let (id, count, json) = (call.id, call.arguments, call.json);
                let argument = match &call.string {
                    Some(source) => crate::tool::ToolArguments::String(
                        output.string_argument_source(source, &frame.locals, frame.batch as u32)?,
                    ),
                    None => crate::tool::ToolArguments::Numeric([frame.batch, frame.total], count),
                };
                authorize_tool(&cancel, provider.allows_tool(call.id), &mut calls)?;
                if call.string.as_ref().is_some_and(|s| s.roots() != 0) {
                    output
                        .json
                        .as_mut()
                        .expect("JSON plan")
                        .release(frame.batch as u32);
                }
                let target = frame.target as usize;
                let p = provider.clone();
                let (tx, rx) = oneshot::channel();
                tokio::spawn(async move {
                    let _ = tx.send(p.call_bound_tool(id, argument).await);
                });
                match (json, receive(rx, &cancel).await?) {
                    (true, crate::tool::Reply::Json(bytes)) => output
                        .json
                        .as_mut()
                        .expect("sealed JSON plan")
                        .install(target, bytes, limits.buffer_bytes.saturating_sub(held))?,
                    (false, crate::tool::Reply::I64(v)) => frame.locals[target] = v,
                    _ => return Err(TaskError::Json("Tool reply does not match declaration")),
                }
            }
            _ => return Err(TaskError::Budget),
        }
    }
}
#[cfg(test)]
mod output_tests {
    use super::*;
    #[cfg(feature = "tool-json")]
    #[test]
    fn borrowed_sink_prepares_before_authority_and_commits_atomically() {
        use crate::tool::{PathSlot, Plan, StringSource as S};
        for (allowed, max, scan, invalid, expected) in [
            (
                false,
                100,
                100,
                true,
                TaskError::Json("expected UTF-8 string"),
            ),
            (false, 100, 0, false, TaskError::Budget),
            (false, 100, 100, false, TaskError::Permission),
            (true, 2, 100, false, TaskError::Budget),
        ] {
            let plan = Rc::new(Plan {
                roots: 1,
                paths: ["a", "b"]
                    .into_iter()
                    .map(|key| PathSlot {
                        root: 0,
                        parent: None,
                        key: key.into(),
                    })
                    .collect(),
                string_paths: true,
                ..Plan::default()
            });
            let mut out = Output::new(max, allowed);
            out.stdout.push_str("1\n");
            let mut state = crate::json_view::State::new(plan, 1024, scan);
            state
                .install(
                    0,
                    bytes::Bytes::from_static(br#"{"a":"h\u00e9\n","b":4}"#),
                    1024,
                )
                .unwrap();
            out.json = Some(Box::new(state));
            let source = S::Joined(vec![
                S::Literal(Arc::from("[")),
                S::Json { slot: 0, root: 0 },
                S::Json {
                    slot: usize::from(invalid),
                    root: 0,
                },
                S::Literal(Arc::from("]")),
            ]);
            assert_eq!(out.append_string_source(&source, &[]), Err(expected));
            assert_eq!(out.stdout, "1\n");
        }
        println!(
            "sink_layout output={} part={} scalar_slots={}",
            std::mem::size_of::<Output>(),
            std::mem::size_of::<SinkPart>(),
            std::mem::size_of::<[Option<SinkPart>; 32]>()
        );
    }
    #[test]
    fn growth_never_exceeds_the_task_cap_and_rejected_lines_are_atomic() {
        for max in [0, 1, 2, 3, 7, 17, 31, 64] {
            let mut out = Output::new(max, true);
            for _ in 0..64 {
                let before = out.stdout.clone();
                let status = unsafe { native_print(&mut out, 7, 0) };
                assert!(out.stdout.capacity() <= max);
                if status != 0 {
                    assert_eq!(out.stdout, before);
                    assert_eq!(out.error, Some(TaskError::Budget));
                    break;
                }
                assert_eq!(out.stdout.len(), before.len() + 2);
            }
        }
    }
}
