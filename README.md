# tasksitter

Graph-based workflow engine for Rust. Model, execute, and control arbitrary task graphs (DAGs or cyclic) with async/sync nodes and dynamic routing.

## Why use a runtime task graph?

It's tempting to hard-code workflow logic or teach non-programmers how to write code. With a runtime graph, you can assemble, modify, and execute workflows on the fly—driven by config, user input, or external state. This is useful when the sequence, branching, or composition of tasks isn’t fixed, or needs to adapt without redeploying, and allows parts of the graph to be pre-defined, while other parts of the graph can be changed by the user.

`tasksitter` provides primitives for building, connecting, and running these graphs. You get flexible execution, dynamic routing, and support for both synchronous and asynchronous tasks—all without committing to a static pipeline or imperative control flow. You can introspect the state of the graph at runtime, pause/resume between tasks, and handle errors gracefully.

## High-Level Overview

- Define a workflow as a directed graph of nodes.
- Each node encapsulates a task (sync/async, decider, custom).
- Nodes pass values/results to successors.
- Execution proceeds by running nodes, propagating outputs, and following graph edges until completion.
