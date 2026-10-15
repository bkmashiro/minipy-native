//! Preparation-owned Tool declarations and statically bound JSON paths.
use crate::ir::{AdmissionKind, CompileError};

pub const LOAD_QUOTES: u32 = 0;
pub const RECORD_TOTAL: u32 = 1;
pub const MAX_TOOLS: usize = 32;
pub const MAX_PATHS: usize = 128;

#[derive(Clone, Debug)]
pub enum ResultSchema {
    I64,
    /// Explicit i64 leaf paths. Other paths may form raw subviews, never values.
    Json(Vec<Vec<String>>),
    /// Explicit typed leaves; strings support literal `in` and bounded `print`.
    JsonTyped(Vec<(Vec<String>, JsonLeaf)>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonLeaf {
    I64,
    String,
}
#[derive(Clone, Debug)]
pub struct ToolSchema {
    pub id: u32,
    pub name: String,
    pub arguments: Vec<String>,
    pub result: ResultSchema,
}
impl ToolSchema {
    pub fn validate_all(tools: &[Self]) -> Result<(), CompileError> {
        if tools.len() > MAX_TOOLS {
            return Err(CompileError::new(
                AdmissionKind::Schema,
                "too many Tool declarations",
            ));
        }
        let mut ids = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for t in tools {
            let identifier = |s: &str| {
                !s.is_empty()
                    && s.chars().enumerate().all(|(i, c)| {
                        c == '_' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit()
                    })
            };
            if t.id < 2
                || !ids.insert(t.id)
                || !names.insert(&t.name)
                || !identifier(&t.name)
                || ["load_quotes", "record_total"].contains(&t.name.as_str())
            {
                return Err(CompileError::new(
                    AdmissionKind::Schema,
                    "Tool IDs/names must be unique and non-reserved",
                ));
            }
            let mut args = std::collections::HashSet::new();
            if t.arguments.len() > 2
                || t.arguments
                    .iter()
                    .any(|a| !identifier(a) || !args.insert(a))
            {
                return Err(CompileError::new(
                    AdmissionKind::Schema,
                    "Tools accept at most two unique named i64 arguments",
                ));
            }
            let paths: Vec<&Vec<String>> = match &t.result {
                ResultSchema::Json(paths) => paths.iter().collect(),
                ResultSchema::JsonTyped(paths) => paths.iter().map(|(p, _)| p).collect(),
                ResultSchema::I64 => Vec::new(),
            };
            if !paths.is_empty() {
                if paths.len() > MAX_PATHS
                    || paths
                        .iter()
                        .any(|p| p.is_empty() || p.len() > 32 || p.iter().any(|k| k.len() > 256))
                {
                    return Err(CompileError::new(
                        AdmissionKind::Schema,
                        "JSON schema exceeds bounded leaf paths",
                    ));
                }
                for (i, p) in paths.iter().enumerate() {
                    if paths[..i]
                        .iter()
                        .any(|q| p.starts_with(q) || q.starts_with(p))
                    {
                        return Err(CompileError::new(
                            AdmissionKind::Schema,
                            "JSON leaf paths overlap",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct BoundCall {
    pub id: u32,
    pub arguments: usize,
    pub json: bool,
}
#[derive(Clone, Debug)]
pub struct PathSlot {
    pub root: usize,
    pub parent: Option<usize>,
    pub key: String,
}
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub roots: usize,
    pub paths: Vec<PathSlot>,
    pub calls: Vec<BoundCall>,
    pub needles: Vec<String>,
    pub string_paths: bool,
}
impl Plan {
    pub(crate) fn owned_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.calls.capacity() * std::mem::size_of::<BoundCall>()
            + self.paths.capacity() * std::mem::size_of::<PathSlot>()
            + self.paths.iter().map(|p| p.key.capacity()).sum::<usize>()
            + self.needles.capacity() * std::mem::size_of::<String>()
            + self.needles.iter().map(String::capacity).sum::<usize>()
    }
    pub(crate) fn path(&mut self, root: usize, keys: &[String]) -> Result<usize, CompileError> {
        let mut parent = None;
        for key in keys {
            let slot = if let Some(i) = self
                .paths
                .iter()
                .position(|p| p.root == root && p.parent == parent && &p.key == key)
            {
                i
            } else {
                if self.paths.len() >= MAX_PATHS {
                    return Err(CompileError::new(
                        AdmissionKind::Schema,
                        "too many bound JSON path slots",
                    ));
                }
                let i = self.paths.len();
                self.paths.push(PathSlot {
                    root,
                    parent,
                    key: key.clone(),
                });
                i
            };
            parent = Some(slot);
        }
        parent.ok_or_else(|| {
            CompileError::new(
                AdmissionKind::Schema,
                "JSON scalar access requires a leaf path",
            )
        })
    }
}
/// Capability-facing reply. Bytes is transferred by ownership, never reconstructed.
pub enum Reply {
    I64(i64),
    Json(bytes::Bytes),
}
