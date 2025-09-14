//! This library provides functionality for defining and executing graph-based workflows.
//!
//! To get started, create a [`Workflow`](crate::workflow::Workflow), add [`Node`](crate::node::Node)s
//! with associated [`Task`](crate::task::Task)s, and then run the workflow's
//! [`run_next`](crate::workflow::Workflow::run_next) method to execute the graph until it returns
//! [`WorkflowState::Completed`](crate::workflow::WorkflowState::Completed) or an error occurs.

pub mod error;
pub mod node;
pub mod task;
pub mod workflow;
