# graph-flows

Graph-based workflow engine for Rust. Model, execute, and control arbitrary task graphs (DAGs or cyclic) with async/sync nodes and dynamic routing.

## High-Level Overview

- Define a workflow as a directed graph of nodes.
- Each node encapsulates a task (sync/async, decider, custom).
- Nodes pass values/results to successors.
- Execution proceeds by running nodes, propagating outputs, and following graph edges until completion.

## Low-Level Details

- `Workflow`: Owns nodes, tracks execution state, manages scheduling.
- `Node`: Wraps a `Task`, holds successor indices (`NextNode`), manages dynamic routing.
- `Task`: Enum for trivial, generic, async, decider, and trait-based tasks.
- `NodeValue`/`DynValue`: Typed payloads passed between nodes.
- Execution: Call `run_next()` to process active nodes, update state, and schedule successors. Decider nodes can alter routing at runtime.

## Example

```graph-flows/README.md#L27-49
use graph_flows::{
    workflow::Workflow,
    node::{Node, NextNode, NodeValue, DynValue},
    task::Task,
};

#[tokio::main]
async fn main() {
    let mut wf = Workflow::new();

    // Node 0: returns 42
    let n0 = Node::new(Task::Trivial(|_| NodeValue::Value(DynValue::Int(42))), NextNode::None);
    wf.add_node(n0);

    // Run workflow
    let state = wf.run_next().await.unwrap();
    assert_eq!(state, graph_flows::workflow::WorkflowState::Completed);
    assert_eq!(
        wf.results.get(&0),
        Some(&NodeValue::Value(DynValue::Int(42)))
    );
}
```

## See Also

- `src/workflow.rs`, `src/node.rs`, `src/task.rs` for API.
- Supports async, custom, and decider nodes.
- Handles errors via `Result<NodeValue>`.