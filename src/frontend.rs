//! Restricted Python source frontend: RustPython AST -> typed semantic IR.
use crate::ir::{self, BinOp, CompileError, Expr, Field, Program, Stmt, Type};
use rustpython_parser::{Parse, ast};
use std::collections::{HashMap, HashSet};

const MAX_DEPTH: usize = 32;
type Result<T> = std::result::Result<T, CompileError>;
fn err(message: impl Into<String>) -> CompileError {
    CompileError(message.into())
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
        return Err(err("source exceeds MAX_SOURCE_BYTES"));
    }
    check_delimiter_depth(source)?;
    validate_inputs(inputs)?;
    #[cfg(feature = "compiler-profile")]
    let start = std::time::Instant::now();
    let module = ast::Suite::parse(source, "<minipy>")
        .map_err(|e| err(format!("Python syntax error: {e}")))?;
    #[cfg(feature = "compiler-profile")]
    crate::compiler_profile::update(|p| p.parse_ns = crate::compiler_profile::elapsed(start));
    #[cfg(feature = "compiler-profile")]
    let start = std::time::Instant::now();
    let result = Compiler::new(inputs)?.program(module);
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
    crate::tool::ToolSchema::validate_all(tools)?;
    if source.len() > ir::MAX_SOURCE_BYTES {
        return Err(err("source exceeds MAX_SOURCE_BYTES"));
    }
    check_delimiter_depth(source)?;
    validate_inputs(inputs)?;
    let module = ast::Suite::parse(source, "<minipy>")
        .map_err(|e| err(format!("Python syntax error: {e}")))?;
    let mut compiler = Compiler::new(inputs)?;
    compiler.tools = tools.to_vec();
    compiler.program(module)
}

fn validate_inputs(inputs: &[(&str, Type)]) -> Result<()> {
    if inputs.len() > ir::MAX_LOCALS {
        return Err(err("program exceeds MAX_LOCALS"));
    }
    let mut names = HashSet::new();
    for (name, _) in inputs {
        if name.is_empty() || !names.insert(*name) {
            return Err(err("input names must be unique identifiers"));
        }
        if !name
            .chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_alphanumeric() && (i != 0 || !c.is_numeric()))
            || ["tools", "print", "quotes", "True", "False", "i64", "bool"].contains(name)
        {
            return Err(err(format!("reserved or invalid input name '{name}'")));
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
        return Err(err("statement nesting exceeds the pre-parser limit"));
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
                    return Err(err("expression nesting exceeds maximum depth 32"));
                }
            }
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

