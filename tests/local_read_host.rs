#![cfg(all(feature = "tool-json", not(target_arch = "wasm32")))]
//! A controlled, read-only filesystem Host adapter for whole-script acceptance.
//! This is not the Hermes read_file implementation or its permission model.
use bytes::Bytes;
use minipy_native::{
    buffer::{Quote, QuoteBuffer},
    codegen::CodeUnit,
    error::TaskError,
    interpreter::{self, InterpreterUnit},
    provider::Provider,
    runtime::{self, Cancellation, ScriptLimits},
    tool::{ArgumentSchema, JsonLeaf, Reply, ResultSchema, ToolArguments, ToolDeclaration},
};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

const MAX_FILE: u64 = 4096;
const ID: u32 = 20;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "minipy-read-host-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("notes.txt"), "hello 🌍\n").unwrap();
        fs::write(dir.join("other.txt"), "again\n").unwrap();
        fs::write(dir.join("link.txt"), "other.txt").unwrap();
        fs::write(dir.join("escaped.txt"), "line \"quoted\"\n").unwrap();
        fs::write(dir.join("invalid.txt"), [0xff]).unwrap();
        fs::write(dir.join("large.txt"), vec![b'x'; MAX_FILE as usize + 1]).unwrap();
        fs::write(dir.join("full.txt"), vec![b'x'; 3900]).unwrap();
        std::os::unix::fs::symlink("/etc/hosts", dir.join("escape.txt")).unwrap();
        Self(dir)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct LocalReadHost {
    root: PathBuf,
    calls: Mutex<Vec<String>>,
}
impl LocalReadHost {
    fn new(root: &Path) -> Self {
        Self {
            root: root.canonicalize().unwrap(),
            calls: Mutex::new(Vec::new()),
        }
    }
}
impl Provider for LocalReadHost {
    fn allows_tool(&self, id: u32) -> bool {
        id == ID
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        if id != ID {
            return Err(TaskError::Permission);
        }
        let ToolArguments::String(value) = args else {
            return Err(TaskError::Input);
        };
        let path = value.as_str();
        self.calls.lock().unwrap().push(path.to_owned());
        let relative = Path::new(path);
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(TaskError::Permission);
        }
        let absolute = self
            .root
            .join(relative)
            .canonicalize()
            .map_err(|_| TaskError::Provider("missing file".into()))?;
        if !absolute.starts_with(&self.root) || absolute == self.root {
            return Err(TaskError::Permission);
        }
        let metadata =
            fs::metadata(&absolute).map_err(|_| TaskError::Provider("missing file".into()))?;
        if !metadata.is_file() || metadata.len() > MAX_FILE {
            return Err(TaskError::Budget);
        }
        let bytes = fs::read(&absolute).map_err(|_| TaskError::Provider("read failed".into()))?;
        if bytes.len() > MAX_FILE as usize {
            return Err(TaskError::Budget);
        }
        let text =
            String::from_utf8(bytes).map_err(|_| TaskError::Provider("invalid UTF-8".into()))?;
        let response = serde_json::json!({"content": text, "total_lines": text.lines().count()});
        Ok(Reply::Json(Bytes::from(
            serde_json::to_vec(&response).unwrap(),
        )))
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Err(TaskError::Permission)
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}
fn declaration() -> [ToolDeclaration; 1] {
    [ToolDeclaration {
        id: ID,
        name: "read_file".into(),
        arguments: vec![ArgumentSchema::String("path".into())],
        result: ResultSchema::JsonTyped(vec![
            (vec!["content".into()], JsonLeaf::String),
            (vec!["total_lines".into()], JsonLeaf::I64),
        ]),
    }]
}
#[tokio::test]
async fn whole_scripts_read_real_files_with_bound_host_policy() {
    let fixture = Fixture::new();
    let host = Arc::new(LocalReadHost::new(&fixture.0));
    let cases = [
        (
            "data = tools.read_file(\"notes.txt\")\nprint(data[\"content\"])",
            Ok("hello 🌍\n\n"),
            vec!["notes.txt"],
        ),
        (
            "data = tools.read_file(path=\"notes.txt\")\nprint(\"[\" + data[\"content\"] + \"]\")",
            Ok("[hello 🌍\n]\n"),
            vec!["notes.txt"],
        ),
        (
            "first = tools.read_file(\"link.txt\")\nname = first[\"content\"]\nsecond = tools.read_file(name)\nprint(second[\"content\"])",
            Ok("again\n\n"),
            vec!["link.txt", "other.txt"],
        ),
        (
            "first = tools.read_file(\"notes.txt\")\nn = first[\"total_lines\"]\nnext = tools.read_file(f\"{n}\" + \".txt\")",
            Err(TaskError::Provider("missing file".into())),
            vec!["notes.txt", "1.txt"],
        ),
        (
            "first = tools.read_file(\"notes.txt\")\nprint(f\"[{first['content']}]\")\nsecond = tools.read_file(\"other.txt\")\nprint(\"<\" + second[\"content\"] + \">\")",
            Ok("[hello 🌍\n]\n<again\n>\n"),
            vec!["notes.txt", "other.txt"],
        ),
        (
            "print(1)\ndata = tools.read_file(\"../outside\")",
            Err(TaskError::Permission),
            vec!["../outside"],
        ),
        (
            "print(1)\ndata = tools.read_file(\"escape.txt\")",
            Err(TaskError::Permission),
            vec!["escape.txt"],
        ),
        (
            "print(1)\ndata = tools.read_file(\"/etc/hosts\")",
            Err(TaskError::Permission),
            vec!["/etc/hosts"],
        ),
        (
            "print(1)\ndata = tools.read_file(\"large.txt\")",
            Err(TaskError::Budget),
            vec!["large.txt"],
        ),
        (
            "print(1)\ndata = tools.read_file(\"invalid.txt\")",
            Err(TaskError::Provider("invalid UTF-8".into())),
            vec!["invalid.txt"],
        ),
        (
            "print(1)\ndata = tools.read_file(\"missing.txt\")",
            Err(TaskError::Provider("missing file".into())),
            vec!["missing.txt"],
        ),
        (
            "data = tools.read_file(\"escaped.txt\")\nprint(data[\"content\"])",
            Ok("line \"quoted\"\n\n"),
            vec!["escaped.txt"],
        ),
    ];
    for native in [false, true] {
        for (source, status, paths) in &cases {
            host.calls.lock().unwrap().clear();
            let output = if native {
                let code = Rc::new(
                    CodeUnit::compile_with_declarations(source, &[], &declaration()).unwrap(),
                );
                runtime::execute_script(
                    code,
                    host.clone(),
                    &[],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                )
                .await
            } else {
                let code = Rc::new(
                    InterpreterUnit::prepare_with_declarations(source, &[], &declaration())
                        .unwrap(),
                );
                interpreter::execute_script(
                    code,
                    host.clone(),
                    &[],
                    ScriptLimits::default(),
                    Cancellation::default(),
                    None,
                )
                .await
            };
            let expected = status.as_ref().map(|_| ()).map_err(Clone::clone);
            assert_eq!(output.status, expected, "native={native} source={source}");
            assert_eq!(
                *host.calls.lock().unwrap(),
                *paths,
                "native={native} source={source}"
            );
            if let Ok(text) = status {
                assert_eq!(&output.stdout, text);
            }
            if source.starts_with("print(1)") {
                assert_eq!(output.stdout, "1\n");
            }
        }
    }
}

