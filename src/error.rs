use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("Invalid index: {0}")]
    InvalidIndex(usize),
    #[error("Generic error: {0}")]
    Generic(String),
}
pub type Result<T> = std::result::Result<T, NodeError>;
