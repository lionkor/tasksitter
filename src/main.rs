mod workflow;

use log::info;
use macroquad::prelude::*;
use macroquad::window::next_frame;

#[macroquad::main("Graph-Flows!")]
async fn main() {
    simple_logger::init_with_level(log::Level::Trace).unwrap();

    info!("Welcome to Graph-Flows!");
    let node0 = workflow::Node::new(
        workflow::Task::Trivial(|_node| {
            info!("Running node0! Next node: 1");
            workflow::NodeValue::Void
        }),
        workflow::NextNode::Single(1),
    );

    // Insert a decider node at node1
    let node1 = workflow::Node::new(
        workflow::Task::TrivialDecider(|_node, next| {
            use std::io::{self, Write};
            info!("Running decider node1! Deciding next node...");
            let mut chosen_next = workflow::NextNode::None;
            match next {
                workflow::NextNode::Multiple(ref v) if !v.is_empty() => {
                    println!("Choose the next node to run:");
                    for (i, idx) in v.iter().enumerate() {
                        println!("  {}: Node {}", i, idx);
                    }
                    print!("Enter your choice: ");
                    io::stdout().flush().unwrap();
                    let mut input = String::new();
                    if let Ok(_) = io::stdin().read_line(&mut input) {
                        if let Ok(choice) = input.trim().parse::<usize>() {
                            if let Some(&node_idx) = v.get(choice) {
                                chosen_next = workflow::NextNode::Single(node_idx);
                            }
                        }
                    }
                    if let workflow::NextNode::None = chosen_next {
                        println!("Invalid choice, defaulting to first node.");
                        chosen_next = workflow::NextNode::Single(v[0]);
                    }
                }
                _ => {
                    chosen_next = workflow::NextNode::None;
                }
            }
            (workflow::NodeValue::Void, chosen_next)
        }),
        workflow::NextNode::Multiple(vec![2, 3]),
    );

    let node2 = workflow::Node::new(
        workflow::Task::Async(|_node| Box::pin(async {
            info!("Running node2! Next node: None");
            Ok(workflow::NodeValue::Void)
        })),
        workflow::NextNode::None,
    );

    let node3 = workflow::Node::new(
        workflow::Task::Trivial(|_node| {
            info!("Running node3! Next node: None");
            workflow::NodeValue::Void
        }),
        workflow::NextNode::None,
    );

    let mut workflow = workflow::Workflow::new();
    let idx0 = workflow.add_node(node0);
    let idx1 = workflow.add_node(node1);
    let idx2 = workflow.add_node(node2);
    let idx3 = workflow.add_node(node3);

    info!(
        "Workflow nodes added at indices: {}, {}, {}, {}",
        idx0, idx1, idx2, idx3
    );

    let mut state = workflow::WorkflowState::NotStarted;

    while state != workflow::WorkflowState::Completed {
        match workflow.run_next().await {
            Ok(s) => {
                state = s;
                info!("Workflow state: {:?}", state);
            }
            Err(e) => {
                info!("Error running workflow: {}", e);
                break;
            }
        }
    }

    loop {
        next_frame().await
    }
}
