//! Restricted Python source frontend: RustPython AST -> typed scan IR.
use crate::ir::{self, BinOp, CompileError, Expr, Field, Program, Stmt, Type};
use rustpython_parser::{Parse, ast};
use std::collections::HashSet;

const MAX_DEPTH: usize = 32;

type Result<T> = std::result::Result<T, CompileError>;
fn err(message: impl Into<String>) -> CompileError {
    CompileError(message.into())
}

/// Parse the restricted MiniPy program syntax into the fixed typed IR.
pub fn parse(source: &str) -> Result<Program> {
    if source.len() > ir::MAX_SOURCE_BYTES {
        return Err(err("source exceeds MAX_SOURCE_BYTES"));
    }
    check_delimiter_depth(source)?;
    let module = ast::Suite::parse(source, "<minipy>")
        .map_err(|e| err(format!("Python syntax error: {e}")))?;
    Compiler::new().program(module)
}

// A cheap pre-parser guard also bounds deeply nested parentheses before the
// general-purpose parser gets a chance to recurse through them.
fn check_delimiter_depth(s: &str) -> Result<()> {
    // Deeply indented suites also recurse in the Python parser even though they
    // do not contain delimiters. Bound them before parsing as well.
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
    locals: Vec<Option<Type>>,
    names: Vec<String>,
    initialized: HashSet<usize>,
    q_name: Option<String>,
}
impl Compiler {
    fn new() -> Self {
        Self {
            nodes: 0,
            locals: vec![Some(Type::I64), Some(Type::I64)],
            names: vec!["batch_id".into(), "price_limit".into()],
            initialized: [0, 1].into_iter().collect(),
            q_name: None,
        }
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
        let [ast::Stmt::AsyncFunctionDef(fun)] = suite.as_slice() else {
            return Err(err("source must contain exactly one async def main"));
        };
        if fun.name.as_str() != "main"
            || !fun.decorator_list.is_empty()
            || !fun.type_params.is_empty()
            || fun.type_comment.is_some()
        {
            return Err(err("only undecorated async def main is supported"));
        }
        self.check_signature(&fun.args, fun.returns.as_deref())?;
        if fun.body.len() < 3 {
            return Err(err(
                "main must contain load_quotes, one loop, and a final record_total return",
            ));
        }
        self.node(1)?;
        let first = &fun.body[0];
        let quotes_name = match first {
            ast::Stmt::Assign(a) if a.targets.len() == 1 && a.type_comment.is_none() => {
                let n = self.target_name(&a.targets[0])?;
                if n == "batch_id" || n == "price_limit" || n == "tools" {
                    return Err(err("reserved name cannot be assigned"));
                }
                if !matches!(a.value.as_ref(), ast::Expr::Await(_)) {
                    return Err(err(
                        "first statement must await tools.load_quotes(batch_id)",
                    ));
                }
                self.check_load_call(a.value.as_ref())?;
                n
            }
            _ => {
                return Err(err(
                    "first statement must assign await tools.load_quotes(batch_id)",
                ));
            }
        };
        if quotes_name != "quotes" {
            return Err(err("load_quotes result must be assigned to 'quotes'"));
        }
        // quotes is a reserved immutable capability result, not an IR local.
        if self.names.contains(&quotes_name) {
            return Err(err("quotes variable shadows a reserved name"));
        }
        let for_indices: Vec<usize> = fun
            .body
            .iter()
            .enumerate()
            .filter_map(|(i, s)| matches!(s, ast::Stmt::For(_)).then_some(i))
            .collect();
        if for_indices.len() != 1 {
            return Err(err("main must contain exactly one for loop"));
        }
        let fi = for_indices[0];
        if fi == 0 || fi + 2 != fun.body.len() {
            return Err(err(
                "loop must follow initialization and precede the final return",
            ));
        }
        let mut init = Vec::new();
        for s in &fun.body[1..fi] {
            init.extend(self.compile_stmt(s, 0, false, true)?);
        }
        if (2..self.names.len()).any(|i| !self.initialized.contains(&i)) {
            return Err(err(
                "every local must be definitely initialized during initialization",
            ));
        }
        let ret = match &fun.body[fi + 1] {
            ast::Stmt::Return(r) => r
                .value
                .as_deref()
                .ok_or_else(|| err("return requires await tools.record_total"))?,
            _ => return Err(err("final statement must return await tools.record_total")),
        };
        let total = self.check_return(ret, &quotes_name)?;
        let loop_stmt = match &fun.body[fi] {
            ast::Stmt::For(f) => f,
            _ => unreachable!(),
        };
        if !loop_stmt.orelse.is_empty() {
            return Err(err("for-else is unsupported"));
        }
        if !matches!(loop_stmt.iter.as_ref(), ast::Expr::Name(n) if n.id.as_str()==quotes_name) {
            return Err(err("the sole for loop must iterate over quotes"));
        }
        let q = self.target_name(&loop_stmt.target)?;
        if q != "q"
            || q == "batch_id"
            || q == "price_limit"
            || q == quotes_name
            || q == "tools"
            || self.names.contains(&q)
        {
            return Err(err("loop variable shadows a reserved or initialized name"));
        }
        self.q_name = Some(q.clone());
        let before_loop = self.initialized.clone();
        let body = self.compile_block(&loop_stmt.body, 1, true, false)?;
        // Branch merges are handled by compile_stmt. Loop-only initializations
        // do not make a value available after the loop.
        self.initialized = before_loop;
        let locals = self
            .locals
            .into_iter()
            .map(|t| t.ok_or_else(|| err("internal untyped local")))
            .collect::<Result<Vec<_>>>()?;
        if locals.len() > ir::MAX_LOCALS {
            return Err(err("program exceeds MAX_LOCALS"));
        }
        Ok(Program {
            locals,
            init,
            body,
            total,
        })
    }
    fn check_signature(
        &mut self,
        args: &ast::Arguments,
        returns: Option<&ast::Expr>,
    ) -> Result<()> {
        if !args.posonlyargs.is_empty()
            || args.args.len() != 2
            || args.vararg.is_some()
            || !args.kwonlyargs.is_empty()
            || args.kwarg.is_some()
        {
            return Err(err(
                "main signature must be (batch_id: i64, price_limit: i64)",
            ));
        }
        for (a, n) in args.args.iter().zip(["batch_id", "price_limit"]) {
            if a.default.is_some()
                || a.def.arg.as_str() != n
                || !matches!(a.def.annotation.as_deref(),Some(ast::Expr::Name(x)) if x.id.as_str()=="i64")
            {
                return Err(err(
                    "main signature must be (batch_id: i64, price_limit: i64)",
                ));
            }
        }
        if !matches!(returns,Some(ast::Expr::Name(x)) if x.id.as_str()=="i64") {
            return Err(err("main must have return annotation i64"));
        }
        Ok(())
    }
    fn check_load_call(&mut self, e: &ast::Expr) -> Result<()> {
        let ast::Expr::Await(a) = e else {
            return Err(err("load_quotes must be awaited"));
        };
        let ast::Expr::Call(c) = a.value.as_ref() else {
            return Err(err("expected await tools.load_quotes(batch_id)"));
        };
        if !matches!(c.func.as_ref(),ast::Expr::Attribute(x) if x.attr.as_str()=="load_quotes" && matches!(x.value.as_ref(),ast::Expr::Name(n) if n.id.as_str()=="tools"))
            || c.args.len() != 1
            || !c.keywords.is_empty()
            || !matches!(c.args[0],ast::Expr::Name(ref n) if n.id.as_str()=="batch_id")
        {
            return Err(err("only await tools.load_quotes(batch_id) is allowed"));
        }
        Ok(())
    }
    fn check_return(&mut self, e: &ast::Expr, quotes: &str) -> Result<Expr> {
        let ast::Expr::Await(a) = e else {
            return Err(err("final return must await tools.record_total"));
        };
        let ast::Expr::Call(c) = a.value.as_ref() else {
            return Err(err("final return must await tools.record_total"));
        };
        if !matches!(c.func.as_ref(),ast::Expr::Attribute(x) if x.attr.as_str()=="record_total" && matches!(x.value.as_ref(),ast::Expr::Name(n) if n.id.as_str()=="tools"))
            || c.args.len() != 2
            || !c.keywords.is_empty()
            || !matches!(c.args[0],ast::Expr::Name(ref n) if n.id.as_str()=="batch_id")
        {
            return Err(err(
                "only await tools.record_total(batch_id, total) is allowed",
            ));
        }
        let x = self.expr(&c.args[1], Type::I64, 0, false, quotes)?;
        Ok(x.0)
    }
    fn target_name(&self, e: &ast::Expr) -> Result<String> {
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
            .ok_or_else(|| err(format!("unknown or not-yet-initialized local '{name}'")))
    }
    fn declare(&mut self, name: &str, ty: Type) -> Result<usize> {
        if ["batch_id", "price_limit", "quotes", "tools"].contains(&name)
            || self.q_name.as_deref() == Some(name)
        {
            return Err(err(format!("cannot assign reserved name '{name}'")));
        }
        if let Some(i) = self.names.iter().position(|n| n == name) {
            if self.locals[i] != Some(ty) {
                return Err(err(format!("local '{name}' changes type")));
            }
            return Ok(i);
        }
        if self.names.len() >= ir::MAX_LOCALS {
            return Err(err("program exceeds MAX_LOCALS"));
        }
        let i = self.names.len();
        self.names.push(name.to_owned());
        self.locals.push(Some(ty));
        Ok(i)
    }
    fn compile_block(
        &mut self,
        stmts: &[ast::Stmt],
        depth: usize,
        in_loop: bool,
        initializing: bool,
    ) -> Result<Vec<Stmt>> {
        let mut out = Vec::new();
        for s in stmts {
            out.extend(self.compile_stmt(s, depth, in_loop, initializing)?);
        }
        Ok(out)
    }
    fn compile_stmt(
        &mut self,
        s: &ast::Stmt,
        depth: usize,
        in_loop: bool,
        initializing: bool,
    ) -> Result<Vec<Stmt>> {
        self.node(depth)?;
        match s {
            ast::Stmt::AnnAssign(a) if initializing && !in_loop => {
                let name = self.target_name(&a.target)?;
                if !a.simple {
                    return Err(err("only simple local annotations are supported"));
                }
                let ty = match a.annotation.as_ref() {
                    ast::Expr::Name(n) if n.id.as_str() == "i64" => Type::I64,
                    ast::Expr::Name(n) if n.id.as_str() == "bool" => Type::Bool,
                    _ => return Err(err("local annotations must be i64 or bool")),
                };
                let value = a
                    .value
                    .as_deref()
                    .ok_or_else(|| err("locals must be initialized at declaration"))?;
                let (e, t) = self.expr(value, ty, depth + 1, in_loop, "quotes")?;
                if t != ty {
                    return Err(err(format!("initializer for '{name}' has the wrong type")));
                }
                let i = self.declare(&name, ty)?;
                self.initialized.insert(i);
                Ok(vec![Stmt::Set(i, e)])
            }
            ast::Stmt::Assign(a) if a.targets.len() == 1 && a.type_comment.is_none() => {
                if !initializing && !in_loop {
                    return Err(err(
                        "assignments are only allowed in initialization or loop body",
                    ));
                }
                let n = self.target_name(&a.targets[0])?;
                if in_loop && !self.names.iter().any(|name| name == &n) {
                    return Err(err("loop body cannot declare new locals"));
                }
                let (e, t) = self.expr(
                    &a.value,
                    self.target_type(&n).unwrap_or(Type::I64),
                    depth + 1,
                    in_loop,
                    "quotes",
                )?;
                let i = self.declare(&n, t)?;
                self.initialized.insert(i);
                Ok(vec![Stmt::Set(i, e)])
            }
            ast::Stmt::AugAssign(a) => {
                let n = self.target_name(&a.target)?;
                if ["batch_id", "price_limit", "quotes", "tools"].contains(&n.as_str())
                    || self.q_name.as_deref() == Some(&n)
                {
                    return Err(err(format!("cannot assign reserved name '{n}'")));
                }
                let i = self.lookup(&n)?;
                if !self.initialized.contains(&i) {
                    return Err(err(format!("local '{n}' used before initialization")));
                }
                let ty = self.locals[i].unwrap();
                if ty != Type::I64 {
                    return Err(err("augmented arithmetic requires i64"));
                }
                let op = match a.op {
                    ast::Operator::Add => BinOp::Add,
                    ast::Operator::Sub => BinOp::Sub,
                    ast::Operator::Mult => BinOp::Mul,
                    _ => return Err(err("only +=, -=, and *= are supported")),
                };
                let (rhs, t) = self.expr(&a.value, Type::I64, depth + 1, in_loop, "quotes")?;
                if t != Type::I64 {
                    return Err(err("augmented arithmetic requires i64"));
                }
                Ok(vec![Stmt::Set(
                    i,
                    Expr::Binary(op, Box::new(Expr::Local(i)), Box::new(rhs)),
                )])
            }
            ast::Stmt::If(a) => {
                let (test, t) = self.expr(&a.test, Type::Bool, depth + 1, in_loop, "quotes")?;
                if t != Type::Bool {
                    return Err(err("if condition must be bool"));
                }
                let incoming = self.initialized.clone();
                let then_body = self.compile_block(&a.body, depth + 1, in_loop, initializing)?;
                let then_set = self.initialized.clone();
                self.initialized = incoming.clone();
                let else_body = self.compile_block(&a.orelse, depth + 1, in_loop, initializing)?;
                let else_set = self.initialized.clone();
                self.initialized = then_set.intersection(&else_set).copied().collect();
                Ok(vec![Stmt::If(test, then_body, else_body)])
            }
            _ => Err(err("unsupported statement in restricted program")),
        }
    }
    fn target_type(&self, n: &str) -> Option<Type> {
        self.names
            .iter()
            .position(|x| x == n)
            .and_then(|i| self.locals[i])
    }
    fn expr(
        &mut self,
        e: &ast::Expr,
        _expected: Type,
        depth: usize,
        in_loop: bool,
        quotes: &str,
    ) -> Result<(Expr, Type)> {
        self.node(depth)?;
        match e {
            ast::Expr::Constant(c) => match &c.value {
                ast::Constant::Bool(b) => Ok((Expr::Bool(*b), Type::Bool)),
                ast::Constant::Int(n) => {
                    let s = n.to_string();
                    let v = s
                        .parse::<i64>()
                        .map_err(|_| err("integer literal is outside i64"))?;
                    Ok((Expr::Int(v), Type::I64))
                }
                _ => Err(err("only i64 and bool constants are supported")),
            },
            ast::Expr::Name(n) => {
                let name = n.id.as_str();
                if name == "True" || name == "False" {
                    return Ok((Expr::Bool(name == "True"), Type::Bool));
                }
                if name == "tools" || name == "quotes" || name == quotes {
                    return Err(err(format!("reserved name '{name}' is not an expression")));
                }
                let i = self.lookup(name)?;
                if !self.initialized.contains(&i) {
                    return Err(err(format!("local '{name}' used before initialization")));
                }
                let ty = self.locals[i].unwrap();
                Ok((Expr::Local(i), ty))
            }
            ast::Expr::Attribute(a) if in_loop => {
                let field = match (a.value.as_ref(), a.attr.as_str()) {
                    (ast::Expr::Name(n), "unit_price_cents")
                        if self.q_name.as_deref() == Some(n.id.as_str()) =>
                    {
                        Field::Price
                    }
                    (ast::Expr::Name(n), "quantity")
                        if self.q_name.as_deref() == Some(n.id.as_str()) =>
                    {
                        Field::Quantity
                    }
                    _ => return Err(err("only q.unit_price_cents and q.quantity are available")),
                };
                Ok((Expr::Field(field), Type::I64))
            }
            ast::Expr::UnaryOp(u) => match u.op {
                ast::UnaryOp::Not => {
                    let (x, t) = self.expr(&u.operand, Type::Bool, depth + 1, in_loop, quotes)?;
                    if t != Type::Bool {
                        return Err(err("not requires bool"));
                    }
                    Ok((Expr::Not(Box::new(x)), Type::Bool))
                }
                ast::UnaryOp::USub => {
                    if let ast::Expr::Constant(c) = u.operand.as_ref()
                        && let ast::Constant::Int(n) = &c.value
                    {
                        let s = n.to_string();
                        if s == "9223372036854775808" {
                            return Ok((Expr::Int(i64::MIN), Type::I64));
                        }
                    }
                    let (x, t) = self.expr(&u.operand, Type::I64, depth + 1, in_loop, quotes)?;
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
                let (l, lt) = self.expr(&b.left, Type::I64, depth + 1, in_loop, quotes)?;
                let (r, rt) = self.expr(&b.right, Type::I64, depth + 1, in_loop, quotes)?;
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
                let mut it = b.values.iter();
                let (first, t) =
                    self.expr(it.next().unwrap(), Type::Bool, depth + 1, in_loop, quotes)?;
                if t != Type::Bool {
                    return Err(err("and/or operands must be bool"));
                }
                let mut result = first;
                for rhs in it {
                    let (r, t) = self.expr(rhs, Type::Bool, depth + 1, in_loop, quotes)?;
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
                let (l, lt) = self.expr(&c.left, _expected, depth + 1, in_loop, quotes)?;
                let (r, rt) = self.expr(&c.comparators[0], lt, depth + 1, in_loop, quotes)?;
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
    const BASE: &str = r#"async def main(batch_id: i64, price_limit: i64) -> i64:
    quotes = await tools.load_quotes(batch_id)
    total: i64 = 0
    enabled: bool = True
    for q in quotes:
        if enabled and q.quantity > 0 and q.unit_price_cents <= price_limit:
            total += q.unit_price_cents * q.quantity
        else:
            total -= 0
    return await tools.record_total(batch_id, total)
"#;
    fn replace(old: &str, new: &str) -> String {
        BASE.replace(old, new)
    }

    #[test]
    fn compiles_typed_scan_and_cross_line_else() {
        let p = parse(BASE).unwrap();
        assert_eq!(p.locals, vec![Type::I64, Type::I64, Type::I64, Type::Bool]);
        assert_eq!(p.init.len(), 2);
        assert_eq!(p.body.len(), 1);
        assert!(matches!(p.body[0], Stmt::If(_, _, ref other) if other.len()==1));
    }
    #[test]
    fn supports_previous_branch_and_short_circuit_boolean() {
        let s = replace(
            "    enabled: bool = True",
            "    enabled: bool = batch_id > 0\n    if enabled:\n        total += 1\n    else:\n        total -= 1",
        );
        assert!(parse(&s).is_ok());
        let stateful = replace(
            "    enabled: bool = True",
            "    enabled: bool = True\n    previous_price: i64 = 0",
        )
        .replace(
            "if enabled and q.quantity > 0 and q.unit_price_cents <= price_limit:",
            "if enabled and q.quantity > 0 and q.unit_price_cents <= price_limit and q.unit_price_cents >= previous_price:",
        )
        .replace(
            "total += q.unit_price_cents * q.quantity",
            "total += q.unit_price_cents * q.quantity\n            previous_price = q.unit_price_cents",
        );
        assert!(parse(&stateful).is_ok());
    }
    #[test]
    fn supports_i64_min_literal_and_arithmetic_forms() {
        let s = replace(
            "    total: i64 = 0",
            "    total: i64 = -9223372036854775808",
        );
        assert!(parse(&s).is_ok());
        for op in ["+=", "-=", "*="] {
            let s = replace("total +=", &format!("total {op}"));
            assert!(parse(&s).is_ok());
        }
    }
    #[test]
    fn rejects_signature_calls_types_and_permissions() {
        let cases = [
            replace("price_limit: i64", "price_limit: bool"),
            replace(
                "quotes = await tools.load_quotes(batch_id)",
                "quotes = tools.load_quotes(batch_id)",
            ),
            replace("q.quantity > 0", "q.secret > 0"),
            replace("q.quantity > 0", "other.quantity > 0"),
            replace("total += q.unit_price_cents * q.quantity", "batch_id += 1"),
            replace(
                "await tools.load_quotes(batch_id)",
                "await other.load_quotes(batch_id)",
            ),
            replace(
                "await tools.record_total(batch_id, total)",
                "await tools.record_total(price_limit, total)",
            ),
            replace("q.quantity > 0", "q.quantity"),
            replace("q.quantity > 0", "q.quantity < price_limit < 10"),
            replace(
                "total += q.unit_price_cents * q.quantity",
                "fresh = q.quantity",
            ),
        ];
        for s in cases {
            assert!(parse(&s).is_err(), "accepted invalid source:\n{s}");
        }
    }
    #[test]
    fn rejects_uninitialized_and_type_changing_locals() {
        let wrong_initializer = replace("total: i64 = 0", "total: i64 = True");
        assert!(parse(&wrong_initializer).is_err());
        let s = replace("    total: i64 = 0", "    total: i64\n    total = 0");
        assert!(parse(&s).is_err());
        let s = replace(
            "    total: i64 = 0",
            "    total: i64 = 0\n    if enabled:\n        maybe = 2",
        );
        assert!(parse(&s).is_err());
        let s = replace("total += q.unit_price_cents * q.quantity", "total = True");
        assert!(parse(&s).is_err());
    }
    #[test]
    fn rejects_unsupported_syntax_and_structure() {
        for snippet in [
            "while True:\n        total += 1",
            "for q in quotes:\n        break",
            "total = [x for x in ()]",
            "total += q.quantity / 2",
            "q.unit_price_cents = 2",
        ] {
            let s = replace("total += q.unit_price_cents * q.quantity", snippet);
            assert!(parse(&s).is_err(), "accepted unsupported source:\n{s}");
        }
        let s = BASE.replace("    return await", "    pass\n    return await");
        assert!(parse(&s).is_err());
    }
    #[test]
    fn enforces_source_ast_and_depth_budgets() {
        assert!(parse(&" ".repeat(ir::MAX_SOURCE_BYTES + 1)).is_err());
        let deep = format!(
            "async def main(batch_id: i64, price_limit: i64) -> i64:\n    quotes = await tools.load_quotes(batch_id)\n    total: i64 = {}0{}\n    for q in quotes:\n        total += 1\n    return await tools.record_total(batch_id, total)\n",
            "(".repeat(MAX_DEPTH + 1),
            ")".repeat(MAX_DEPTH + 1)
        );
        assert!(parse(&deep).is_err());
        let mut many = String::from("    total: i64 = 0\n");
        for _ in 0..ir::MAX_NODES {
            many.push_str("    total += 1\n");
        }
        let s = format!(
            "async def main(batch_id: i64, price_limit: i64) -> i64:\n    quotes = await tools.load_quotes(batch_id)\n{many}    for q in quotes:\n        total += 1\n    return await tools.record_total(batch_id, total)\n"
        );
        assert!(parse(&s).is_err());
    }
}
