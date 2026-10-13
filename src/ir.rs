//! Restricted, typed scan IR. Only the frontend constructs this from source.
pub const MAX_LOCALS: usize = 32;
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
    Field(Field),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}
#[derive(Clone, Debug)]
pub enum Stmt {
    Set(usize, Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
}
#[derive(Clone, Debug)]
pub struct Program {
    /// Slots 0 and 1 are batch_id and price_limit. No other parameters.
    pub locals: Vec<Type>,
    pub init: Vec<Stmt>,
    pub body: Vec<Stmt>,
    pub total: Expr,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError(pub String);
impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for CompileError {}
