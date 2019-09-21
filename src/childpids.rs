use std::collections::{HashMap, LinkedList};

use sysinfo::{Process, ProcessExt, System, SystemExt};

#[derive(Debug)]
pub struct Node<T> {
    value: T,
    children: Vec<Node<T>>,
}

type ProcessNode = Node<i32>;
type PidT = i32;

fn get_children(parent_pid: PidT, processes: &HashMap<PidT, Process>) -> Vec<ProcessNode> {
    let mut children = Vec::new();
    for process in processes.values() {
        if let Some(parent) = process.parent() {
            if parent == parent_pid {
                children.push(ProcessNode {
                    value: process.pid(),
                    children: get_children(process.pid(), processes),
                })
            }
        }
    }
    children
}

fn get_child_process_tree(parent_pid: PidT) -> Vec<ProcessNode> {
    let sys = System::new();
    let processes = sys.get_process_list();
    get_children(parent_pid, processes)
}

fn flatten_tree_breadth_first<T>(root: Node<T>, mut children: Vec<T>) -> Vec<T> {
    let mut queue: LinkedList<Node<T>> = LinkedList::new();
    queue.push_back(root);
    let mut i = 0;
    while let Some(current) = queue.pop_front() {
        if i > 0 {
            children.push(current.value);
        }
        for child in current.children {
            queue.push_back(child);
        }
        i += 1;
    }
    children
}

pub fn get_children_pids(parent_pid: PidT) -> Vec<PidT> {
    let children_tree = get_child_process_tree(parent_pid);
    let mut children = Vec::new();
    children.push(parent_pid);
    flatten_tree_breadth_first(ProcessNode {
        value: parent_pid,
        children: children_tree,
    }, children)
}

#[test]
fn flatten_tree_breadth_first_test() {
    let tree: Node<i32> = Node {
        value: 20363,
        children: vec![
            Node {
                value: 20365,
                children: vec![
                    Node {
                        value: 21549,
                        children: vec![
                            Node {
                                value: 21587,
                                children: vec![],
                            },
                            Node {
                                value: 21586,
                                children: vec![
                                    Node {
                                        value: 21660,
                                        children: vec![
                                            Node {
                                                value: 21661,
                                                children: vec![],
                                            },
                                        ],
                                    },
                                ],
                            },
                        ],
                    },
                    Node {
                        value: 20401,
                        children: vec![
                            Node {
                                value: 20449,
                                children: vec![],
                            },
                            Node {
                                value: 20448,
                                children: vec![
                                    Node {
                                        value: 20531,
                                        children: vec![
                                            Node {
                                                value: 20532,
                                                children: vec![],
                                            },
                                        ],
                                    },
                                ],
                            },
                        ],
                    },
                    Node {
                        value: 20403,
                        children: vec![],
                    },
                ],
            },
        ],
    };
    let actual = flatten_tree_breadth_first(tree, Vec::new());
    let expected = [
        //20363,
        20365,
        21549,
        20401,
        20403,
        21587,
        21586,
        20449,
        20448,
        21660,
        20531,
        21661,
        20532,
    ];
    assert_eq!(actual, expected);
}
