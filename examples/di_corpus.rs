//! Controlled corpus replay only. No external Python, filesystem Tool or network executor.
use bytes::Bytes;
use minipy_native::{
    buffer::QuoteBuffer,
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    ir::{AdmissionKind, Type},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{
        ArgumentSchema, JsonLeaf, ObjectArraySchema, Reply, ResultSchema, ToolArguments,
        ToolDeclaration,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    io::{self, BufRead},
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
pub struct Case {
    pub case_id: String,
    pub source: String,
    #[serde(default)]
    inputs: Vec<Input>,
    #[serde(default)]
    tools: Vec<Tool>,
    #[serde(default)]
    replies: Vec<Replay>,
    expected: Option<Value>,
    contract_complete: Option<bool>,
    oracle_complete: Option<bool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    name: String,
    r#type: Scalar,
    value: i64,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Scalar {
    I64,
    Bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tool {
    id: u32,
    name: String,
    arguments: Vec<Argument>,
    result: ResultSpec,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Argument {
    name: String,
    r#type: ArgType,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum ArgType {
    I64,
    String,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ResultSpec {
    I64,
    Json {
        leaves: Vec<Leaf>,
    },
    ObjectArray {
        path: Vec<String>,
        i64_fields: Vec<String>,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Leaf {
    path: Vec<String>,
    r#type: ArgType,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Replay {
    tool_id: u32,
    arguments: Vec<Value>,
    i64: Option<i64>,
    json: Option<String>,
    error: Option<String>,
    #[serde(default)]
    wait_ms: u64,
}
impl Case {
    fn declarations(&self) -> Vec<ToolDeclaration> {
        self.tools
            .iter()
            .map(|t| ToolDeclaration {
                id: t.id,
                name: t.name.clone(),
                arguments: t
                    .arguments
                    .iter()
                    .map(|a| match a.r#type {
                        ArgType::I64 => ArgumentSchema::I64(a.name.clone()),
                        ArgType::String => ArgumentSchema::String(a.name.clone()),
                    })
                    .collect(),
                result: match &t.result {
                    ResultSpec::I64 => ResultSchema::I64,
                    ResultSpec::Json { leaves } => ResultSchema::JsonTyped(
                        leaves
                            .iter()
                            .map(|l| {
                                (
                                    l.path.clone(),
                                    match l.r#type {
                                        ArgType::I64 => JsonLeaf::I64,
                                        ArgType::String => JsonLeaf::String,
                                    },
                                )
                            })
                            .collect(),
                    ),
                    ResultSpec::ObjectArray { path, i64_fields } => {
                        ResultSchema::JsonObjectArray(Box::new(ObjectArraySchema {
                            path: path.clone(),
                            i64_fields: i64_fields.clone(),
                        }))
                    }
                },
            })
            .collect()
    }
    pub fn validate_replay(&self) -> Result<(), String> {
        if self.replies.len() > 512 {
            return Err("at most 512 replies".into());
        }
        for r in &self.replies {
            if usize::from(r.i64.is_some())
                + usize::from(r.json.is_some())
                + usize::from(r.error.is_some())
                != 1
            {
                return Err("reply requires exactly one of i64/json/error".into());
            }
            if r.wait_ms > 1000 {
                return Err("wait_ms exceeds smoke limit 1000".into());
            }
            if r.arguments.len() > 2
                || r.arguments
                    .iter()
                    .any(|v| v.as_i64().is_none() && !v.is_string())
            {
                return Err("reply arguments must be <=2 i64/string values".into());
            }
            if r.json.as_ref().is_some_and(|s| s.len() > 16_000_000) {
                return Err("reply exceeds 16MB".into());
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct Ownership {
    next: usize,
    live: BTreeMap<usize, usize>,
    snapshots: Vec<Value>,
}
impl Ownership {
    fn snapshot(&mut self, phase: &str) {
        self.snapshots.push(json!({"phase":phase,"unique_json_owner_backing_bytes":self.live.values().sum::<usize>(),"unique_json_owners":self.live.len(),"task_state_bytes":null}));
    }
}
struct TrackedBytes {
    data: Vec<u8>,
    id: usize,
    ownership: Arc<Mutex<Ownership>>,
}
impl AsRef<[u8]> for TrackedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}
impl Drop for TrackedBytes {
    fn drop(&mut self) {
        self.ownership.lock().unwrap().live.remove(&self.id);
    }
}
#[derive(Default)]
struct Log {
    calls: Vec<Value>,
    waits: Vec<Value>,
    mismatch: bool,
}
struct Host {
    replies: Mutex<VecDeque<Replay>>,
    allowed: Vec<u32>,
    log: Mutex<Log>,
    ownership: Option<Arc<Mutex<Ownership>>>,
}
impl Host {
    fn snapshot(&self, phase: &str) {
        if let Some(o) = &self.ownership {
            o.lock().unwrap().snapshot(phase);
        }
    }
    async fn submit(&self, id: u32, arguments: Vec<Value>) -> Result<Reply, TaskError> {
        self.snapshot("before_submission");
        let reply = {
            let mut log = self.log.lock().unwrap();
            log.calls.push(json!({"tool_id":id,"arguments":arguments}));
            match self.replies.lock().unwrap().pop_front() {
                Some(r) if r.tool_id == id && r.arguments == arguments => Some(r),
                _ => {
                    log.mismatch = true;
                    None
                }
            }
        }
        .ok_or_else(|| TaskError::Provider("replay submission mismatch".into()))?;
        if reply.wait_ms > 0 {
            self.snapshot("tool_wait_start");
            let start = Instant::now();
            tokio::time::sleep(Duration::from_millis(reply.wait_ms)).await;
            let ns = start.elapsed().as_nanos() as u64;
            let submission = self.log_index();
            self.log.lock().unwrap().waits.push(
                json!({"submission":submission,"requested_ms":reply.wait_ms,"observed_ns":ns}),
            );
            self.snapshot("tool_wait_end");
        }
        if let Some(error) = reply.error {
            return Err(TaskError::Provider(error));
        }
        if let Some(n) = reply.i64 {
            return Ok(Reply::I64(n));
        }
        let data = reply.json.unwrap().into_bytes();
        let bytes = if let Some(o) = &self.ownership {
            let mut state = o.lock().unwrap();
            let id = state.next;
            state.next += 1;
            state.live.insert(id, data.capacity());
            drop(state);
            Bytes::from_owner(TrackedBytes {
                data,
                id,
                ownership: o.clone(),
            })
        } else {
            Bytes::from(data)
        };
        self.snapshot("reply_handoff");
        Ok(Reply::Json(bytes))
    }
    fn log_index(&self) -> usize {
        self.log.lock().unwrap().calls.len() - 1
    }
}
impl Provider for Host {
    fn allows_tool(&self, id: u32) -> bool {
        self.allowed.contains(&id)
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        let values = match args {
            ToolArguments::Numeric(v, n) => v[..n].iter().map(|n| json!(n)).collect(),
            ToolArguments::String(s) => vec![json!(s.as_str())],
        };
        self.submit(id, values).await
    }
    async fn load_quotes(&self, batch: i64) -> Result<QuoteBuffer, TaskError> {
        self.log
            .lock()
            .unwrap()
            .calls
            .push(json!({"tool_id":0,"arguments":[batch]}));
        Err(TaskError::Permission)
    }
    async fn record_total(&self, a: i64, b: i64) -> Result<i64, TaskError> {
        match self.submit(1, vec![json!(a), json!(b)]).await? {
            Reply::I64(n) => Ok(n),
            _ => Err(TaskError::Input),
        }
    }
}

enum Prepared {
    Tape(Rc<InterpreterUnit>),
    Native(Rc<CodeUnit>),
}
impl Prepared {
    fn memory(&self) -> Value {
        match self {
            Self::Tape(u) => {
                json!({"phase":"prepared","logical_prepared_bytes":u.prepared_bytes(),"emitted_code_bytes":0,"native_metadata_bytes":null})
            }
            Self::Native(u) => {
                json!({"phase":"prepared","logical_prepared_bytes":null,"emitted_code_bytes":u.code_bytes,"native_metadata_bytes":null})
            }
        }
    }
}
fn error_kind(e: &TaskError) -> &'static str {
    match e {
        TaskError::Overflow => "Overflow",
        TaskError::Budget => "Budget",
        TaskError::Cancelled => "Cancelled",
        TaskError::Permission => "Permission",
        TaskError::Buffer => "Buffer",
        TaskError::Input => "Input",
        TaskError::Provider(_) => "Provider",
        TaskError::Json(_) => "Json",
    }
}

pub async fn run(case: &Case, native: bool, diagnostic: bool) -> Value {
    let request_start = Instant::now();
    let ownership = diagnostic.then(|| Arc::new(Mutex::new(Ownership::default())));
    let setup = Instant::now();
    let host = Arc::new(Host {
        replies: Mutex::new(case.replies.clone().into()),
        allowed: case.tools.iter().map(|t| t.id).collect(),
        log: Mutex::new(Log::default()),
        ownership: ownership.clone(),
    });
    let declarations = case.declarations();
    let inputs: Vec<_> = case
        .inputs
        .iter()
        .map(|i| {
            (
                i.name.as_str(),
                match i.r#type {
                    Scalar::I64 => Type::I64,
                    Scalar::Bool => Type::Bool,
                },
            )
        })
        .collect();
    let values: Vec<_> = case.inputs.iter().map(|i| i.value).collect();
    let trace = diagnostic.then(|| Rc::new(RefCell::new(Vec::new())));
    let setup_ns = setup.elapsed().as_nanos() as u64;
    // Like BT: no prepared-source cache. Like CY: provider access follows WHOLE preparation.
    let start = Instant::now();
    let source = case.source.clone();
    let prep = Instant::now();
    let prepared = if native {
        CodeUnit::compile_with_declarations(&source, &inputs, &declarations)
            .map(|u| Prepared::Native(Rc::new(u)))
    } else {
        InterpreterUnit::prepare_with_declarations(&source, &inputs, &declarations)
            .map(|u| Prepared::Tape(Rc::new(u)))
    };
    let prepare_ns = prep.elapsed().as_nanos() as u64;
    let mut memory = None;
    let (route, status, reason, location, stdout, execute_ns, cleanup_ns) = match prepared {
        Err(e) => {
            let route = if e.kind == AdmissionKind::Unsupported {
                "fallback_stub"
            } else {
                "rejected"
            };
            (
                route,
                format!("{:?}", e.kind),
                Some(e.reason),
                e.location,
                String::new(),
                0,
                0,
            )
        }
        Ok(unit) => {
            if diagnostic {
                memory = Some(unit.memory());
            }
            let exec = Instant::now();
            let out = match &unit {
                Prepared::Tape(u) => {
                    interpreter::execute_script(
                        u.clone(),
                        host.clone(),
                        &values,
                        ScriptLimits::default(),
                        Cancellation::default(),
                        trace.clone(),
                    )
                    .await
                }
                Prepared::Native(u) => {
                    runtime::execute_script(
                        u.clone(),
                        host.clone(),
                        &values,
                        ScriptLimits::default(),
                        Cancellation::default(),
                        trace.clone(),
                    )
                    .await
                }
            };
            let execute_ns = exec.elapsed().as_nanos() as u64;
            host.snapshot("execution_end");
            let status = out
                .status
                .as_ref()
                .err()
                .map_or("ok", error_kind)
                .to_owned();
            let reason = out.status.as_ref().err().map(ToString::to_string);
            let stdout = out.stdout.clone();
            let cleanup = Instant::now();
            drop(out);
            drop(unit);
            let cleanup_ns = cleanup.elapsed().as_nanos() as u64;
            host.snapshot("prepared_dropped");
            (
                "executed", status, reason, None, stdout, execute_ns, cleanup_ns,
            )
        }
    };
    drop(source);
    let total_ns = start.elapsed().as_nanos() as u64;
    let log = host.log.lock().unwrap();
    let calls = log.calls.clone();
    let waits = log.waits.clone();
    let replay_matched = !log.mismatch;
    drop(log);
    drop(host);
    drop(declarations);
    drop(inputs);
    drop(values);
    let request_total_ns = request_start.elapsed().as_nanos() as u64;
    let actual = json!({"stdout":stdout,"status":status,"calls":calls});
    let correct = if case.contract_complete == Some(true) && case.oracle_complete == Some(true) {
        case.expected.as_ref().map(|e| *e == actual)
    } else {
        None
    };
    let events = trace.as_ref().map(|t| t.borrow().iter().map(|e| json!({"phase":e.phase,"runtime_buffer_accounting_bytes":e.buffer_bytes,"reported_frame_bytes":e.frame_bytes,"remaining":e.remaining,"index":e.index})).collect::<Vec<_>>());
    let snapshots = ownership
        .as_ref()
        .map(|o| o.lock().unwrap().snapshots.clone());
    json!({"case_id":case.case_id,"backend":if native {"native"} else {"tape"},
        "mode":if diagnostic {"diagnostic"} else {"timing"},"route":route,
        "whole_source_admitted":route=="executed","error":{"category":if status=="ok" {None} else {Some(status)},"reason":reason,"location":location},
        "actual":actual,"correct":correct,"replay_matched":replay_matched,
        "timing":if diagnostic {Value::Null} else {json!({"setup_ns":setup_ns,"request_including_setup_cleanup_ns":request_total_ns,"fresh_source_prepare_execute_cleanup_ns":total_ns,"prepare_ns":prepare_ns,"execute_ns":execute_ns,"prepared_cleanup_ns":cleanup_ns,"tool_waits":waits})},
        "memory":if diagnostic {json!({"prepared":memory,"runtime_events":events,"owner_phases":snapshots,"total_task_state_bytes":null,"rss_pss":null})} else {Value::Null}})
}

#[allow(dead_code)] // Imported by targeted integration tests.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3
        || !["tape", "native"].contains(&args[1].as_str())
        || !["timing", "diagnostic"].contains(&args[2].as_str())
    {
        return Err("di_corpus <tape|native> <timing|diagnostic>; JSONL on stdin".into());
    }
    let runtime_start = Instant::now();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;
    let runtime_setup_ns = runtime_start.elapsed().as_nanos() as u64;
    // One unreported warmup per process, no corpus source/payload warming or cache.
    let warm: Case = serde_json::from_value(json!({"case_id":"warmup","source":"print(1)"}))?;
    let warmup = rt.block_on(run(&warm, args[1] == "native", false));
    assert_eq!(warmup["actual"]["status"], "ok");
    drop(warmup);
    drop(warm);
    println!(
        "{}",
        json!({"kind":"setup","runtime_setup_ns":runtime_setup_ns,"warmup_requests":1})
    );
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let row = match serde_json::from_str::<Case>(&line) {
            Ok(case) => match case.validate_replay() {
                Ok(()) => rt.block_on(run(&case, args[1] == "native", args[2] == "diagnostic")),
                Err(reason) => {
                    json!({"case_id":case.case_id,"route":"fixture_error","reason":reason})
                }
            },
            Err(e) => json!({"route":"fixture_error","reason":e.to_string()}),
        };
        println!("{row}");
    }
    Ok(())
}
