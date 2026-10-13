#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskError {
    Overflow,
    Budget,
    Cancelled,
    Permission,
    Buffer,
    Input,
    Provider(String),
}
impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for TaskError {}
