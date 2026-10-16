//! Restricted Python source frontend: RustPython AST -> typed semantic IR.
use crate::ir::{self, AdmissionKind, BinOp, CompileError, Expr, Field, Program, Stmt, Type};
use rustpython_parser::{Parse, ast, ast::Ranged};
use std::collections::{HashMap, HashSet};

#[cfg(all(test, feature = "tool-json", not(target_arch = "wasm32")))]
mod schema_probe;

const MAX_DEPTH: usize = 32;
type Result<T> = std::result::Result<T, CompileError>;
fn err(message: impl Into<String>) -> CompileError {
    CompileError::new(AdmissionKind::Unsupported, message)
}
fn type_err(message: impl Into<String>) -> CompileError {
    CompileError::new(AdmissionKind::Type, message)
}
fn resource_err(message: impl Into<String>) -> CompileError {
    CompileError::new(AdmissionKind::Resource, message)
}

/// Parse with the historical two scalar inputs.
pub fn parse(source: &str) -> Result<Program> {
    parse_with_inputs(
        source,
        &[("batch_id", Type::I64), ("price_limit", Type::I64)],
    )
}

/// Parse a restricted script with an explicitly declared scalar input schema.
pub fn parse_with_inputs(source: &str, inputs: &[(&str, Type)]) -> Result<Program> {
    if source.len() > ir::MAX_SOURCE_BYTES {
        return Err(resource_err("source exceeds MAX_SOURCE_BYTES"));
    }
    check_delimiter_depth(source)?;
    validate_inputs(inputs)?;
    #[cfg(feature = "compiler-profile")]
    let start = std::time::Instant::now();
    let module = ast::Suite::parse(source, "<minipy>").map_err(|e| {
        CompileError::new(AdmissionKind::Syntax, format!("Python syntax error: {e}"))
            .at(source, e.offset.to_usize())
    })?;
    #[cfg(feature = "compiler-profile")]
    crate::compiler_profile::update(|p| p.parse_ns = crate::compiler_profile::elapsed(start));
    #[cfg(feature = "compiler-profile")]
    let start = std::time::Instant::now();
    let result = Compiler::new(inputs, source)?.program(module);
    #[cfg(feature = "compiler-profile")]
    crate::compiler_profile::update(|p| p.type_ns = crate::compiler_profile::elapsed(start));
    result
}

/// Same parser/typechecker with a Host-declared finite Tool table.
#[cfg(feature = "tool-json")]
pub fn parse_with_tools(
    source: &str,
    inputs: &[(&str, Type)],
    tools: &[crate::tool::ToolSchema],
) -> Result<Program> {
    parse_with_declarations(
        source,
        inputs,
        &tools.iter().map(Into::into).collect::<Vec<_>>(),
    )
}
#[cfg(feature = "tool-json")]
pub fn parse_with_declarations(
    source: &str,
    inputs: &[(&str, Type)],
    tools: &[crate::tool::ToolDeclaration],
) -> Result<Program> {
    crate::tool::ToolDeclaration::validate_all(tools)?;
    parse_validated_tools(source, inputs, tools)
}
#[cfg(feature = "tool-json")]
pub(crate) fn parse_with_environment(
    source: &str,
    inputs: &[(&str, Type)],
    environment: &crate::tool::ToolEnvironment,
) -> Result<Program> {
    parse_validated_tools(source, inputs, environment.declarations())
}
#[cfg(feature = "tool-json")]
fn parse_validated_tools(
    source: &str,
    inputs: &[(&str, Type)],
    tools: &[crate::tool::ToolDeclaration],
) -> Result<Program> {
    if source.len() > ir::MAX_SOURCE_BYTES {
        return Err(resource_err("source exceeds MAX_SOURCE_BYTES"));
    }
    check_delimiter_depth(source)?;
    validate_inputs(inputs)?;
    #[cfg(feature = "compiler-profile")]
    let start = std::time::Instant::now();
    let module = ast::Suite::parse(source, "<minipy>").map_err(|e| {
        CompileError::new(AdmissionKind::Syntax, format!("Python syntax error: {e}"))
            .at(source, e.offset.to_usize())
    });
    #[cfg(feature = "compiler-profile")]
    crate::compiler_profile::update(|p| p.parse_ns = crate::compiler_profile::elapsed(start));
    let module = module?;
    #[cfg(feature = "compiler-profile")]
    let start = std::time::Instant::now();
    let mut compiler = Compiler::new(inputs, source)?;
    compiler.tools = tools;
    let result = compiler.program(module);
    #[cfg(feature = "compiler-profile")]
    crate::compiler_profile::update(|p| p.type_ns = crate::compiler_profile::elapsed(start));
    result
}

fn validate_inputs(inputs: &[(&str, Type)]) -> Result<()> {
    if inputs.len() > ir::MAX_LOCALS {
        return Err(CompileError::new(
            AdmissionKind::Schema,
            "input schema exceeds MAX_LOCALS",
        ));
    }
    let mut names = HashSet::new();
    for (name, _) in inputs {
        if name.is_empty() || !names.insert(*name) {
            return Err(CompileError::new(
                AdmissionKind::Schema,
                "input names must be unique identifiers",
            ));
        }
        if !name
            .chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_alphanumeric() && (i != 0 || !c.is_numeric()))
            || [
                "tools", "print", "quotes", "True", "False", "i64", "bool", "len", "abs", "min",
                "max",
            ]
            .contains(name)
        {
            return Err(CompileError::new(
                AdmissionKind::Schema,
                format!("reserved or invalid input name '{name}'"),
            ));
        }
    }
    Ok(())
}

// Bound deep expressions and suites before RustPython recursively parses them.
fn check_delimiter_depth(s: &str) -> Result<()> {
    if s.lines().any(|line| {
        line.chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .map(|c| if c == '\t' { 8 } else { 1 })
            .sum::<usize>()
            > MAX_DEPTH * 4
    }) {
        return Err(resource_err(
            "statement nesting exceeds the pre-parser limit",
        ));
    }
    let (mut depth, mut quote, mut escaped, mut comment) = (0usize, None, false, false);
    for c in s.chars() {
        if comment {
            if c == '\n' {
                comment = false;
            }
            continue;
        }
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '#' => comment = true,
            '\'' | '"' => quote = Some(c),
            '(' | '[' | '{' => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(resource_err("expression nesting exceeds maximum depth 32"));
                }
            }
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

// These views live only during preparation. The resulting Plan owns its bound paths.
#[cfg(feature = "tool-json")]
struct SchemaLeaves<'a>(&'a crate::tool::ResultSchema);
#[cfg(feature = "tool-json")]
impl SchemaLeaves<'_> {
    fn iter(&self) -> impl Iterator<Item = (&Vec<String>, &crate::tool::JsonLeaf)> {
        use crate::tool::{JsonLeaf, ResultSchema};
        let (numeric, typed): (&[Vec<String>], &[(Vec<String>, JsonLeaf)]) = match self.0 {
            ResultSchema::Json(paths) => (paths, &[]),
            ResultSchema::JsonTyped(paths) => (&[], paths),
            ResultSchema::I64 => (&[], &[]),
            ResultSchema::JsonObjectArray(_) => (&[], &[]),
        };
        numeric
            .iter()
            .map(|p| (p, &JsonLeaf::I64))
            .chain(typed.iter().map(|(p, t)| (p, t)))
    }
}

