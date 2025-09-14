use async_trait::async_trait;

use crate::error::Result;
use crate::node::{NextNode, NodeValue};
use std::pin::Pin;

pub trait Runnable {
    fn run(&mut self, arg: NodeValue) -> Result<NodeValue>;
}

pub trait RunnableDecider {
    fn run(&mut self, arg: NodeValue, next: NextNode) -> Result<(NodeValue, NextNode)>;
}

#[async_trait]
pub trait AsyncRunnable {
    async fn run(&mut self, arg: NodeValue) -> Result<NodeValue>;
}

#[async_trait]
pub trait AsyncRunnableDecider {
    async fn run(&mut self, arg: NodeValue, next: NextNode) -> Result<(NodeValue, NextNode)>;
}

/// Represents a function which does some work in a [Node](crate::node::Node).
///
/// Nodes can be synchronous or asynchronous, and can optionally be "deciders" which
/// determine the next node(s) to execute (selected only from the set of existing "successors").
///
/// All tasks can be constructed directly, but the async variants are easier to construct via
/// the helper functions [Task::from_async_fn] and [Task::from_async_decider_fn]. Please note that
/// the async variants contain boxed function pointers, which may be undesirable if you're looking
/// for cache-friendliness.
pub enum Task {
    /// A simple task which cannot fail.
    Trivial(fn(NodeValue) -> NodeValue),
    /// A normal task which can fail.
    Generic(fn(NodeValue) -> Result<NodeValue>),
    /// An asynchronous task which can fail.
    Async(
        Box<
            dyn Fn(NodeValue) -> Pin<Box<dyn Future<Output = Result<NodeValue>> + Send>>
                + Send
                + Sync,
        >,
    ),
    /// A simple decider task which cannot fail.
    TrivialDecider(fn(NodeValue, NextNode) -> (NodeValue, NextNode)),
    /// A normal decider task which can fail.
    GenericDecider(fn(NodeValue, NextNode) -> Result<(NodeValue, NextNode)>),
    /// An asynchronous decider task which can fail.
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
    /// A complex task implemented via the [Runnable] trait.
    Custom(Box<dyn Runnable>),
    /// A complex decider task implemented via the [RunnableDecider] trait.
    CustomDecider(Box<dyn RunnableDecider>),
    /// A complex asynchronous task implemented via the [AsyncRunnable] trait.
    AsyncCustom(Box<dyn AsyncRunnable>),
    /// A complex asynchronous decider task implemented via the [AsyncRunnableDecider] trait.
    AsyncCustomDecider(Box<dyn AsyncRunnableDecider>),
}

impl std::fmt::Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Task::Trivial(fptr) => write!(f, "Task::Trivial({:p})", *fptr as *const ()),
            Task::Generic(fptr) => write!(f, "Task::Generic({:p})", *fptr as *const ()),
            Task::Async(fptr) => write!(f, "Task::Async({:p})", &**fptr as *const _),
            Task::TrivialDecider(fptr) => write!(f, "Task::TrivialDecider({:p})", *fptr as *const ()),
            Task::GenericDecider(fptr) => write!(f, "Task::GenericDecider({:p})", *fptr as *const ()),
            Task::AsyncDecider(fptr) => write!(f, "Task::AsyncDecider({:p})", &**fptr as *const _),
            Task::Custom(ptr) => write!(f, "Task::Custom({:p})", &**ptr as *const _),
            Task::CustomDecider(ptr) => write!(f, "Task::CustomDecider({:p})", &**ptr as *const _),
            Task::AsyncCustom(ptr) => write!(f, "Task::AsyncCustom({:p})", &**ptr as *const _),
            Task::AsyncCustomDecider(ptr) => write!(f, "Task::AsyncCustomDecider({:p})", &**ptr as *const _),
        }
    }
}

impl Task {
    /// Constructs a [Task::Async] from an async function or closure.
    /// This hides the complexity of boxing and pinning the future, but
    /// keep in mind the performance implications of async tasks in general.
    pub fn from_async_fn<F, Fut>(f: F) -> Self
    where
        F: Fn(NodeValue) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<NodeValue>> + Send + 'static,
    {
        Task::Async(Box::new(move |arg| Box::pin(f(arg))))
    }

    /// Constructs a [Task::AsyncDecider] from an async function or closure.
    /// This hides the complexity of boxing and pinning the future, but
    /// keep in mind the performance implications of async tasks in general.
    pub fn from_async_decider_fn<F, Fut>(f: F) -> Self
    where
        F: Fn(NodeValue, NextNode) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(NodeValue, NextNode)>> + Send + 'static,
    {
        Task::AsyncDecider(Box::new(move |arg, next| Box::pin(f(arg, next))))
    }
}