struct Compiler {
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
    tools: Vec<crate::tool::ToolSchema>,
    #[cfg(feature = "tool-json")]
    json: crate::tool::Plan,
    #[cfg(feature = "tool-json")]
    json_names: HashMap<String, (usize, Vec<String>)>,
    #[cfg(feature = "tool-json")]
    json_leaves: Vec<Vec<(Vec<String>, crate::tool::JsonLeaf)>>,
}
impl Compiler {
    fn new(inputs: &[(&str, Type)]) -> Result<Self> {
        if inputs.len() > ir::MAX_LOCALS {
            return Err(err("program exceeds MAX_LOCALS"));
        }
        Ok(Self {
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
            tools: Vec::new(),
            #[cfg(feature = "tool-json")]
            json: crate::tool::Plan::default(),
            #[cfg(feature = "tool-json")]
            json_names: HashMap::new(),
            #[cfg(feature = "tool-json")]
            json_leaves: Vec::new(),
        })
    }
    fn node(&mut self, depth: usize) -> Result<()> {
        self.nodes += 1;
        if self.nodes > ir::MAX_NODES {
            return Err(err("AST exceeds MAX_NODES"));
        }
        if depth > MAX_DEPTH {
            return Err(err("AST nesting exceeds maximum depth 32"));
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
                    return Err(err("only undecorated async def main is supported"));
                }
                self.check_legacy_signature(&fun.args)?;
                if !matches!(fun.returns.as_deref(), Some(ast::Expr::Name(n)) if ["i64", "int"].contains(&n.id.as_str()))
                {
                    return Err(err("legacy main must return int/i64"));
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
                    ));
                }
                let value = r
                    .value
                    .as_deref()
                    .ok_or_else(|| err("return requires an expression"))?;
                self.compile_legacy_return(value, 0, &mut statements)?;
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
            return Err(err("program exceeds MAX_LOCALS"));
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
        if self.json_names.contains_key(name) {
            return Err(err("JSON view names cannot be rebound"));
        }
        if ["tools", "print", "True", "False"].contains(&name)
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
                return Err(err(format!("local '{name}' changes type")));
            }
            if !allow_existing && self.initialized & (1 << i) != 0 {
                return Err(err(format!("local '{name}' is already initialized")));
            }
            return Ok(i);
        }
        if self.names.len() >= ir::MAX_LOCALS {
            return Err(err("program exceeds MAX_LOCALS"));
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
                        return Err(err("load_quotes batch expression must be i64"));
                    }
                    if self.next_buffer >= ir::MAX_BUFFERS {
                        return Err(err("program exceeds 32 quote buffers"));
                    }
                    if self.names.iter().any(|n| n == &name)
                        || self.buffers.contains_key(&name)
                        || ["tools", "print"].contains(&name.as_str())
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
                    return Err(err(format!("initializer for '{name}' has the wrong type")));
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
                    return Err(err(format!("initializer for '{name}' has the wrong type")));
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
                    return Err(err("augmented arithmetic requires i64"));
                }
                let op = match a.op {
                    ast::Operator::Add => BinOp::Add,
                    ast::Operator::Sub => BinOp::Sub,
                    ast::Operator::Mult => BinOp::Mul,
                    _ => return Err(err("only +=, -=, and *= are supported")),
                };
                let (rhs, ty) = self.expr(&a.value, depth + 1, in_loop)?;
                if ty != Type::I64 {
                    return Err(err("augmented arithmetic requires i64"));
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
                    return Err(err("if condition must be bool"));
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
            ast::Stmt::For(f) => {
                if in_loop {
                    return Err(err("nested loops are unsupported"));
                }
                if !f.orelse.is_empty() {
                    return Err(err("for-else is unsupported"));
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
                    || ["tools", "print"].contains(&row.as_str())
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
                        return Err(err("too many quote buffers"));
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
            return Err(err("program exceeds MAX_LOCALS"));
        }
        self.names.push(name);
        self.locals.push(ty);
        Ok(i)
    }
    #[cfg(feature = "tool-json")]
    fn json_path(&self, e: &ast::Expr) -> Result<Option<(usize, Vec<String>)>> {
        match e {
            ast::Expr::Name(n) => Ok(self.json_names.get(n.id.as_str()).cloned()),
            ast::Expr::Subscript(s) => {
                let Some((root, mut keys)) = self.json_path(&s.value)? else {
                    return Ok(None);
                };
                let ast::Expr::Constant(c) = s.slice.as_ref() else {
                    return Err(err("JSON indices must be literal field keys"));
                };
                let ast::Constant::Str(key) = &c.value else {
                    return Err(err("JSON indices must be literal field keys"));
                };
                keys.push(key.clone());
                if keys.len() > 32
                    || !self.json_leaves[root]
                        .iter()
                        .any(|(p, _)| p.starts_with(&keys))
                {
                    return Err(err("JSON field path is absent from the declared schema"));
                }
                Ok(Some((root, keys)))
            }
            _ => Ok(None),
        }
    }
    #[cfg(feature = "tool-json")]
    fn bind_json(&mut self, name: &str, root: usize, path: Vec<String>) -> Result<()> {
        if self.names.iter().any(|n| n == name)
            || self.buffers.contains_key(name)
            || self.rows.contains(name)
            || self.json_names.contains_key(name)
            || ["tools", "print", "True", "False"].contains(&name)
        {
            return Err(err(
                "JSON view name conflicts with an existing/reserved binding",
            ));
        }
        if self.json_names.len() >= 128 {
            return Err(err("too many JSON view bindings"));
        }
        self.json_names.insert(name.to_owned(), (root, path));
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
        let Some(tool) = self
            .tools
            .iter()
            .find(|t| t.name == a.attr.as_str())
            .cloned()
        else {
            return Ok(false);
        };
        if in_loop {
            return Err(err("tool calls are unsupported inside loops"));
        }
        self.node(depth)?;
        if c.args.len() > tool.arguments.len() {
            return Err(err("too many Tool arguments"));
        }
        let mut args: Vec<Option<Expr>> = vec![None; tool.arguments.len()];
        // Evaluate in Python source order into scalar slots BEFORE canonical binding.
        for (position, arg) in c.args.iter().enumerate() {
            let (e, t) = self.expr(arg, depth + 1, false)?;
            if t != Type::I64 {
                return Err(err("Tool argument must be i64"));
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
                .position(|a| a == key.as_str())
                .ok_or_else(|| err("unknown Tool argument name"))?;
            if args[position].is_some() {
                return Err(err("duplicate Tool argument"));
            }
            let (e, t) = self.expr(&kw.value, depth + 1, false)?;
            if t != Type::I64 {
                return Err(err("Tool argument must be i64"));
            }
            let slot = self.hidden_scalar(t)?;
            out.push(Stmt::Set(slot, e));
            args[position] = Some(Expr::Local(slot));
        }
        let args = args
            .into_iter()
            .map(|a| a.ok_or_else(|| err("missing Tool argument")))
            .collect::<Result<Vec<_>>>()?;
        let json = !matches!(tool.result, crate::tool::ResultSchema::I64);
        let target = if json {
            if self.json.roots >= 32 {
                return Err(err("too many JSON owners"));
            }
            let root = self.json.roots;
            self.bind_json(name, root, Vec::new())?;
            self.json.roots += 1;
            let leaves = match tool.result {
                crate::tool::ResultSchema::Json(paths) => paths
                    .into_iter()
                    .map(|p| (p, crate::tool::JsonLeaf::I64))
                    .collect(),
                crate::tool::ResultSchema::JsonTyped(paths) => paths,
                crate::tool::ResultSchema::I64 => unreachable!(),
            };
            self.json_leaves.push(leaves);
            root
        } else {
            let slot = self.declare(name, Type::I64, true)?;
            self.initialized |= 1 << slot;
            slot
        };
        let call = self.json.calls.len();
        self.json.calls.push(crate::tool::BoundCall {
            id: tool.id,
            arguments: args.len(),
            json,
        });
        out.push(Stmt::Tool {
            call,
            target,
            json,
            args,
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
            return Err(err("return expression must be i64"));
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
    fn call_args<'a>(e: &'a ast::Expr, tool: &str) -> Result<&'a [ast::Expr]> {
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
            return Err(err("tool batch expression must be i64"));
        }
        if tool == "load_quotes" {
            return Ok(Some((batch, Expr::Int(0))));
        }
        let (total, tt) = self.expr(&args[1], depth + 1, in_loop)?;
        if tt != Type::I64 {
            return Err(err("record_total total expression must be i64"));
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
        self.node(depth)?;
        #[cfg(feature = "tool-json")]
        if let Some((root, path)) = self.json_path(e)? {
            let leaf = self.json_leaves[root]
                .iter()
                .find(|(p, _)| p == &path)
                .map(|(_, t)| *t)
                .ok_or_else(|| err("JSON expression must name a declared leaf"))?;
            if leaf != crate::tool::JsonLeaf::I64 {
                return Err(err(
                    "string leaf is only supported as the right operand of literal `in`",
                ));
            }
            let slot = self.json.path(root, &path)?;
            return Ok((Expr::JsonI64 { slot, root }, Type::I64));
        }
        match e {
            ast::Expr::Constant(c) => match &c.value {
                ast::Constant::Bool(b) => Ok((Expr::Bool(*b), Type::Bool)),
                ast::Constant::Int(n) => {
                    let value = n
                        .to_string()
                        .parse::<i64>()
                        .map_err(|_| err("integer literal is outside i64"))?;
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
                        return Err(err("not requires bool"));
                    }
                    Ok((Expr::Not(Box::new(x)), Type::Bool))
                }
                ast::UnaryOp::USub => {
                    if let ast::Expr::Constant(c) = u.operand.as_ref()
                        && let ast::Constant::Int(n) = &c.value
                        && n.to_string() == "9223372036854775808"
                    {
                        return Ok((Expr::Int(i64::MIN), Type::I64));
                    }
                    let (x, t) = self.expr(&u.operand, depth + 1, in_loop)?;
                    if t != Type::I64 {
                        return Err(err("unary minus requires i64"));
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
                    return Err(err("arithmetic operands must be i64"));
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
                    return Err(err("and/or operands must be bool"));
                }
                let mut result = first;
                for rhs in iter {
                    let (r, t) = self.expr(rhs, depth + 1, in_loop)?;
                    if t != Type::Bool {
                        return Err(err("and/or operands must be bool"));
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
                if matches!(c.ops[0], ast::CmpOp::In) {
                    let ast::Expr::Constant(literal) = c.left.as_ref() else {
                        return Err(err("contains needle must be a string literal"));
                    };
                    let ast::Constant::Str(needle) = &literal.value else {
                        return Err(err("contains needle must be a string literal"));
                    };
                    if needle.len() > 256 {
                        return Err(err("contains needle exceeds 256 UTF-8 bytes"));
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
                    return Err(err("comparison operands must have the same type"));
                }
                if !matches!(op, BinOp::Eq | BinOp::Ne) && lt != Type::I64 {
                    return Err(err("ordering comparisons require i64"));
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
