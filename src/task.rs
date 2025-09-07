use std::pin::Pin;
use crate::node::{NextNode, NodeValue};
use crate::error::Result;

pub enum Task {
    Trivial(fn(NodeValue) -> NodeValue),
    Generic(fn(NodeValue) -> Result<NodeValue>),
    Async(
        Box<
            dyn Fn(NodeValue) -> Pin<Box<dyn Future<Output = Result<NodeValue>> + Send>>
                + Send
                + Sync,
        >,
    ),
    TrivialDecider(fn(NodeValue, NextNode) -> (NodeValue, NextNode)),
    GenericDecider(fn(NodeValue, NextNode) -> Result<(NodeValue, NextNode)>),
    AsyncDecider(
        Box<
            dyn Fn(
                    NodeValue,
                    NextNode,
                )
                    -> Pin<Box<dyn Future<Output = Result<(NodeValue, NextNode)>> + Send>>
                + Send
                + Sync,
        >,
    ),
}

impl std::fmt::Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Task::Trivial(_) => write!(f, "Task::Trivial"),
            Task::Generic(_) => write!(f, "Task::Generic"),
            Task::Async(_) => write!(f, "Task::Async"),
            Task::TrivialDecider(_) => write!(f, "Task::TrivialDecider"),
            Task::GenericDecider(_) => write!(f, "Task::GenericDecider"),
            Task::AsyncDecider(_) => write!(f, "Task::AsyncDecider"),
        }
    }
}

impl Task {
    pub fn from_async_fn<F, Fut>(f: F) -> Self
    where
        F: Fn(NodeValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<NodeValue>> + Send + 'static,
    {
        Task::Async(Box::new(move |arg| Box::pin(f(arg))))
    }

    pub fn from_async_decider_fn<F, Fut>(f: F) -> Self
    where
        F: Fn(NodeValue, NextNode) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(NodeValue, NextNode)>> + Send + 'static,
    {
        Task::AsyncDecider(Box::new(move |arg, next| Box::pin(f(arg, next))))
    }
}
