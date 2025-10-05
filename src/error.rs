use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("Invalid index: {0}")]
    InvalidIndex(usize),
    #[error("Generic error: {0}")]
    Generic(String),
    #[error("Error in Node {0}: {1}")]
    GenericInNode(usize, Box<dyn std::error::Error>),
    #[error("Error: {0}")]
    WithInner(Box<dyn std::error::Error>),
}
pub type Result<T> = std::result::Result<T, NodeError>;