#[tokio::test]
async fn dead_json_owner_released_before_next_literal_tool_reply() {
    let fixture = Fixture::new();
    let host = Arc::new(LocalReadHost::new(&fixture.0));
    let source = "ignored = tools.read_file(\"full.txt\")\nnext = tools.read_file(\"full.txt\")\nprint(next[\"total_lines\"])";
    let limits = ScriptLimits {
        limits: runtime::Limits {
            buffer_bytes: 5000,
            ..runtime::Limits::default()
        },
        ..ScriptLimits::default()
    };
    for native in [false, true] {
        let outcome = if native {
            runtime::execute_script(
                Rc::new(CodeUnit::compile_with_declarations(source, &[], &declaration()).unwrap()),
                host.clone(),
                &[],
                limits,
                Cancellation::default(),
                None,
            )
            .await
        } else {
            interpreter::execute_script(
                Rc::new(
                    InterpreterUnit::prepare_with_declarations(source, &[], &declaration())
                        .unwrap(),
                ),
                host.clone(),
                &[],
                limits,
                Cancellation::default(),
                None,
            )
            .await
        };
        assert_eq!(outcome.status, Ok(()), "native={native}");
        assert_eq!(outcome.stdout, "1\n");
    }
}

struct MixedHost(LocalReadHost);
impl Provider for MixedHost {
    fn allows_tool(&self, id: u32) -> bool {
        self.0.allows_tool(id)
    }
    async fn call_bound_tool(&self, id: u32, args: ToolArguments) -> Result<Reply, TaskError> {
        self.0.call_bound_tool(id, args).await
    }
    async fn load_quotes(&self, _: i64) -> Result<QuoteBuffer, TaskError> {
        Ok(QuoteBuffer::from_quotes(vec![
            Quote {
                unit_price_cents: 1,
                quantity: 1
            };
            256
        ])
        .unwrap())
    }
    async fn record_total(&self, _: i64, _: i64) -> Result<i64, TaskError> {
        Err(TaskError::Permission)
    }
}

#[tokio::test]
async fn dead_json_owner_released_before_quote_load_budget() {
    let fixture = Fixture::new();
    let host = Arc::new(MixedHost(LocalReadHost::new(&fixture.0)));
    let source =
        "ignored = tools.read_file(\"full.txt\")\nrows = tools.load_quotes(1)\nprint(len(rows))";
    let limits = ScriptLimits {
        limits: runtime::Limits {
            buffer_bytes: 5000,
            ..runtime::Limits::default()
        },
        ..ScriptLimits::default()
    };
    for native in [false, true] {
        let outcome = if native {
            runtime::execute_script(
                Rc::new(CodeUnit::compile_with_declarations(source, &[], &declaration()).unwrap()),
                host.clone(),
                &[],
                limits,
                Cancellation::default(),
                None,
            )
            .await
        } else {
            interpreter::execute_script(
                Rc::new(
                    InterpreterUnit::prepare_with_declarations(source, &[], &declaration())
                        .unwrap(),
                ),
                host.clone(),
                &[],
                limits,
                Cancellation::default(),
                None,
            )
            .await
        };
        assert_eq!(outcome.status, Ok(()), "native={native}");
        assert_eq!(outcome.stdout, "256\n");
    }
}
