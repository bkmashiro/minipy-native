//! Preparation-owned Tool declarations and statically bound JSON paths.
use crate::ir::{AdmissionKind, CompileError};
use std::{ops::Range, sync::Arc};

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
        validate_tools(tools.iter().map(|t| {
            (
                t.id,
                t.name.as_str(),
                t.arguments.iter().map(String::as_str),
                &t.result,
            )
        }))
    }
}

fn validate_tools<'a, A: ExactSizeIterator<Item = &'a str>>(
    tools: impl ExactSizeIterator<Item = (u32, &'a str, A, &'a ResultSchema)>,
) -> Result<(), CompileError> {
    if tools.len() > MAX_TOOLS {
        return Err(CompileError::new(
            AdmissionKind::Schema,
            "too many Tool declarations",
        ));
    }
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for (id, name, mut arguments, result) in tools {
        let identifier = |s: &str| {
            !s.is_empty()
                && s.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit()
                })
        };
        if id < 2
            || !ids.insert(id)
            || !names.insert(name)
            || !identifier(name)
            || ["load_quotes", "record_total"].contains(&name)
        {
            return Err(CompileError::new(
                AdmissionKind::Schema,
                "Tool IDs/names must be unique and non-reserved",
            ));
        }
        let mut args = std::collections::HashSet::new();
        if arguments.len() > 2 || arguments.any(|a| !identifier(a) || !args.insert(a)) {
            return Err(CompileError::new(
                AdmissionKind::Schema,
                "Tools accept at most two unique named i64 arguments",
            ));
        }
        let paths: Vec<&Vec<String>> = match result {
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
/// New declarations share the finite name/ID namespace with legacy numeric Tools.
/// The first string slice admits exactly one named string argument.
#[derive(Clone, Debug)]
pub enum ArgumentSchema {
    I64(String),
    String(String),
}
impl ArgumentSchema {
    pub fn name(&self) -> &str {
        match self {
            Self::I64(n) | Self::String(n) => n,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ToolDeclaration {
    pub id: u32,
    pub name: String,
    pub arguments: Vec<ArgumentSchema>,
    pub result: ResultSchema,
}
/// Immutable Tool declarations validated once at Host registration.
/// This describes types, not permission to invoke a Tool.
#[derive(Debug)]
pub struct ToolEnvironment {
    declarations: Vec<ToolDeclaration>,
}
impl ToolEnvironment {
    pub fn new(declarations: Vec<ToolDeclaration>) -> Result<Self, CompileError> {
        ToolDeclaration::validate_all(&declarations)?;
        Ok(Self { declarations })
    }
    pub(crate) fn declarations(&self) -> &[ToolDeclaration] {
        &self.declarations
    }
}
impl From<&ToolSchema> for ToolDeclaration {
    fn from(value: &ToolSchema) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            arguments: value
                .arguments
                .iter()
                .cloned()
                .map(ArgumentSchema::I64)
                .collect(),
            result: value.result.clone(),
        }
    }
}
impl ToolDeclaration {
    pub fn validate_all(tools: &[Self]) -> Result<(), CompileError> {
        validate_tools(tools.iter().map(|t| {
            (
                t.id,
                t.name.as_str(),
                t.arguments.iter().map(ArgumentSchema::name),
                &t.result,
            )
        }))?;
        for tool in tools {
            let strings = tool
                .arguments
                .iter()
                .filter(|a| matches!(a, ArgumentSchema::String(_)))
                .count();
            if strings > 0 && (strings != 1 || tool.arguments.len() != 1) {
                return Err(CompileError::new(
                    AdmissionKind::Schema,
                    "string Tool requires one string argument",
                ));
            }
        }
        Ok(())
    }
}
/// An operation-owned string view. The borrowed JSON span is validated before
/// the owner is cloned; decoded escapes share their already allocated cache.
#[derive(Clone, Debug)]
pub enum StringValue {
    Literal(Arc<str>),
    Json {
        owner: bytes::Bytes,
        range: Range<usize>,
    },
    Decoded(Arc<String>),
    Joined(String),
}
impl StringValue {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Literal(s) => s,
            Self::Decoded(s) => s,
            Self::Joined(s) => s,
            Self::Json { owner, range } => {
                // The runtime constructs this span only after validation. Keep
                // public values memory-safe even if a Host constructs one itself.
                std::str::from_utf8(owner.get(range.clone()).unwrap_or_default()).unwrap_or("")
            }
        }
    }
}
pub enum ToolArguments {
    Numeric([i64; 2], usize),
    String(StringValue),
}
#[derive(Clone, Debug)]
pub enum StringSource {
    Literal(Arc<str>),
    Json { slot: usize, root: usize },
    JsonInteger { slot: usize, root: usize },
    Scalar { slot: usize, boolean: bool },
    Joined(Vec<StringSource>),
}
impl StringSource {
    pub(crate) fn roots(&self) -> u32 {
        match self {
            Self::Json { root, .. } | Self::JsonInteger { root, .. } => 1 << root,
            Self::Joined(parts) => parts.iter().fold(0, |mask, p| mask | p.roots()),
            _ => 0,
        }
    }
    pub(crate) fn scalars(&self) -> u32 {
        match self {
            Self::Scalar { slot, .. } => 1 << slot,
            Self::Joined(parts) => parts.iter().fold(0, |mask, p| mask | p.scalars()),
            _ => 0,
        }
    }
    pub(crate) fn owned_bytes(&self) -> usize {
        match self {
            Self::Literal(s) => s.len(),
            Self::Joined(parts) => {
                parts.capacity() * std::mem::size_of::<Self>()
                    + parts.iter().map(Self::owned_bytes).sum::<usize>()
            }
            _ => 0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct BoundCall {
    pub id: u32,
    pub arguments: usize,
    pub json: bool,
    pub string: Option<StringSource>,
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
    /// Literal operands for contains and exact string equality.
    pub needles: Vec<String>,
    pub string_paths: bool,
    pub prints: Vec<StringSource>,
}
impl Plan {
    pub(crate) fn owned_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.calls.capacity() * std::mem::size_of::<BoundCall>()
            + self
                .calls
                .iter()
                .map(|c| c.string.as_ref().map_or(0, StringSource::owned_bytes))
                .sum::<usize>()
            + self.prints.capacity() * std::mem::size_of::<StringSource>()
            + self
                .prints
                .iter()
                .map(StringSource::owned_bytes)
                .sum::<usize>()
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
