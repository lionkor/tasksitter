use std::sync::Arc;
use crate::task::Task;
use crate::error::Result;

#[derive(Debug, Clone)]
pub enum NextNode {
    Single(usize),
    Multiple(Vec<usize>),
    None,
}

#[derive(Debug)]
pub struct Node {
    task: Task,
    pub next: NextNode,
}

impl Node {
    pub fn new(task: Task, next: NextNode) -> Self {
        Self { task, next }
    }

    pub async fn run(self: &mut Self, prev_result: NodeValue) -> Result<NodeValue> {
        match &self.task {
            Task::Trivial(f) => Ok(f(prev_result)),
            Task::Generic(f) => f(prev_result),
            Task::Async(f) => f(prev_result).await,
            Task::TrivialDecider(f) => {
                let (result, next) = f(prev_result, self.next.clone());
                self.next = next;
                Ok(result)
            }
            Task::GenericDecider(f) => {
                let (result, next) = f(prev_result, self.next.clone())?;
                self.next = next;
                Ok(result)
            }
            Task::AsyncDecider(f) => {
                let (result, next) = f(prev_result, self.next.clone()).await?;
                self.next = next;
                Ok(result)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum DynValue {
    Int(i32),
    Float(f32),
    Bool(bool),
    Char(char),
    UInt(u32),
    Double(f64),
    USize(usize),
    Str(String),
    List(Vec<DynValue>),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone)]
pub enum NodeValue {
    Void,
    Value(DynValue),
    Any(Arc<Box<dyn std::any::Any + Send + Sync>>),
}
