use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use log::{debug, info};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeError {}

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
    Any(Arc<Box<dyn std::any::Any>>),
}

pub type Result<T> = std::result::Result<T, NodeError>;

#[derive(Debug)]
pub enum Task {
    Trivial(fn(NodeValue) -> NodeValue),
    Generic(fn(NodeValue) -> Result<NodeValue>),
    Async(fn(NodeValue) -> Pin<Box<dyn Future<Output = Result<NodeValue>> + Send>>),
    TrivialDecider(fn(NodeValue, NextNode) -> (NodeValue, NextNode)),
    GenericDecider(fn(NodeValue, NextNode) -> Result<(NodeValue, NextNode)>),
    AsyncDecider(
        fn(
            NodeValue,
            NextNode,
        ) -> Pin<Box<dyn Future<Output = Result<(NodeValue, NextNode)>> + Send>>,
    ),
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
    result: HashMap<usize, NodeValue>,
}

impl Workflow {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            running: Vec::new(),
            next_nodes: Vec::new(),
            start: 0,
            state: WorkflowState::NotStarted,
            result: HashMap::new(),
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
            let arg = self.result.remove(i).unwrap_or(NodeValue::Void);
            let node = &mut self.nodes[*i];
            // TODO: propagate this error with a node ID or similar
            let res = node.run(arg).await?;
            match &node.next {
                NextNode::Single(i) => {
                    self.result.insert(*i, res);
                    self.next_nodes.push(*i);
                }
                NextNode::Multiple(items) => {
                    for item in items {
                        self.result.insert(*item, res.clone());
                        self.next_nodes.push(*item);
                    }
                }
                NextNode::None => (),
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
        match self.task {
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
