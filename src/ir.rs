//! Restricted typed semantic IR. Names and schemas are resolved at compilation.
pub const MAX_LOCALS: usize = 32;
pub const MAX_BUFFERS: usize = 32;
pub const MAX_SOURCE_BYTES: usize = 16 * 1024;
pub const MAX_NODES: usize = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    I64,
    Bool,
}
#[derive(Clone, Copy, Debug)]
pub enum Field {
    Price,
    Quantity,
}
#[derive(Clone, Copy, Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}
#[derive(Clone, Debug)]
pub enum Expr {
    Int(i64),
    Bool(bool),
    Local(usize),
    #[cfg(feature = "tool-json")]
    JsonI64 {
        slot: usize,
        root: usize,
    },
    #[cfg(feature = "tool-json")]
    JsonView {
        slot: usize,
        root: usize,
    },
    Field(Field),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}
#[derive(Clone, Debug)]
pub enum Stmt {
    Set(usize, Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    For(usize, Vec<Stmt>),
    Load(usize, Expr),
    Record(usize, Expr, Expr),
    #[cfg(feature = "tool-json")]
    Tool {
        call: usize,
        target: usize,
        json: bool,
        args: Vec<Expr>,
    },
    Print(Expr, Type),
    Eval(Expr),
    Return(Expr),
}
#[derive(Clone, Debug)]
pub struct Program {
    pub locals: Vec<Type>,
    /// Host scalar inputs occupy the leading scalar slots.
    pub inputs: usize,
    /// Quote owners occupy a separate, statically numbered namespace.
    pub buffers: usize,
    pub statements: Vec<Stmt>,
    pub legacy: bool,
    #[cfg(feature = "tool-json")]
    pub json: crate::tool::Plan,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError(pub String);
impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for CompileError {}