struct Compiler<'a> {
    source: &'a str,
    nodes: usize,
    locals: Vec<Type>,
    names: Vec<String>,
    initialized: u32,
    buffers: HashMap<String, usize>,
    rows: HashSet<String>,
    next_buffer: usize,
    inputs: usize,
    legacy: bool,
    #[cfg(feature = "tool-json")]
    tools: &'a [crate::tool::ToolDeclaration],
    #[cfg(feature = "tool-json")]
    json: crate::tool::Plan,
    #[cfg(feature = "tool-json")]
    json_names: HashMap<String, (usize, Vec<String>)>,
    #[cfg(feature = "tool-json")]
    json_leaves: Vec<SchemaLeaves<'a>>,
    #[cfg(feature = "tool-json")]
    json_row: Option<(String, usize, Vec<String>)>,
    #[cfg(feature = "tool-json")]
    line: Option<(String, usize)>,
}
impl<'a> Compiler<'a> {
    fn new(inputs: &[(&str, Type)], source: &'a str) -> Result<Self> {
        if inputs.len() > ir::MAX_LOCALS {
            return Err(resource_err("program exceeds MAX_LOCALS"));
        }
        Ok(Self {
            source,
            nodes: 0,
            locals: inputs.iter().map(|x| x.1).collect(),
            names: inputs.iter().map(|x| x.0.to_owned()).collect(),
            initialized: ((1u64 << inputs.len()) - 1) as u32,
            buffers: HashMap::new(),
            rows: HashSet::new(),
            next_buffer: 0,
            inputs: inputs.len(),
            legacy: false,
            #[cfg(feature = "tool-json")]
            tools: &[],
            #[cfg(feature = "tool-json")]
            json: crate::tool::Plan::default(),
            #[cfg(feature = "tool-json")]
            json_names: HashMap::new(),
            #[cfg(feature = "tool-json")]
            json_leaves: Vec::new(),
            #[cfg(feature = "tool-json")]
            json_row: None,
            #[cfg(feature = "tool-json")]
            line: None,
        })
    }
    fn node(&mut self, depth: usize) -> Result<()> {
        self.nodes += 1;
        if self.nodes > ir::MAX_NODES {
            return Err(resource_err("AST exceeds MAX_NODES"));
        }
        if depth > MAX_DEPTH {
            return Err(resource_err("AST nesting exceeds maximum depth 32"));
        }
        Ok(())
    }
    fn program(mut self, suite: ast::Suite) -> Result<Program> {
        self.node(0)?;
        let (body, legacy_args) = if suite.len() == 1 {
            if let ast::Stmt::AsyncFunctionDef(fun) = &suite[0] {
                self.legacy = true;
                if fun.name.as_str() != "main"
                    || !fun.decorator_list.is_empty()
                    || !fun.type_params.is_empty()
                    || fun.type_comment.is_some()
                {
                    return Err(err("only undecorated async def main is supported")
                        .at(self.source, fun.start().to_usize()));
                }
                self.check_legacy_signature(&fun.args)
                    .map_err(|e| e.at(self.source, fun.start().to_usize()))?;
                if !matches!(fun.returns.as_deref(), Some(ast::Expr::Name(n)) if ["i64", "int"].contains(&n.id.as_str()))
                {
                    return Err(err("legacy main must return int/i64").at(
                        self.source,
                        fun.returns
                            .as_deref()
                            .map_or_else(|| fun.start().to_usize(), |e| e.start().to_usize()),
                    ));
                }
                (&fun.body[..], true)
            } else {
                (&suite[..], false)
            }
        } else {
            (&suite[..], false)
        };
        let _ = legacy_args;
        if body.is_empty() {
            return self.finish(vec![Stmt::Return(Expr::Int(0))]);
        }
        let mut statements = Vec::new();
        for (i, statement) in body.iter().enumerate() {
            let terminal = i + 1 == body.len();
            if let ast::Stmt::Return(r) = statement {
                if !self.legacy || !terminal {
                    return Err(err(
                        "return is only supported as the final statement of async main",
                    )
                    .at(self.source, r.start().to_usize()));
                }
                let value = r.value.as_deref().ok_or_else(|| {
                    err("return requires an expression").at(self.source, r.start().to_usize())
                })?;
                self.compile_legacy_return(value, 0, &mut statements)
                    .map_err(|e| e.at(self.source, value.start().to_usize()))?;
                continue;
            }
            self.compile_stmt(statement, 0, false, false, &mut statements)?;
        }
        if !matches!(statements.last(), Some(Stmt::Return(_))) {
            statements.push(Stmt::Return(Expr::Int(0)));
        }
        self.finish(statements)
    }
    fn finish(self, statements: Vec<Stmt>) -> Result<Program> {
        if self.locals.len() > ir::MAX_LOCALS {
            return Err(resource_err("program exceeds MAX_LOCALS"));
        }
        Ok(Program {
            locals: self.locals,
            inputs: self.inputs,
            buffers: self.next_buffer,
            statements,
            legacy: self.legacy,
            #[cfg(feature = "tool-json")]
            json: self.json,
        })
    }
    fn check_legacy_signature(&self, args: &ast::Arguments) -> Result<()> {
        if !args.posonlyargs.is_empty()
            || args.args.len() != self.inputs
            || args.vararg.is_some()
            || !args.kwonlyargs.is_empty()
            || args.kwarg.is_some()
        {
            return Err(err(
                "async main parameters must match the declared input schema",
            ));
        }
        for (a, n) in args.args.iter().zip(&self.names) {
            if a.default.is_some()
                || a.def.arg.as_str() != n
                || !matches!(a.def.annotation.as_deref(), Some(ast::Expr::Name(x)) if (x.id.as_str()=="i64" || x.id.as_str()=="int") && self.locals[self.names.iter().position(|v|v==n).unwrap()]==Type::I64)
                    && !matches!(a.def.annotation.as_deref(), Some(ast::Expr::Name(x)) if x.id.as_str()=="bool" && self.locals[self.names.iter().position(|v|v==n).unwrap()]==Type::Bool)
            {
                return Err(err(
                    "async main signature must match the declared input schema",
                ));
            }
        }
        Ok(())
    }
    fn target_name(e: &ast::Expr) -> Result<String> {
        if let ast::Expr::Name(n) = e {
            Ok(n.id.to_string())
        } else {
            Err(err("only simple local names may be assigned"))
        }
    }
    fn lookup(&self, name: &str) -> Result<usize> {
        self.names
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| err(format!("unknown local '{name}'")))
    }
    fn declare(&mut self, name: &str, ty: Type, allow_existing: bool) -> Result<usize> {
        #[cfg(feature = "tool-json")]
        if self.json_names.contains_key(name) || self.line.as_ref().is_some_and(|l| l.0 == name) {
            return Err(err("JSON view names cannot be rebound"));
        }
        if [
            "tools", "print", "True", "False", "len", "abs", "min", "max",
        ]
        .contains(&name)
            || self.buffers.contains_key(name)
            || self.rows.contains(name)
        {
            return Err(err(format!("cannot assign reserved name '{name}'")));
        }
        if let Some(i) = self.names.iter().position(|n| n == name) {
            if i < self.inputs {
                return Err(err(format!("host input '{name}' is immutable")));
            }
            if self.locals[i] != ty {
                return Err(type_err(format!("local '{name}' changes type")));
            }
            if !allow_existing && self.initialized & (1 << i) != 0 {
                return Err(err(format!("local '{name}' is already initialized")));
            }
            return Ok(i);
        }
        if self.names.len() >= ir::MAX_LOCALS {
            return Err(resource_err("program exceeds MAX_LOCALS"));
        }
        let i = self.names.len();
        self.names.push(name.to_owned());
        self.locals.push(ty);
        Ok(i)
    }
    fn compile_stmt(
        &mut self,
        s: &ast::Stmt,
        depth: usize,
        in_loop: bool,
        in_loop_body: bool,
        out: &mut Vec<Stmt>,
    ) -> Result<()> {
        self.compile_stmt_inner(s, depth, in_loop, in_loop_body, out)
            .map_err(|e| e.at(self.source, s.start().to_usize()))
    }
    fn compile_stmt_inner(
        &mut self,
        s: &ast::Stmt,
        depth: usize,
        in_loop: bool,
        in_loop_body: bool,
        out: &mut Vec<Stmt>,
    ) -> Result<()> {
        self.node(depth)?;
        match s {
            ast::Stmt::Assign(a) if a.targets.len() == 1 && a.type_comment.is_none() => {
                let name = Self::target_name(&a.targets[0])?;
                #[cfg(feature = "tool-json")]
                {
                    if self.compile_tool(&name, &a.value, depth + 1, in_loop, out)? {
                        return Ok(());
                    }
                    if let Some((root, path)) = self.json_path(&a.value)?
                        && (path.is_empty()
                            || !self.json_leaves[root].iter().any(|(p, _)| p == &path))
                    {
                        if in_loop {
                            return Err(err("JSON view aliases are unsupported inside loops"));
                        }
                        if !path.is_empty() {
                            let slot = self.json.path(root, &path)?;
                            out.push(Stmt::Eval(Expr::JsonView { slot, root }));
                        }
                        self.bind_json(&name, root, path)?;
                        return Ok(());
                    }
                    if let Some((root, path)) = self.json_path(&a.value)?
                        && self.json_leaves[root]
                            .iter()
                            .any(|(p, t)| p == &path && *t == crate::tool::JsonLeaf::String)
                    {
                        if in_loop {
                            return Err(err("JSON string aliases are unsupported inside loops"));
                        }
                        let slot = self.json.path(root, &path)?;
                        self.json.string_paths = true;
                        out.push(Stmt::Eval(Expr::JsonString { slot, root }));
                        self.bind_json(&name, root, path)?;
                        return Ok(());
                    }
                }
                if Self::is_tool(&a.value, "record_total") {
                    let (batch, total) = self
                        .tool_call(&a.value, "record_total", depth + 1, in_loop)?
                        .unwrap();
                    if in_loop {
                        return Err(err("tool calls are unsupported inside loops"));
                    }
                    let i = self.declare(&name, Type::I64, true)?;
                    self.initialized |= 1 << i;
                    out.push(Stmt::Record(i, batch, total));
                    return Ok(());
                }
                if Self::is_tool(&a.value, "load_quotes") {
                    if in_loop {
                        return Err(err("tool calls are unsupported inside loops"));
                    }
                    self.node(depth + 1)?;
                    let args = Self::call_args(&a.value, "load_quotes")?;
                    if args.len() != 1 {
                        return Err(err("tools.load_quotes expects one positional argument"));
                    }
                    let (batch, ty) = self.expr(&args[0], depth + 2, false)?;
                    if ty != Type::I64 {
                        return Err(type_err("load_quotes batch expression must be i64"));
                    }
                    if self.next_buffer >= ir::MAX_BUFFERS {
                        return Err(resource_err("program exceeds 32 quote buffers"));
                    }
                    if self.names.iter().any(|n| n == &name)
                        || self.buffers.contains_key(&name)
                        || ["tools", "print", "len", "abs", "min", "max"].contains(&name.as_str())
                    {
                        return Err(err("buffer name conflicts with a reserved or scalar name"));
                    }
                    #[cfg(feature = "tool-json")]
                    if self.json_names.contains_key(&name) {
                        return Err(err("buffer name conflicts with JSON view"));
                    }
                    let slot = self.next_buffer;
                    self.next_buffer += 1;
                    self.buffers.insert(name, slot);
                    out.push(Stmt::Load(slot, batch));
                    return Ok(());
                }
                let expected = self
                    .names
                    .iter()
                    .position(|n| n == &name)
                    .map(|i| self.locals[i]);
                let (value, ty) = self.expr(&a.value, depth + 1, in_loop)?;
                if let Some(expected) = expected
                    && expected != ty
                {
                    return Err(type_err(format!(
                        "initializer for '{name}' has the wrong type"
                    )));
                }
                let i = self.declare(&name, ty, true)?;
                self.initialized |= 1 << i;
                out.push(Stmt::Set(i, value));
                Ok(())
            }
            ast::Stmt::AnnAssign(a) if a.simple => {
                let name = Self::target_name(&a.target)?;
                let ty = Self::annotation(&a.annotation)?;
                let value = a
                    .value
                    .as_deref()
                    .ok_or_else(|| err("locals must be initialized at declaration"))?;
                let (value, actual) = self.expr(value, depth + 1, in_loop)?;
                if ty != actual {
                    return Err(type_err(format!(
                        "initializer for '{name}' has the wrong type"
                    )));
                }
                let i = self.declare(&name, ty, true)?;
                self.initialized |= 1 << i;
                out.push(Stmt::Set(i, value));
                Ok(())
            }
            ast::Stmt::AugAssign(a) => {
                let n = Self::target_name(&a.target)?;
                let i = self.lookup(&n)?;
                if i < self.inputs || self.initialized & (1 << i) == 0 {
                    return Err(err(format!("local '{n}' is immutable or uninitialized")));
                }
                if self.locals[i] != Type::I64 {
                    return Err(type_err("augmented arithmetic requires i64"));
                }
                let op = match a.op {
                    ast::Operator::Add => BinOp::Add,
                    ast::Operator::Sub => BinOp::Sub,
                    ast::Operator::Mult => BinOp::Mul,
                    _ => return Err(err("only +=, -=, and *= are supported")),
                };
                let (rhs, ty) = self.expr(&a.value, depth + 1, in_loop)?;
                if ty != Type::I64 {
                    return Err(type_err("augmented arithmetic requires i64"));
                }
                out.push(Stmt::Set(
                    i,
                    Expr::Binary(op, Box::new(Expr::Local(i)), Box::new(rhs)),
                ));
                Ok(())
            }
            ast::Stmt::If(a) => {
                let (test, ty) = self.expr(&a.test, depth + 1, in_loop)?;
                if ty != Type::Bool {
                    return Err(type_err("if condition must be bool"));
                }
                let buffers_before = self.next_buffer;
                #[cfg(feature = "tool-json")]
                let json_before = self.json_names.len();
                let incoming = self.initialized;
                let then_body = self.compile_block(&a.body, depth + 1, in_loop, in_loop_body)?;
                #[cfg(feature = "tool-json")]
                if self.json_names.len() != json_before {
                    return Err(err("JSON owner/view bindings must be unconditional"));
                }
                if self.next_buffer != buffers_before {
                    return Err(err("buffer loads must be unconditional"));
                }
                let then_set = self.initialized;
                self.initialized = incoming;
                let else_body = self.compile_block(&a.orelse, depth + 1, in_loop, in_loop_body)?;
                #[cfg(feature = "tool-json")]
                if self.json_names.len() != json_before {
                    return Err(err("JSON owner/view bindings must be unconditional"));
                }
                if self.next_buffer != buffers_before {
                    return Err(err("buffer loads must be unconditional"));
                }
                self.initialized &= then_set;
                out.push(Stmt::If(test, then_body, else_body));
                Ok(())
            }
            #[cfg(feature = "tool-json")]
            ast::Stmt::Break(_) if self.line.is_some() => {
                out.push(Stmt::LineBreak);
                Ok(())
            }
            #[cfg(feature = "tool-json")]
            ast::Stmt::Break(_) if self.json_row.is_some() => {
                out.push(Stmt::JsonBreak);
                Ok(())
            }
            ast::Stmt::For(f) => {
                if in_loop {
                    return Err(err("nested loops are unsupported"));
                }
                if !f.orelse.is_empty() {
                    return Err(err("for-else is unsupported"));
                }
                #[cfg(feature = "tool-json")]
                if let ast::Expr::Call(c) = f.iter.as_ref()
                    && let ast::Expr::Attribute(method) = c.func.as_ref()
                    && method.attr.as_str() == "splitlines"
                {
                    if !c.args.is_empty() || !c.keywords.is_empty() {
                        return Err(err("splitlines accepts no arguments in this subset"));
                    }
                    if !self.is_json_string(&method.value)? {
                        return Err(err("splitlines requires a declared JSON string leaf"));
                    }
                    let (root, path) = self.json_path(&method.value)?.expect("string leaf");
                    let name = Self::target_name(&f.target)?;
                    if self.names.contains(&name)
                        || self.buffers.contains_key(&name)
                        || self.json_names.contains_key(&name)
                        || ["tools", "print", "len", "abs", "min", "max"].contains(&name.as_str())
                    {
                        return Err(err("line name conflicts with existing/reserved binding"));
                    }
                    let slot = self.json.path(root, &path)?;
                    self.line = Some((name, root));
                    let before = self.initialized;
                    let body = self.compile_block(&f.body, depth + 1, true, true)?;
                    self.initialized = before;
                    self.line = None;
                    self.json.string_paths = true;
                    out.push(Stmt::LineFor { slot, root, body });
                    return Ok(());
                }
                #[cfg(feature = "tool-json")]
                if let Some((root, path)) = self.json_path(&f.iter)? {
                    let crate::tool::ResultSchema::JsonObjectArray(schema) =
                        self.json_leaves[root].0
                    else {
                        return Err(err("for requires a declared JSON object array"));
                    };
                    if path != schema.path {
                        return Err(err("for requires the declared array path"));
                    }
                    let row = Self::target_name(&f.target)?;
                    if self.names.contains(&row)
                        || self.buffers.contains_key(&row)
                        || self.json_names.contains_key(&row)
                        || ["tools", "print", "len", "abs", "min", "max"].contains(&row.as_str())
                    {
                        return Err(err(
                            "array row name conflicts with existing/reserved binding",
                        ));
                    }
                    let slot = self.json.path(root, &path)?;
                    self.rows.insert(row.clone());
                    self.json_row = Some((row.clone(), root, path));
                    let before = self.initialized;
                    let body = self.compile_block(&f.body, depth + 1, true, true)?;
                    self.initialized = before;
                    self.json_row = None;
                    self.rows.remove(&row);
                    out.push(Stmt::JsonFor { slot, root, body });
                    self.json.array_paths = true;
                    return Ok(());
                }
                let ast::Expr::Name(buf_name) = f.iter.as_ref() else {
                    return Err(err("for must iterate over a loaded quote buffer"));
                };
                let slot = *self
                    .buffers
                    .get(buf_name.id.as_str())
                    .ok_or_else(|| err("for must iterate over a previously loaded buffer"))?;
                let row = Self::target_name(&f.target)?;
                if self.names.iter().any(|n| n == &row)
                    || self.buffers.contains_key(&row)
                    || ["tools", "print", "len", "abs", "min", "max"].contains(&row.as_str())
                {
                    return Err(err(
                        "row name conflicts with a scalar, buffer, or reserved name",
                    ));
                }
                #[cfg(feature = "tool-json")]
                if self.json_names.contains_key(&row) {
                    return Err(err("row name conflicts with JSON view"));
                }
                self.rows.insert(row.clone());
                let before = self.initialized;
                let compiled = self.compile_block(&f.body, depth + 1, true, true)?;
                self.initialized = before;
                self.rows.remove(&row);
                out.push(Stmt::For(slot, compiled));
                Ok(())
            }
            ast::Stmt::Expr(e) => {
                if Self::is_tool(&e.value, "record_total") {
                    let (batch, total) = self
                        .tool_call(&e.value, "record_total", depth + 1, in_loop)?
                        .unwrap();
                    if in_loop {
                        return Err(err("tool calls are unsupported inside loops"));
                    }
                    let i = self.hidden_scalar(Type::I64)?;
                    self.initialized |= 1 << i;
                    out.push(Stmt::Record(i, batch, total));
                    return Ok(());
                }
                if Self::is_tool(&e.value, "load_quotes") {
                    let (batch, _) = self
                        .tool_call(&e.value, "load_quotes", depth + 1, in_loop)?
                        .unwrap();
                    if in_loop {
                        return Err(err("tool calls are unsupported inside loops"));
                    }
                    let slot = self.next_buffer;
                    if slot >= ir::MAX_BUFFERS {
                        return Err(resource_err("too many quote buffers"));
                    }
                    self.next_buffer += 1;
                    self.buffers.insert(format!("\0discard{slot}"), slot);
                    out.push(Stmt::Load(slot, batch));
                    return Ok(());
                }
                if let Some(arg) = Self::print_arg(&e.value)? {
                    self.node(depth + 1)?;
                    #[cfg(feature = "tool-json")]
                    if !self.tools.is_empty() && Self::call_parts(arg).is_some() {
                        // Reuse ordinary Tool binding/suspension, then the bounded
                        // scalar printer. NUL names cannot collide with source names.
                        let name = format!("\0print{}", self.locals.len());
                        if self.compile_tool(&name, arg, depth + 2, in_loop, out)? {
                            if self.json.calls.last().expect("bound Tool").json {
                                return Err(err("print accepts only scalar Tool results"));
                            }
                            let slot = self.lookup(&name)?;
                            out.push(Stmt::Print(Expr::Local(slot), Type::I64));
                            return Ok(());
                        }
                    }
                    if let Some((batch, total)) =
                        self.tool_call(arg, "record_total", depth + 2, in_loop)?
                    {
                        if in_loop {
                            return Err(err("tool calls are unsupported inside loops"));
                        }
                        let i = self.hidden_scalar(Type::I64)?;
                        self.initialized |= 1 << i;
                        out.extend([
                            Stmt::Record(i, batch, total),
                            Stmt::Print(Expr::Local(i), Type::I64),
                        ]);
                        return Ok(());
                    }
                    #[cfg(feature = "tool-json")]
                    if let Some((root, path)) = self.json_path(arg)?
                        && self.json_leaves[root]
                            .iter()
                            .any(|(p, t)| p == &path && *t == crate::tool::JsonLeaf::String)
                    {
                        let slot = self.json.path(root, &path)?;
                        self.json.string_paths = true;
                        out.push(Stmt::PrintJsonString { slot, root });
                        return Ok(());
                    }
                    #[cfg(feature = "tool-json")]
                    if !self.tools.is_empty()
                        && (self.line_receiver(arg).is_some()
                            || Self::string_expression(arg)
                            || matches!(arg, ast::Expr::BinOp(b) if b.op == ast::Operator::Add && (self.is_json_string(&b.left)? || self.is_json_string(&b.right)?)))
                    {
                        let source = self.string_source(arg, true)?;
                        let plan = self.json.prints.len();
                        out.push(Stmt::PrintString {
                            plan,
                            roots: source.roots(),
                            scalars: source.scalars(),
                        });
                        self.json.prints.push(source);
                        return Ok(());
                    }
                    let (value, ty) = self.expr(arg, depth + 2, in_loop)?;
                    if !matches!(ty, Type::I64 | Type::Bool) {
                        return Err(err("print accepts one i64 or bool argument"));
                    }
                    out.push(Stmt::Print(value, ty));
                    return Ok(());
                }
                #[cfg(feature = "tool-json")]
                if !self.tools.is_empty() && Self::call_parts(&e.value).is_some() {
                    // Preserve the declared Tool effect while keeping its scalar
                    // result in a compiler-only slot that source cannot name.
                    let name = format!("\0discard{}", self.locals.len());
                    if self.compile_tool(&name, &e.value, depth + 1, in_loop, out)? {
                        if self.json.calls.last().expect("bound Tool").json {
                            return Err(err("discarded Tool results must be scalar"));
                        }
                        return Ok(());
                    }
                }
                let (value, _) = self.expr(&e.value, depth + 1, in_loop)?;
                out.push(Stmt::Eval(value));
                Ok(())
            }
            _ => Err(err("unsupported statement in restricted program")),
        }
    }
    fn compile_block(
        &mut self,
        body: &[ast::Stmt],
        depth: usize,
        in_loop: bool,
        in_loop_body: bool,
    ) -> Result<Vec<Stmt>> {
        let mut out = Vec::new();
        for s in body {
            self.compile_stmt(s, depth, in_loop, in_loop_body, &mut out)?;
        }
        Ok(out)
    }
    fn hidden_scalar(&mut self, ty: Type) -> Result<usize> {
        let name = format!("\0result{}", self.locals.len());
        let i = self.locals.len();
        if i >= ir::MAX_LOCALS {
            return Err(resource_err("program exceeds MAX_LOCALS"));
        }
        self.names.push(name);
        self.locals.push(ty);
        Ok(i)
    }
    #[cfg(feature = "tool-json")]
    fn json_path(&self, e: &ast::Expr) -> Result<Option<(usize, Vec<String>)>> {
        match e {
            ast::Expr::Name(n) => Ok(self.json_names.get(n.id.as_str()).cloned()),
            ast::Expr::Subscript(_) | ast::Expr::Attribute(_) => {
                let value = match e {
                    ast::Expr::Subscript(s) => &s.value,
                    ast::Expr::Attribute(a) => &a.value,
                    _ => unreachable!(),
                };
                let Some((root, mut keys)) = self.json_path(value)? else {
                    return Ok(None);
                };
                keys.push(Self::json_field_key(
                    e,
                    "JSON indices must be literal field keys",
                )?);
                if keys.len() > 32
                    || !self.json_leaves[root]
                        .iter()
                        .any(|(p, _)| p.starts_with(&keys))
                        && !matches!(self.json_leaves[root].0,
                            crate::tool::ResultSchema::JsonObjectArray(schema) if schema.path.starts_with(&keys))
                {
                    return Err(err("JSON field path is absent from the declared schema"));
                }
                Ok(Some((root, keys)))
            }
            _ => Ok(None),
        }
    }
    #[cfg(feature = "tool-json")]
    fn json_field_key(e: &ast::Expr, message: &str) -> Result<String> {
        match e {
            ast::Expr::Attribute(a) => Ok(a.attr.to_string()),
            ast::Expr::Subscript(s) => match s.slice.as_ref() {
                ast::Expr::Constant(c) => match &c.value {
                    ast::Constant::Str(key) => Ok(key.clone()),
                    _ => Err(err(message)),
                },
                _ => Err(err(message)),
            },
            _ => unreachable!(),
        }
    }
    #[cfg(feature = "tool-json")]
    fn bind_json(&mut self, name: &str, root: usize, path: Vec<String>) -> Result<()> {
        if self.names.iter().any(|n| n == name)
            || self.line.as_ref().is_some_and(|l| l.0 == name)
            || self.buffers.contains_key(name)
            || self.rows.contains(name)
            || self.json_names.contains_key(name)
            || [
                "tools", "print", "True", "False", "len", "abs", "min", "max",
            ]
            .contains(&name)
        {
            return Err(err(
                "JSON view name conflicts with an existing/reserved binding",
            ));
        }
        if self.json_names.len() >= 128 {
            return Err(resource_err("too many JSON view bindings"));
        }
        self.json_names.insert(name.to_owned(), (root, path));
        Ok(())
    }
    #[cfg(feature = "tool-json")]
    fn line_receiver(&self, e: &ast::Expr) -> Option<usize> {
        if let ast::Expr::Name(n) = e {
            self.line
                .as_ref()
                .and_then(|(name, root)| (n.id.as_str() == name).then_some(*root))
        } else {
            None
        }
    }
    #[cfg(feature = "tool-json")]
    fn is_json_string(&self, e: &ast::Expr) -> Result<bool> {
        Ok(self.json_path(e)?.is_some_and(|(root, path)| {
            self.json_leaves[root]
                .iter()
                .any(|(p, t)| p == &path && *t == crate::tool::JsonLeaf::String)
        }))
    }
    #[cfg(feature = "tool-json")]
    fn predicate_literal(&self, c: &ast::ExprConstant, literal: &str, name: &str) -> Result<()> {
        if literal.len() > 4096 {
            return Err(resource_err(format!(
                "{name} literal exceeds 4096 UTF-8 bytes"
            )));
        }
        // RustPython 0.4 replaces surrogate escapes with U+FFFD. Only inspect
        // raw tokens when that scalar occurs, preserving genuine U+FFFD, raw
        // strings, escaped backslashes and implicitly concatenated literals.
        if literal.contains('\u{fffd}') {
            use rustpython_parser::{Mode, Tok, lexer};
            let source = &self.source[c.start().to_usize()..c.end().to_usize()];
            for token in lexer::lex(source, Mode::Expression) {
                let (token, _) = token.map_err(|_| err(format!("invalid {name} literal")))?;
                if let Tok::String { value, kind, .. } = token {
                    if kind.is_raw() {
                        continue;
                    }
                    let mut chars = value.chars();
                    while let Some(ch) = chars.next() {
                        if ch != '\\' {
                            continue;
                        }
                        let digits = match chars.next() {
                            Some('u') => 4,
                            Some('U') => 8,
                            _ => continue,
                        };
                        let mut scalar = 0u32;
                        for _ in 0..digits {
                            let digit = chars
                                .next()
                                .and_then(|c| c.to_digit(16))
                                .ok_or_else(|| err(format!("invalid {name} literal escape")))?;
                            scalar = (scalar << 4) | digit;
                        }
                        if (0xd800..=0xdfff).contains(&scalar) {
                            return Err(err(format!(
                                "surrogate escapes are unsupported in {name} literals"
                            )));
                        }
                    }
                }
            }
        }
        Ok(())
    }
    #[cfg(feature = "tool-json")]
    fn get_expression(e: &ast::Expr) -> bool {
        matches!(e, ast::Expr::Call(c) if matches!(c.func.as_ref(), ast::Expr::Attribute(a) if a.attr.as_str() == "get"))
    }
    #[cfg(feature = "tool-json")]
    fn get_string_source(&mut self, e: &ast::Expr) -> Result<crate::tool::StringSource> {
        let ast::Expr::Call(c) = e else {
            unreachable!()
        };
        let ast::Expr::Attribute(a) = c.func.as_ref() else {
            unreachable!()
        };
        let Some((root, mut path)) = self.json_path(&a.value)? else {
            return Err(err("get receiver must be a bound JSON object path"));
        };
        if self.json_leaves[root].iter().any(|(p, _)| p == &path) {
            return Err(type_err(
                "get receiver must be an object, not a declared leaf",
            ));
        }
        if c.args.len() != 2 || !c.keywords.is_empty() {
            return Err(err("get requires two positional string literals"));
        }
        fn literal<'e>(compiler: &Compiler<'_>, e: &'e ast::Expr) -> Result<&'e str> {
            let ast::Expr::Constant(c) = e else {
                return Err(err("get requires literal key and default"));
            };
            let ast::Constant::Str(s) = &c.value else {
                return Err(err("get requires string key and default"));
            };
            compiler.predicate_literal(c, s, "get")?;
            Ok(s)
        }
        let key = literal(self, &c.args[0])?;
        let default = std::sync::Arc::from(literal(self, &c.args[1])?);
        path.push(key.to_owned());
        let leaf = self.json_leaves[root]
            .iter()
            .find_map(|(p, t)| (p == &path).then_some(*t));
        match leaf {
            None => {
                return Err(CompileError::new(
                    AdmissionKind::Schema,
                    "get field path is absent from the declared schema",
                ));
            }
            Some(crate::tool::JsonLeaf::String) => {}
            _ => return Err(type_err("get requires a declared JSON string leaf")),
        }
        let slot = self.json.path(root, &path)?;
        self.json.string_paths = true;
        Ok(crate::tool::StringSource::JsonGetLiteral {
            slot,
            root,
            default,
        })
    }
    #[cfg(feature = "tool-json")]
    fn string_expression(e: &ast::Expr) -> bool {
        Self::get_expression(e)
            || matches!(
                e,
                ast::Expr::JoinedStr(_)
                    | ast::Expr::Constant(ast::ExprConstant {
                        value: ast::Constant::Str(_),
                        ..
                    })
            )
            || matches!(e, ast::Expr::BinOp(b) if b.op == ast::Operator::Add && (Self::string_expression(&b.left) || Self::string_expression(&b.right)))
    }
    #[cfg(feature = "tool-json")]
    fn string_source(&mut self, e: &ast::Expr, format: bool) -> Result<crate::tool::StringSource> {
        use crate::tool::StringSource as S;
        let mut parts = Vec::new();
        self.string_parts(e, format, &mut parts)?;
        if parts.len() == 1 {
            Ok(parts.pop().unwrap())
        } else {
            Ok(S::Joined(parts))
        }
    }
    #[cfg(feature = "tool-json")]
    fn string_parts(
        &mut self,
        e: &ast::Expr,
        format: bool,
        parts: &mut Vec<crate::tool::StringSource>,
    ) -> Result<()> {
        use crate::tool::{JsonLeaf, StringSource as S};
        match e {
            e if self.line_receiver(e).is_some() => {
                parts.push(S::Line {
                    root: self.line_receiver(e).unwrap(),
                });
            }
            e if Self::get_expression(e) => parts.push(self.get_string_source(e)?),
            ast::Expr::Constant(c) if matches!(c.value, ast::Constant::Str(_)) => {
                let ast::Constant::Str(s) = &c.value else {
                    unreachable!()
                };
                if s.len() > 4096 {
                    return Err(resource_err("string literal exceeds 4096 bytes"));
                }
                parts.push(S::Literal(std::sync::Arc::from(s.as_str())));
            }
            ast::Expr::BinOp(b) if b.op == ast::Operator::Add => {
                self.string_parts(&b.left, false, parts)?;
                self.string_parts(&b.right, false, parts)?;
            }
            ast::Expr::JoinedStr(s) => {
                for value in &s.values {
                    if let ast::Expr::FormattedValue(f) = value {
                        if f.format_spec.is_some() || f.conversion != ast::ConversionFlag::None {
                            return Err(err(
                                "f-string conversions and format specs are unsupported",
                            ));
                        }
                        self.string_parts(&f.value, true, parts)?;
                    } else {
                        self.string_parts(value, false, parts)?;
                    }
                }
            }
            _ => {
                if let Some((root, path)) = self.json_path(e)? {
                    let leaf = self.json_leaves[root]
                        .iter()
                        .find_map(|(p, t)| (p == &path).then_some(*t));
                    match leaf {
                        Some(JsonLeaf::String) => {
                            let slot = self.json.path(root, &path)?;
                            self.json.string_paths = true;
                            parts.push(S::Json { slot, root });
                        }
                        Some(JsonLeaf::I64) if format => {
                            let slot = self.json.path(root, &path)?;
                            parts.push(S::JsonInteger { slot, root });
                        }
                        _ => {
                            return Err(type_err(
                                "string part requires declared JSON string leaf or f-string i64 leaf",
                            ));
                        }
                    }
                } else if format {
                    match e {
                        ast::Expr::Name(n) => {
                            let slot = self.lookup(n.id.as_str())?;
                            if self.initialized & (1 << slot) == 0 {
                                return Err(err("uninitialized f-string field"));
                            }
                            parts.push(S::Scalar {
                                slot,
                                boolean: self.locals[slot] == Type::Bool,
                            });
                        }
                        ast::Expr::Constant(c) => {
                            let text = match &c.value {
                                ast::Constant::Bool(v) => {
                                    if *v {
                                        "True".to_owned()
                                    } else {
                                        "False".to_owned()
                                    }
                                }
                                ast::Constant::Int(v) => v.to_string(),
                                _ => {
                                    return Err(type_err(
                                        "f-string fields require scalar locals or JSON strings",
                                    ));
                                }
                            };
                            if text.parse::<i64>().is_err()
                                && !matches!(c.value, ast::Constant::Bool(_))
                            {
                                return Err(type_err("f-string integer exceeds i64"));
                            }
                            parts.push(S::Literal(std::sync::Arc::from(text)));
                        }
                        _ => {
                            return Err(type_err(
                                "f-string fields require scalar locals or JSON strings",
                            ));
                        }
                    }
                } else {
                    return Err(type_err(
                        "string part requires literal or declared JSON string leaf",
                    ));
                }
            }
        }
        if parts.len() > 32 {
            return Err(resource_err("string expression exceeds 32 parts"));
        }
        Ok(())
    }
    #[cfg(feature = "tool-json")]
    fn compile_tool(
        &mut self,
        name: &str,
        e: &ast::Expr,
        depth: usize,
        in_loop: bool,
        out: &mut Vec<Stmt>,
    ) -> Result<bool> {
        let Some((call, _)) = Self::call_parts(e) else {
            return Ok(false);
        };
        let ast::Expr::Call(c) = call else {
            unreachable!()
        };
        let ast::Expr::Attribute(a) = c.func.as_ref() else {
            return Ok(false);
        };
        if !matches!(a.value.as_ref(), ast::Expr::Name(n) if n.id.as_str() == "tools") {
            return Ok(false);
        }
        let Some(tool) = self.tools.iter().find(|t| t.name == a.attr.as_str()) else {
            return Ok(false);
        };
        if in_loop {
            return Err(err("tool calls are unsupported inside loops"));
        }
        self.node(depth)?;
        if matches!(
            tool.arguments.first(),
            Some(crate::tool::ArgumentSchema::String(_))
        ) {
            if c.args.len() + c.keywords.len() != 1
                || c.args.len() > 1
                || c.keywords.first().is_some_and(|k| {
                    k.arg.as_ref().map(|s| s.as_str()) != Some(tool.arguments[0].name())
                })
            {
                return Err(err("string Tool requires its declared argument"));
            }
            let value = if let Some(value) = c.args.first() {
                value
            } else {
                &c.keywords[0].value
            };
            let source = if Self::string_expression(value)
                || matches!(value, ast::Expr::BinOp(b) if b.op == ast::Operator::Add)
            {
                self.string_source(value, false)?
            } else if let ast::Expr::Constant(v) = value {
                if let ast::Constant::Str(s) = &v.value {
                    if s.len() > 4096 {
                        return Err(resource_err("string Tool literal exceeds 4096 bytes"));
                    }
                    crate::tool::StringSource::Literal(std::sync::Arc::from(s.as_str()))
                } else {
                    return Err(type_err(
                        "string Tool requires a string literal or declared JSON leaf",
                    ));
                }
            } else if let Some((root, path)) = self.json_path(value)? {
                if !self.json_leaves[root]
                    .iter()
                    .any(|(p, t)| p == &path && *t == crate::tool::JsonLeaf::String)
                {
                    return Err(type_err("string Tool requires a declared JSON string leaf"));
                }
                let slot = self.json.path(root, &path)?;
                self.json.string_paths = true;
                crate::tool::StringSource::Json { slot, root }
            } else {
                return Err(type_err(
                    "string Tool requires a string literal or declared JSON leaf",
                ));
            };
            return self.bind_tool(name, tool.id, &tool.result, Vec::new(), Some(source), out);
        }
        if c.args.len() > tool.arguments.len() {
            return Err(err("too many Tool arguments"));
        }
        let mut args: Vec<Option<Expr>> = vec![None; tool.arguments.len()];
        // Evaluate in Python source order into scalar slots BEFORE canonical binding.
        for (position, arg) in c.args.iter().enumerate() {
            let (e, t) = self.expr(arg, depth + 1, false)?;
            if t != Type::I64 {
                return Err(type_err("Tool argument must be i64"));
            }
            let slot = self.hidden_scalar(t)?;
            out.push(Stmt::Set(slot, e));
            args[position] = Some(Expr::Local(slot));
        }
        for kw in &c.keywords {
            let key = kw
                .arg
                .as_ref()
                .ok_or_else(|| err("Tool **kwargs is unsupported"))?;
            let position = tool
                .arguments
                .iter()
                .position(|a| a.name() == key.as_str())
                .ok_or_else(|| err("unknown Tool argument name"))?;
            if args[position].is_some() {
                return Err(err("duplicate Tool argument"));
            }
            let (e, t) = self.expr(&kw.value, depth + 1, false)?;
            if t != Type::I64 {
                return Err(type_err("Tool argument must be i64"));
            }
            let slot = self.hidden_scalar(t)?;
            out.push(Stmt::Set(slot, e));
            args[position] = Some(Expr::Local(slot));
        }
        let args = args
            .into_iter()
            .map(|a| a.ok_or_else(|| err("missing Tool argument")))
            .collect::<Result<Vec<_>>>()?;
        self.bind_tool(name, tool.id, &tool.result, args, None, out)
    }
    #[cfg(feature = "tool-json")]
    fn bind_tool(
        &mut self,
        name: &str,
        id: u32,
        result: &'a crate::tool::ResultSchema,
        args: Vec<Expr>,
        string: Option<crate::tool::StringSource>,
        out: &mut Vec<Stmt>,
    ) -> Result<bool> {
        let json = !matches!(result, crate::tool::ResultSchema::I64);
        let target = if json {
            if self.json.roots >= 32 {
                return Err(resource_err("too many JSON owners"));
            }
            let root = self.json.roots;
            self.bind_json(name, root, Vec::new())?;
            self.json.roots += 1;
            self.json_leaves.push(SchemaLeaves(result));
            root
        } else {
            let slot = self.declare(name, Type::I64, true)?;
            self.initialized |= 1 << slot;
            slot
        };
        let call = self.json.calls.len();
        self.json.calls.push(crate::tool::BoundCall {
            id,
            arguments: args.len(),
            json,
            string,
        });
        out.push(Stmt::Tool {
            call,
            target,
            json,
            args,
            string_scalars: self.json.calls[call]
                .string
                .as_ref()
                .map_or(0, crate::tool::StringSource::scalars),
            string_roots: self.json.calls[call]
                .string
                .as_ref()
                .map_or(0, crate::tool::StringSource::roots),
        });
        Ok(true)
    }
    fn annotation(e: &ast::Expr) -> Result<Type> {
        match e {
            ast::Expr::Name(n) if n.id.as_str() == "i64" || n.id.as_str() == "int" => Ok(Type::I64),
            ast::Expr::Name(n) if n.id.as_str() == "bool" => Ok(Type::Bool),
            _ => Err(err("local annotations must be i64 or bool")),
        }
    }
    fn compile_legacy_return(
        &mut self,
        e: &ast::Expr,
        depth: usize,
        out: &mut Vec<Stmt>,
    ) -> Result<()> {
        if let Some((batch, total)) = self.tool_call(e, "record_total", depth + 1, false)? {
            let i = self.hidden_scalar(Type::I64)?;
            self.initialized |= 1 << i;
            out.extend([Stmt::Record(i, batch, total), Stmt::Return(Expr::Local(i))]);
            return Ok(());
        }
        let (value, ty) = self.expr(e, depth + 1, false)?;
        if ty != Type::I64 {
            return Err(type_err("return expression must be i64"));
        }
        out.push(Stmt::Return(value));
        Ok(())
    }
    fn call_parts(e: &ast::Expr) -> Option<(&ast::Expr, bool)> {
        match e {
            ast::Expr::Call(_) => Some((e, false)),
            ast::Expr::Await(a) => match a.value.as_ref() {
                ast::Expr::Call(_) => Some((a.value.as_ref(), true)),
                _ => None,
            },
            _ => None,
        }
    }
    fn call_args<'b>(e: &'b ast::Expr, tool: &str) -> Result<&'b [ast::Expr]> {
        let (call, _) =
            Self::call_parts(e).ok_or_else(|| err(format!("expected tools.{tool}(...)")))?;
        let ast::Expr::Call(c) = call else {
            unreachable!()
        };
        let ast::Expr::Attribute(a) = c.func.as_ref() else {
            return Err(err(format!("expected tools.{tool}(...)")));
        };
        if a.attr.as_str() != tool
            || !matches!(a.value.as_ref(), ast::Expr::Name(n) if n.id.as_str()=="tools")
            || !c.keywords.is_empty()
        {
            return Err(err(format!(
                "only positional tools.{tool}(...) calls are supported"
            )));
        }
        Ok(&c.args)
    }
    fn is_tool(e: &ast::Expr, tool: &str) -> bool {
        let Some((call, _)) = Self::call_parts(e) else {
            return false;
        };
        let ast::Expr::Call(c) = call else {
            return false;
        };
        matches!(c.func.as_ref(), ast::Expr::Attribute(a) if a.attr.as_str()==tool && matches!(a.value.as_ref(), ast::Expr::Name(n) if n.id.as_str()=="tools"))
    }
    fn tool_call(
        &mut self,
        e: &ast::Expr,
        tool: &str,
        depth: usize,
        in_loop: bool,
    ) -> Result<Option<(Expr, Expr)>> {
        let Some((call, _awaited)) = Self::call_parts(e) else {
            return Ok(None);
        };
        if !Self::is_tool(e, tool) {
            return Ok(None);
        }
        let args = Self::call_args(e, tool)?;
        let expected = if tool == "load_quotes" { 1 } else { 2 };
        if args.len() != expected {
            return Err(err(format!(
                "tools.{tool} expects {expected} positional arguments"
            )));
        }
        self.node(depth)?;
        let (batch, bt) = self.expr(&args[0], depth + 1, in_loop)?;
        if bt != Type::I64 {
            return Err(type_err("tool batch expression must be i64"));
        }
        if tool == "load_quotes" {
            return Ok(Some((batch, Expr::Int(0))));
        }
        let (total, tt) = self.expr(&args[1], depth + 1, in_loop)?;
        if tt != Type::I64 {
            return Err(type_err("record_total total expression must be i64"));
        }
        let _ = call;
        Ok(Some((batch, total)))
    }
    fn print_arg(e: &ast::Expr) -> Result<Option<&ast::Expr>> {
        let Some((call, awaited)) = Self::call_parts(e) else {
            return Ok(None);
        };
        let ast::Expr::Call(c) = call else {
            unreachable!()
        };
        if !matches!(c.func.as_ref(), ast::Expr::Name(n) if n.id.as_str()=="print") {
            return Ok(None);
        }
        if awaited {
            return Err(err("await applies only to declared Tool calls"));
        }
        if !c.keywords.is_empty() || c.args.len() != 1 {
            return Err(err("print accepts exactly one positional argument"));
        }
        Ok(Some(&c.args[0]))
    }
    fn expr(&mut self, e: &ast::Expr, depth: usize, in_loop: bool) -> Result<(Expr, Type)> {
        self.expr_inner(e, depth, in_loop)
            .map_err(|error| error.at(self.source, e.start().to_usize()))
    }
    fn expr_inner(&mut self, e: &ast::Expr, depth: usize, in_loop: bool) -> Result<(Expr, Type)> {
        self.node(depth)?;
        #[cfg(feature = "tool-json")]
        if let Some((row, root, path)) = &self.json_row
            && let Some(value) = match e {
                ast::Expr::Subscript(s) => Some(s.value.as_ref()),
                ast::Expr::Attribute(a) => Some(a.value.as_ref()),
                _ => None,
            }
            && let ast::Expr::Name(n) = value
            && n.id.as_str() == row
        {
            let key = Self::json_field_key(e, "array row fields must be literal keys")?;
            let crate::tool::ResultSchema::JsonObjectArray(schema) = self.json_leaves[*root].0
            else {
                unreachable!()
            };
            if !schema.i64_fields.contains(&key) {
                return Err(CompileError::new(
                    AdmissionKind::Schema,
                    "undeclared array row field",
                ));
            }
            let root = *root;
            let mut path = path.clone();
            path.push(key);
            let slot = self.json.path(root, &path)?;
            return Ok((Expr::JsonI64 { slot, root }, Type::I64));
        }
        #[cfg(feature = "tool-json")]
        if let Some((root, path)) = self.json_path(e)? {
            let leaf = self.json_leaves[root]
                .iter()
                .find(|(p, _)| *p == &path)
                .map(|(_, t)| *t)
                .ok_or_else(|| err("JSON expression must name a declared leaf"))?;
            if leaf != crate::tool::JsonLeaf::I64 {
                return Err(err(
                    "string leaf requires a string sink or literal predicate",
                ));
            }
            let slot = self.json.path(root, &path)?;
            return Ok((Expr::JsonI64 { slot, root }, Type::I64));
        }
        match e {
            ast::Expr::Call(c) => {
                #[cfg(feature = "tool-json")]
                if let ast::Expr::Attribute(method) = c.func.as_ref()
                    && method.attr.as_str() == "startswith"
                {
                    if !c.keywords.is_empty() || c.args.len() != 1 {
                        return Err(err("startswith expects one positional string literal"));
                    }
                    let ast::Expr::Constant(constant) = &c.args[0] else {
                        return Err(err("startswith prefix must be a string literal"));
                    };
                    let ast::Constant::Str(literal) = &constant.value else {
                        return Err(err("startswith prefix must be a string literal"));
                    };
                    self.predicate_literal(constant, literal, "startswith")?;
                    if let Some(root) = self.line_receiver(&method.value) {
                        let index = self.json.needles.len();
                        self.json.needles.push(literal.clone());
                        return Ok((
                            Expr::LinePredicate {
                                root,
                                literal: index,
                                prefix: true,
                            },
                            Type::Bool,
                        ));
                    }
                    if !self.is_json_string(&method.value)? {
                        return Err(err(
                            "startswith receiver must be a declared JSON string leaf",
                        ));
                    }
                    self.node(depth + 1)?;
                    let (root, path) = self
                        .json_path(&method.value)?
                        .expect("declared string leaf");
                    let slot = self.json.path(root, &path)?;
                    let index = self.json.needles.len();
                    self.json.needles.push(literal.clone());
                    self.json.string_paths = true;
                    return Ok((
                        Expr::StartsWith {
                            slot,
                            root,
                            literal: index,
                        },
                        Type::Bool,
                    ));
                }
                let ast::Expr::Name(name) = c.func.as_ref() else {
                    return Err(err("unsupported call expression"));
                };
                let name = name.id.as_str();
                if !["len", "abs", "min", "max"].contains(&name) {
                    return Err(err(format!("unsupported builtin '{name}'")));
                }
                let arity = if matches!(name, "len" | "abs") { 1 } else { 2 };
                if !c.keywords.is_empty() || c.args.len() != arity {
                    return Err(err(format!("{name} expects {arity} positional arguments")));
                }
                if name == "len" {
                    if let ast::Expr::Name(n) = &c.args[0]
                        && let Some(&slot) = self.buffers.get(n.id.as_str())
                    {
                        return Ok((Expr::BufferLen(slot), Type::I64));
                    }
                    return Err(err("len requires a loaded quote buffer"));
                }
                let (left, ty) = self.expr(&c.args[0], depth + 1, in_loop)?;
                if ty != Type::I64 {
                    return Err(type_err(format!("{name} requires i64 operands")));
                }
                if name == "abs" {
                    return Ok((Expr::Abs(Box::new(left)), Type::I64));
                }
                let (right, ty) = self.expr(&c.args[1], depth + 1, in_loop)?;
                if ty != Type::I64 {
                    return Err(type_err(format!("{name} requires i64 operands")));
                }
                Ok((
                    if name == "min" {
                        Expr::Min(Box::new(left), Box::new(right))
                    } else {
                        Expr::Max(Box::new(left), Box::new(right))
                    },
                    Type::I64,
                ))
            }
            ast::Expr::Constant(c) => match &c.value {
                ast::Constant::Bool(b) => Ok((Expr::Bool(*b), Type::Bool)),
                ast::Constant::Int(n) => {
                    let value =
                        i64::try_from(n).map_err(|_| err("integer literal is outside i64"))?;
                    Ok((Expr::Int(value), Type::I64))
                }
                _ => Err(err("only i64 and bool constants are supported")),
            },
            ast::Expr::Name(n) => {
                let name = n.id.as_str();
                if name == "True" || name == "False" {
                    return Ok((Expr::Bool(name == "True"), Type::Bool));
                }
                if name == "tools"
                    || name == "print"
                    || self.buffers.contains_key(name)
                    || self.rows.contains(name)
                {
                    return Err(err(format!(
                        "reserved name '{name}' is not a scalar expression"
                    )));
                }
                let i = self.lookup(name)?;
                if self.initialized & (1 << i) == 0 {
                    return Err(err(format!("local '{name}' used before initialization")));
                }
                Ok((Expr::Local(i), self.locals[i]))
            }
            ast::Expr::Attribute(a) if in_loop => {
                let field = match (a.value.as_ref(), a.attr.as_str()) {
                    (ast::Expr::Name(n), "unit_price_cents")
                        if self.rows.contains(n.id.as_str()) =>
                    {
                        Field::Price
                    }
                    (ast::Expr::Name(n), "quantity") if self.rows.contains(n.id.as_str()) => {
                        Field::Quantity
                    }
                    _ => {
                        return Err(err(
                            "only the active row's unit_price_cents and quantity fields are available",
                        ));
                    }
                };
                Ok((Expr::Field(field), Type::I64))
            }
            ast::Expr::UnaryOp(u) => match u.op {
                ast::UnaryOp::Not => {
                    let (x, t) = self.expr(&u.operand, depth + 1, in_loop)?;
                    if t != Type::Bool {
                        return Err(type_err("not requires bool"));
                    }
                    Ok((Expr::Not(Box::new(x)), Type::Bool))
                }
                ast::UnaryOp::USub => {
                    if let ast::Expr::Constant(c) = u.operand.as_ref()
                        && let ast::Constant::Int(n) = &c.value
                        && u64::try_from(n) == Ok(1_u64 << 63)
                    {
                        return Ok((Expr::Int(i64::MIN), Type::I64));
                    }
                    let (x, t) = self.expr(&u.operand, depth + 1, in_loop)?;
                    if t != Type::I64 {
                        return Err(type_err("unary minus requires i64"));
                    }
                    Ok((
                        Expr::Binary(BinOp::Sub, Box::new(Expr::Int(0)), Box::new(x)),
                        Type::I64,
                    ))
                }
                _ => Err(err("only not and unary minus are supported")),
            },
            ast::Expr::BinOp(b) => {
                let op = match b.op {
                    ast::Operator::Add => BinOp::Add,
                    ast::Operator::Sub => BinOp::Sub,
                    ast::Operator::Mult => BinOp::Mul,
                    _ => return Err(err("only +, -, and * arithmetic are supported")),
                };
                let (l, lt) = self.expr(&b.left, depth + 1, in_loop)?;
                let (r, rt) = self.expr(&b.right, depth + 1, in_loop)?;
                if lt != Type::I64 || rt != Type::I64 {
                    return Err(type_err("arithmetic operands must be i64"));
                }
                Ok((Expr::Binary(op, Box::new(l), Box::new(r)), Type::I64))
            }
            ast::Expr::BoolOp(b) => {
                let op = match b.op {
                    ast::BoolOp::And => BinOp::And,
                    ast::BoolOp::Or => BinOp::Or,
                };
                if b.values.len() < 2 {
                    return Err(err("invalid boolean expression"));
                }
                let mut iter = b.values.iter();
                let (first, t) = self.expr(iter.next().unwrap(), depth + 1, in_loop)?;
                if t != Type::Bool {
                    return Err(type_err("and/or operands must be bool"));
                }
                let mut result = first;
                for rhs in iter {
                    let (r, t) = self.expr(rhs, depth + 1, in_loop)?;
                    if t != Type::Bool {
                        return Err(type_err("and/or operands must be bool"));
                    }
                    result = Expr::Binary(op, Box::new(result), Box::new(r));
                }
                Ok((result, Type::Bool))
            }
            ast::Expr::Compare(c) => {
                if c.ops.len() != 1 || c.comparators.len() != 1 {
                    return Err(err("chained comparisons are unsupported"));
                }
                #[cfg(feature = "tool-json")]
                if matches!(c.ops[0], ast::CmpOp::Eq | ast::CmpOp::NotEq) {
                    let pair = match (c.left.as_ref(), &c.comparators[0]) {
                        (ast::Expr::Constant(c), leaf) | (leaf, ast::Expr::Constant(c)) => {
                            if let ast::Constant::Str(literal) = &c.value {
                                Some((leaf, c, literal))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let Some((leaf, constant, literal)) = pair {
                        self.predicate_literal(constant, literal, "equality")?;
                        if !self.is_json_string(leaf)? {
                            return Err(err(
                                "string equality requires a declared JSON string leaf and literal",
                            ));
                        }
                        self.node(depth + 1)?;
                        let (root, path) = self.json_path(leaf)?.expect("declared string leaf");
                        let slot = self.json.path(root, &path)?;
                        let index = self.json.needles.len();
                        self.json.needles.push(literal.clone());
                        self.json.string_paths = true;
                        let equal = Expr::StringEq {
                            slot,
                            root,
                            literal: index,
                        };
                        return Ok((
                            if c.ops[0] == ast::CmpOp::NotEq {
                                Expr::Not(Box::new(equal))
                            } else {
                                equal
                            },
                            Type::Bool,
                        ));
                    }
                }
                #[cfg(feature = "tool-json")]
                if matches!(c.ops[0], ast::CmpOp::In) {
                    let ast::Expr::Constant(literal) = c.left.as_ref() else {
                        return Err(err("contains needle must be a string literal"));
                    };
                    let ast::Constant::Str(needle) = &literal.value else {
                        return Err(err("contains needle must be a string literal"));
                    };
                    if let Some(root) = self.line_receiver(&c.comparators[0]) {
                        self.predicate_literal(literal, needle, "line contains")?;
                        let index = self.json.needles.len();
                        self.json.needles.push(needle.clone());
                        return Ok((
                            Expr::LinePredicate {
                                root,
                                literal: index,
                                prefix: false,
                            },
                            Type::Bool,
                        ));
                    }
                    if needle.len() > 256 {
                        return Err(resource_err("contains needle exceeds 256 UTF-8 bytes"));
                    }
                    let (root, path) = self.json_path(&c.comparators[0])?.ok_or_else(|| {
                        err("contains haystack must be a declared JSON string leaf")
                    })?;
                    if !self.json_leaves[root]
                        .iter()
                        .any(|(p, t)| p == &path && *t == crate::tool::JsonLeaf::String)
                    {
                        return Err(err("contains haystack must be a declared JSON string leaf"));
                    }
                    let slot = self.json.path(root, &path)?;
                    let needle_slot = self.json.needles.len();
                    self.json.needles.push(needle.clone());
                    self.json.string_paths = true;
                    return Ok((
                        Expr::Contains {
                            slot,
                            root,
                            needle: needle_slot,
                        },
                        Type::Bool,
                    ));
                }
                let (l, lt) = self.expr(&c.left, depth + 1, in_loop)?;
                let (r, rt) = self.expr(&c.comparators[0], depth + 1, in_loop)?;
                let op = match c.ops[0] {
                    ast::CmpOp::Eq => BinOp::Eq,
                    ast::CmpOp::NotEq => BinOp::Ne,
                    ast::CmpOp::Lt => BinOp::Lt,
                    ast::CmpOp::LtE => BinOp::Le,
                    ast::CmpOp::Gt => BinOp::Gt,
                    ast::CmpOp::GtE => BinOp::Ge,
                    _ => return Err(err("unsupported comparison")),
                };
                if lt != rt {
                    return Err(type_err("comparison operands must have the same type"));
                }
                if !matches!(op, BinOp::Eq | BinOp::Ne) && lt != Type::I64 {
                    return Err(type_err("ordering comparisons require i64"));
                }
                Ok((Expr::Binary(op, Box::new(l), Box::new(r)), Type::Bool))
            }
            _ => Err(err("unsupported expression in restricted program")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initialization_mask_covers_zero_and_all_32_slots() {
        assert!(parse_with_inputs("x=1\nprint(x)", &[]).is_ok());
        let names: Vec<_> = (0..32).map(|i| format!("input{i}")).collect();
        let schema: Vec<_> = names.iter().map(|s| (s.as_str(), Type::I64)).collect();
        assert!(parse_with_inputs("print(input31)", &schema).is_ok());
        assert!(parse_with_inputs("extra=0", &schema).is_err());
        let prefix: String = (0..31).map(|i| format!("x{i}=0\n")).collect();
        assert!(
            parse_with_inputs(
                &format!("{prefix}if True:\n    x31=1\nelse:\n    x31=2\nprint(x31)"),
                &[]
            )
            .is_ok()
        );
        assert!(
            parse_with_inputs(&format!("{prefix}if True:\n    x31=1\nprint(x31)"), &[]).is_err()
        );
    }
    #[test]
    fn parses_script_inputs_tools_print_and_loops() {
        let p = parse_with_inputs("batch = tools.load_quotes(seed)\ntotal = 0\nfor row in batch:\n    if row.quantity > 0:\n        total += row.unit_price_cents * row.quantity\nprint(total)\nreceipt = await tools.record_total(seed, total)\nreceipt", &[("seed", Type::I64)]).unwrap();
        assert_eq!(p.inputs, 1);
        assert_eq!(p.buffers, 1);
        assert!(!p.legacy);
        assert!(matches!(p.statements.first(), Some(Stmt::Load(0, _))));
        assert!(matches!(p.statements.last(), Some(Stmt::Return(_))));
    }
    #[test]
    fn supports_legacy_main_semantics() {
        let p = parse("async def main(batch_id: i64, price_limit: i64) -> i64:\n    quotes = await tools.load_quotes(batch_id)\n    total: i64 = 0\n    for item in quotes:\n        if item.quantity > 0 and item.unit_price_cents <= price_limit:\n            total += item.unit_price_cents * item.quantity\n    return await tools.record_total(batch_id, total)\n").unwrap();
        assert!(p.legacy);
        assert_eq!(p.buffers, 1);
        assert!(matches!(
            p.statements.last(),
            Some(Stmt::Return(Expr::Local(_)))
        ));
    }
    #[test]
    fn rejects_unsafe_shapes_and_uninitialized_branches() {
        for s in [
            "while True:\n    x=1",
            "import os",
            "return 3",
            "x = tools.load_quotes(x, 1)",
            "print(1, 2)",
        ] {
            assert!(parse_with_inputs(s, &[]).is_err(), "{s}");
        }
        assert!(parse_with_inputs("if flag:\n    x = 1\ny = x", &[("flag", Type::Bool)]).is_err());
        assert!(
            parse_with_inputs("a = tools.load_quotes(x)\nb = a\n", &[("x", Type::I64)]).is_err()
        );
    }
    #[test]
    fn top_level_expressions_are_discarded_and_loop_locals_are_iteration_local() {
        let p = parse_with_inputs(
            "batch = tools.load_quotes(seed)\ntotal: int = 0\nfor q in batch:\n    amount = q.quantity * q.unit_price_cents\n    total += amount\nTrue\n9223372036854775807 + 1",
            &[("seed", Type::I64)],
        )
        .unwrap();
        assert!(matches!(
            p.statements.last(),
            Some(Stmt::Return(Expr::Int(0)))
        ));
        assert!(!p.statements.iter().any(|s| matches!(s, Stmt::Print(_, _))));
        let Some(Stmt::For(_, body)) = p.statements.iter().find(|s| matches!(s, Stmt::For(_, _)))
        else {
            panic!("expected scan loop");
        };
        assert!(matches!(
            body.first(),
            Some(Stmt::Set(_, Expr::Binary(BinOp::Mul, _, _)))
        ));
        let Some(Stmt::Eval(Expr::Binary(BinOp::Add, _, _))) =
            p.statements.get(p.statements.len() - 2)
        else {
            panic!("overflow-capable expression must remain an Eval");
        };
        assert!(
            parse_with_inputs(
                "batch = tools.load_quotes(seed)\nfor q in batch:\n    amount = q.quantity\namount",
                &[("seed", Type::I64)],
            )
            .is_err()
        );
    }

    #[test]
    fn print_is_allowed_in_loop_and_nested_branch_init_is_intersected() {
        let p = parse_with_inputs(
            "batch = tools.load_quotes(seed)\nfor q in batch:\n    if flag:\n        amount = q.quantity\n    else:\n        amount = q.unit_price_cents\n    print(amount)",
            &[("seed", Type::I64), ("flag", Type::Bool)],
        )
        .unwrap();
        let Some(Stmt::For(_, body)) = p.statements.iter().find(|s| matches!(s, Stmt::For(_, _)))
        else {
            panic!("expected scan loop");
        };
        assert!(body.iter().any(|s| matches!(s, Stmt::Print(_, Type::I64))));
        assert!(parse_with_inputs(
            "batch = tools.load_quotes(seed)\nfor q in batch:\n    if flag:\n        amount = q.quantity\n    print(amount)",
            &[("seed", Type::I64), ("flag", Type::Bool)],
        )
        .is_err());
    }

    #[test]
    fn discarded_record_call_still_allocates_a_scratch_scalar() {
        let p = parse_with_inputs(
            "tools.record_total(seed, value)",
            &[("seed", Type::I64), ("value", Type::I64)],
        )
        .unwrap();
        assert_eq!(p.locals.len(), 3);
        assert!(matches!(p.statements.first(), Some(Stmt::Record(2, _, _))));
        assert!(matches!(
            p.statements.last(),
            Some(Stmt::Return(Expr::Int(0)))
        ));
    }

    #[test]
    fn accepts_int_local_annotation() {
        let p = parse_with_inputs("amount: int = seed", &[("seed", Type::I64)]).unwrap();
        assert_eq!(p.locals.last(), Some(&Type::I64));
    }

    #[test]
    fn accepts_definite_branch_init_and_sequential_buffers() {
        let p = parse_with_inputs("a = tools.load_quotes(x)\nb = tools.load_quotes(x + 1)\nif flag:\n    y = 1\nelse:\n    y = 2\nfor anything in a:\n    y += anything.quantity\nfor row in b:\n    y += row.unit_price_cents\ny", &[("x", Type::I64), ("flag", Type::Bool)]).unwrap();
        assert_eq!(p.buffers, 2);
    }
}
