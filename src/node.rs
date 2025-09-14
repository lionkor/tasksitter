use crate::error::Result;
use crate::task::Task;
use std::sync::Arc;

/// Represents the next [Node]s (by index) to execute after the current node.
///
/// The `usize` corresponds to an index of a [Node] in the [Workflow](crate::workflow::Workflow)'s
/// collection of `nodes`.
/// These indices are known before the graph is constructed, and the indices are returned
/// by [Workflow::add_node](crate::workflow::Workflow::add_node) as confirmation.
#[derive(Debug, Clone)]
pub enum NextNode {
    /// Execute a single next node.
    Single(usize),
    /// Execute multiple next nodes.
    Multiple(Vec<usize>),
    /// No next node to execute (signifies an end-node).
    None,
}

/// Represents a node in the graph. Each node has an associated [Task], and one or more [NextNode]s.
#[derive(Debug)]
pub struct Node {
    task: Task,
    /// The successors of this node in the graph. One or more of these nodes may be executed
    /// as the [next nodes(s)](Node::next). The actual nodes to be executed are determined by the
    /// [decider tasks](crate::task::Task::TrivialDecider) in [Node::run], if any.
    ///
    /// The successors are fixed when the graph is constructed, while the [next nodes](Node::next) may change
    /// during execution.
    pub successors: NextNode,
    /// The next node(s) to execute after this one. This set of next nodes might be reduced from
    /// [successors](Node::successors) by [decider tasks](crate::task::Task::TrivialDecider) in [Node::run].
    pub next: NextNode,
}

impl Node {
    /// Constructs a new node with a given task and successor node(s).
    pub fn new(task: Task, successors: NextNode) -> Self {
        Self {
            task,
            successors: successors.clone(),
            next: successors,
        }
    }

    /// Runs the node's [task](Task) with the given input [NodeValue], returning the output [NodeValue].
    /// This method is idempotent, and can be called multiple times with the same input to produce the same output,
    /// as long as the task itself is idempotent, and the node's [successors](Node::successors) are unchanged.
    ///
    /// If the node's task is a [decider](crate::task::Task::TrivialDecider), it may also modify the node's
    /// [next nodes](Node::next).
    ///
    /// This method runs the task directly, and is only non-synchronous if the task is one of the
    /// [async variants](crate::task::Task::Async). For that reason, the [Result] is returned directly, rather
    /// than wrapped in a future, and corresponds to the result returned by the task itself (or the return value
    /// wrapped in a result, for "trivial" tasks).
    pub async fn run(self: &mut Self, prev_result: NodeValue) -> Result<NodeValue> {
        // reset next nodes to ensure runs are idempotent
        self.next = self.successors.clone();
        match &mut self.task {
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
            Task::Custom(runnable) => runnable.run(prev_result),
            Task::CustomDecider(runnable_decider) => {
                let (result, next) = runnable_decider.run(prev_result, self.next.clone())?;
                self.next = next;
                Ok(result)
            }
            Task::AsyncCustom(async_runnable) => async_runnable.run(prev_result).await,
            Task::AsyncCustomDecider(async_runnable_decider) => {
                let (result, next) = async_runnable_decider.run(prev_result, self.next.clone()).await?;
                self.next = next;
                Ok(result)
            }
        }
    }
}

/// Represents a dynamically-typed value which can be passed between nodes in the graph.
/// This enum supports a variety of primitive types, as well as lists and byte arrays.
///
/// For more complex types, consider using [NodeValue::Any].
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

/// Represents the value produced or consumed by a [Node]. This enum can represent the absence of a
/// value ([NodeValue::Void]), a dynamically-typed value ([NodeValue::Value]), or any arbitrary type
/// ([NodeValue::Any]).
///
/// Please note that the `Any` variant uses `Arc<Box<...>>`, which means that cache-friendliness is sacrificed
/// for flexibility. Use this variant sparingly, especially in hot paths. However, it's still obviously
/// preferrable to serializing complex types into a [DynValue] or similar.
#[derive(Debug, Clone)]
pub enum NodeValue {
    Void,
    Value(DynValue),
    Any(Arc<Box<dyn std::any::Any + Send + Sync>>),
}
