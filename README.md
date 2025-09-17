# tasksitter

Graph-based workflow engine for Rust. Model, execute, and control arbitrary task graphs (DAGs or cyclic) with async/sync nodes and dynamic routing.

## High-Level Overview

- Define a workflow as a directed graph of nodes.
- Each node encapsulates a task (sync/async, decider, custom).
- Nodes pass values/results to successors.
- Execution proceeds by running nodes, propagating outputs, and following graph edges until completion.
