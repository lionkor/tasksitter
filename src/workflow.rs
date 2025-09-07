use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use log::debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("Invalid index: {0}")]
    InvalidIndex(usize),
    #[error("Generic error: {0}")]
    Generic(String),
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

pub type Result<T> = std::result::Result<T, NodeError>;

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

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum WorkflowState {
    Running,
    Completed,
    NotStarted,
}

#[derive(Debug)]
pub struct Workflow {
    nodes: Vec<Node>,
    running: Vec<usize>,
    next_nodes: Vec<usize>,
    start: usize,
    state: WorkflowState,
    next_args: HashMap<usize /* to */, NodeValue>,
    results: HashMap<usize /* from */, NodeValue>,
}

impl Workflow {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            running: Vec::new(),
            next_nodes: Vec::new(),
            start: 0,
            state: WorkflowState::NotStarted,
            next_args: HashMap::new(),
            results: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    pub async fn run_next(&mut self) -> Result<WorkflowState> {
        if self.state == WorkflowState::NotStarted {
            self.running.push(self.start);
            self.state = WorkflowState::Running;
        } else if self.state == WorkflowState::Completed {
            debug!("attempted to run a completed workflow");
            return Ok(self.state);
        } else if self.running.is_empty() {
            self.state = WorkflowState::Completed;
            return Ok(self.state);
        }
        self.next_nodes.clear();
        // run the actual node's task
        for i in &self.running {
            let arg = self.next_args.remove(i).unwrap_or(NodeValue::Void);
            let node = &mut self.nodes.get_mut(*i).ok_or(NodeError::InvalidIndex(*i))?;
            // TODO: propagate this error with a node ID or similar
            let res = node.run(arg).await?;
            match &node.next {
                NextNode::Single(i) => {
                    self.next_args.insert(*i, res);
                    self.next_nodes.push(*i);
                }
                NextNode::Multiple(items) => {
                    for item in items {
                        self.next_args.insert(*item, res.clone());
                        self.next_nodes.push(*item);
                    }
                }
                NextNode::None => {
                    if !matches!(res, NodeValue::Void) {
                        self.results.insert(*i, res);
                    }
                }
            };
        }
        if self.next_nodes.is_empty() {
            self.state = WorkflowState::Completed;
        } else {
            std::mem::swap(&mut self.running, &mut self.next_nodes);
        }
        Ok(self.state)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_node() {
        let mut workflow = Workflow::new();
        let node = Node::new(Task::Trivial(|_| NodeValue::Void), NextNode::None);
        let idx = workflow.add_node(node);
        assert_eq!(idx, 0);
        assert_eq!(workflow.nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_run_trivial_node() {
        let mut workflow = Workflow::new();
        let node = Node::new(
            Task::Trivial(|_| NodeValue::Value(DynValue::Int(42))),
            NextNode::None,
        );
        workflow.add_node(node);
        let state = workflow.run_next().await.unwrap();
        assert_eq!(state, WorkflowState::Completed);
        let res = workflow.results.get(&0);
        assert_eq!(res.is_some(), true);
        let res = res.unwrap();
        assert!(match res {
            NodeValue::Value(DynValue::Int(42)) => true,
            _ => false,
        });
    }

    #[tokio::test]
    async fn test_run_trivial_decider_node() {
        // This decider node will always send to node 1 if input is Int(1), else to node 2
        fn decider_fn(input: NodeValue, _next: NextNode) -> (NodeValue, NextNode) {
            match input {
                NodeValue::Value(DynValue::Int(1)) => (
                    NodeValue::Value(DynValue::Str("one".to_string())),
                    NextNode::Single(1),
                ),
                _ => (
                    NodeValue::Value(DynValue::Str("other".to_string())),
                    NextNode::Single(2),
                ),
            }
        }

        let mut workflow = Workflow::new();

        // Node 0: decider
        let node0 = Node::new(Task::TrivialDecider(decider_fn), NextNode::Single(1));
        let idx0 = workflow.add_node(node0);

        // Node 1: trivial, returns 100
        let node1 = Node::new(
            Task::Trivial(|_| NodeValue::Value(DynValue::Int(100))),
            NextNode::None,
        );
        let idx1 = workflow.add_node(node1);

        // Node 2: trivial, returns 200
        let node2 = Node::new(
            Task::Trivial(|_| NodeValue::Value(DynValue::Int(200))),
            NextNode::None,
        );
        let _idx2 = workflow.add_node(node2);

        // Set start node to 0
        workflow.start = idx0;

        // Provide input to decider node
        workflow
            .next_args
            .insert(idx0, NodeValue::Value(DynValue::Int(1)));

        // Run decider node
        let state = workflow.run_next().await.unwrap();
        assert_eq!(state, WorkflowState::Running);

        // Should have scheduled node 1
        assert_eq!(workflow.running, vec![idx1]);
        // No need to run this node, that's not the point of this test

        // Now test with input that goes to node 2
        let mut workflow = Workflow::new();
        let node0 = Node::new(Task::TrivialDecider(decider_fn), NextNode::Single(1));
        let idx0 = workflow.add_node(node0);
        let node1 = Node::new(
            Task::Trivial(|_| NodeValue::Value(DynValue::Int(100))),
            NextNode::None,
        );
        let _idx1 = workflow.add_node(node1);
        let node2 = Node::new(
            Task::Trivial(|_| NodeValue::Value(DynValue::Int(200))),
            NextNode::None,
        );
        let idx2 = workflow.add_node(node2);
        workflow.start = idx0;
        workflow
            .next_args
            .insert(idx0, NodeValue::Value(DynValue::Int(5)));

        let state = workflow.run_next().await.unwrap();
        assert_eq!(state, WorkflowState::Running);
        assert_eq!(workflow.running, vec![idx2]);
        // again, no need to run further
    }

    #[tokio::test]
    async fn test_run_generic_node() {
        fn generic_fn(_input: NodeValue) -> Result<NodeValue> {
            Ok(NodeValue::Value(DynValue::Str("generic".to_string())))
        }
        let mut workflow = Workflow::new();
        let node = Node::new(Task::Generic(generic_fn), NextNode::None);
        workflow.add_node(node);
        let state = workflow.run_next().await.unwrap();
        assert_eq!(state, WorkflowState::Completed);
        let res = workflow.results.get(&0).unwrap();
        assert!(matches!(res, NodeValue::Value(DynValue::Str(s)) if s == "generic"));
    }

    #[tokio::test]
    async fn test_run_async_node() {
        async fn async_fn(_input: NodeValue) -> Result<NodeValue> {
            Ok(NodeValue::Value(DynValue::Int(123)))
        }
        let mut workflow = Workflow::new();
        let node = Node::new(Task::from_async_fn(async_fn), NextNode::None);
        workflow.add_node(node);
        let state = workflow.run_next().await.unwrap();
        assert_eq!(state, WorkflowState::Completed);
        let res = workflow.results.get(&0).unwrap();
        assert!(matches!(res, NodeValue::Value(DynValue::Int(123))));
    }

    #[tokio::test]
    async fn test_run_generic_decider_node() {
        fn generic_decider_fn(_input: NodeValue, _next: NextNode) -> Result<(NodeValue, NextNode)> {
            Ok((
                NodeValue::Value(DynValue::Str("decided".to_string())),
                NextNode::Single(1),
            ))
        }
        let mut workflow = Workflow::new();
        let node0 = Node::new(
            Task::GenericDecider(generic_decider_fn),
            NextNode::Single(1),
        );
        let idx0 = workflow.add_node(node0);
        let node1 = Node::new(Task::Trivial(|_| NodeValue::Void), NextNode::None);
        let idx1 = workflow.add_node(node1);
        workflow.start = idx0;
        let state = workflow.run_next().await.unwrap();
        assert_eq!(state, WorkflowState::Running);
        assert_eq!(workflow.running, vec![idx1]);
    }

    #[tokio::test]
    async fn test_run_async_decider_node() {
        async fn async_decider_fn(
            _input: NodeValue,
            _next: NextNode,
        ) -> Result<(NodeValue, NextNode)> {
            Ok((NodeValue::Value(DynValue::Int(999)), NextNode::Single(1)))
        }
        let mut workflow = Workflow::new();
        let node0 = Node::new(
            Task::from_async_decider_fn(async_decider_fn),
            NextNode::Single(1),
        );
        let idx0 = workflow.add_node(node0);
        let node1 = Node::new(Task::Trivial(|_| NodeValue::Void), NextNode::None);
        let idx1 = workflow.add_node(node1);
        workflow.start = idx0;
        let state = workflow.run_next().await.unwrap();
        assert_eq!(state, WorkflowState::Running);
        assert_eq!(workflow.running, vec![idx1]);
    }
}
