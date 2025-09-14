use crate::error::{NodeError, Result};
use crate::node::NextNode;
use crate::node::{Node, NodeValue};
use log::debug;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum WorkflowState {
    Running,
    Completed,
    NotStarted,
}

/// Represents a board full of [Node]s, connected by edges, to form a (potentially cyclical) graph.
/// Each node contains a task, which can be executed.
#[derive(Debug)]
pub struct Workflow {
    nodes: Vec<Node>,
    running: Vec<usize>,
    next_nodes: Vec<usize>,
    start: usize,
    state: WorkflowState,
    next_args: HashMap<usize /* to */, NodeValue>,
    pub results: HashMap<usize /* from */, NodeValue>,
}

impl Workflow {
    /// Constructs a new, empty workflow.
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

    /// Adds a node to the workflow, returning its index.
    ///
    /// These indices are used for [NextNode] references.
    pub fn add_node(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// Runs the next set of nodes in the workflow. If the workflow has not yet started,
    /// it will start with the node at index `start` (default 0).
    /// If the workflow is already running, it will run the next set of nodes
    /// determined by the previous run.
    /// If the workflow has completed, it will return `WorkflowState::Completed`, and the
    /// results of all terminating nodes can be found in [results](Workflow::results).
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

#[cfg(test)]
mod tests {
    use crate::{node::DynValue, task::{AsyncRunnableDecider, Task}};

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

    struct MyAsyncDecider;

    #[async_trait::async_trait]
    impl AsyncRunnableDecider for MyAsyncDecider {
        async fn run(
            &mut self,
            input: crate::node::NodeValue,
            _next: crate::node::NextNode,
        ) -> crate::error::Result<(crate::node::NodeValue, crate::node::NextNode)> {
            use crate::node::{NodeValue, NextNode};
            use crate::node::DynValue;

            // If input is Int(7), go to node 1, else to node 2
            match input {
                NodeValue::Value(DynValue::Int(7)) => Ok((
                    NodeValue::Value(DynValue::Int(1000)),
                    NextNode::Single(1),
                )),
                _ => Ok((
                    NodeValue::Value(DynValue::Int(2000)),
                    NextNode::Single(2),
                )),
            }
        }
    }

    #[tokio::test]
    async fn test_run_custom_async_decider_node() {
        use crate::task::Task;

        let mut workflow = Workflow::new();

        // Node 0: custom async decider
        let decider = MyAsyncDecider;
        let node0 = Node::new(
            Task::AsyncCustomDecider(Box::new(decider)),
            NextNode::Single(1),
        );
        let idx0 = workflow.add_node(node0);

        // Node 1: trivial, returns 1000
        let node1 = Node::new(
            Task::Trivial(|_| NodeValue::Value(DynValue::Int(1000))),
            NextNode::None,
        );
        let idx1 = workflow.add_node(node1);

        // Node 2: trivial, returns 2000
        let node2 = Node::new(
            Task::Trivial(|_| NodeValue::Value(DynValue::Int(2000))),
            NextNode::None,
        );
        let _idx2 = workflow.add_node(node2);

        workflow.start = idx0;

        // Provide input to decider node: Int(7) should go to node 1
        workflow
            .next_args
            .insert(idx0, NodeValue::Value(DynValue::Int(7)));

        let state = workflow.run_next().await.unwrap();
        assert_eq!(state, WorkflowState::Running);
        assert_eq!(workflow.running, vec![idx1]);

        // Now try with input that should go to node 2
        let mut workflow = Workflow::new();
        let decider = MyAsyncDecider;
        let node0 = Node::new(
            Task::AsyncCustomDecider(Box::new(decider)),
            NextNode::Single(1),
        );
        let idx0 = workflow.add_node(node0);
        let node1 = Node::new(
            Task::Trivial(|_| NodeValue::Value(DynValue::Int(1000))),
            NextNode::None,
        );
        let _idx1 = workflow.add_node(node1);
        let node2 = Node::new(
            Task::Trivial(|_| NodeValue::Value(DynValue::Int(2000))),
            NextNode::None,
        );
        let idx2 = workflow.add_node(node2);

        workflow.start = idx0;
        workflow
            .next_args
            .insert(idx0, NodeValue::Value(DynValue::Int(3)));

        let state = workflow.run_next().await.unwrap();
        assert_eq!(state, WorkflowState::Running);
        assert_eq!(workflow.running, vec![idx2]);
    }
}
