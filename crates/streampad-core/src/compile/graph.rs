//! Validated, indexed view of a graph macro (graph rules of docs/SPEC.md §3.3).

use super::CompileError;
use super::lower::{Op, PRESS_TRIGGER, RELEASE_TRIGGER, lower};
use crate::config::{GraphEdge, Node};
use std::collections::HashMap;

pub(super) type NodeIx = usize;

pub(super) struct Graph<'a> {
    pub(super) nodes: &'a [Node],
    /// Lowered op per node; `None` for nodes not reachable from a trigger (they are ignored).
    ops: Vec<Option<Op>>,
    out: Vec<Vec<(&'a str, NodeIx)>>,
    pub(super) press: NodeIx,
    pub(super) release: Option<NodeIx>,
    /// Reachable nodes, every node after all of its successors.
    post_order: Vec<NodeIx>,
}

impl<'a> Graph<'a> {
    pub(super) fn build(nodes: &'a [Node], edges: &'a [GraphEdge]) -> Result<Graph<'a>, CompileError> {
        let mut index: HashMap<&str, NodeIx> = HashMap::new();
        for (i, n) in nodes.iter().enumerate() {
            if index.insert(n.id.as_str(), i).is_some() {
                return Err(CompileError::node(&n.id, format!("Two nodes share the id \"{}\"", n.id)));
            }
        }
        let press = single_trigger(nodes, PRESS_TRIGGER, "Only one Press trigger is allowed")?
            .ok_or_else(|| CompileError::graph("Add a Press trigger: it is where the macro starts"))?;
        let release = single_trigger(nodes, RELEASE_TRIGGER, "Only one Release trigger is allowed")?;

        // Edges whose ends do not exist are leftovers of deleted nodes; they connect nothing.
        let mut out: Vec<Vec<(&str, NodeIx)>> = vec![Vec::new(); nodes.len()];
        for e in edges {
            if let (Some(&s), Some(&t)) = (index.get(e.source.as_str()), index.get(e.target.as_str())) {
                out[s].push((e.source_port.as_str(), t));
            }
        }

        let mut graph = Graph { nodes, ops: vec![None; nodes.len()], out, press, release, post_order: Vec::new() };
        let reachable = graph.reachable();
        for (i, node) in nodes.iter().enumerate().filter(|(i, _)| reachable[*i]) {
            let op = lower(node).map_err(|m| CompileError::node(&node.id, m))?;
            graph.check_ports(i, &op)?;
            graph.ops[i] = Some(op);
        }
        graph.post_order = graph.acyclic_post_order()?;
        Ok(graph)
    }

    pub(super) fn op(&self, n: NodeIx) -> &Op {
        self.ops[n].as_ref().expect("only reachable nodes are visited")
    }

    pub(super) fn id(&self, n: NodeIx) -> &str {
        &self.nodes[n].id
    }

    /// The node connected to `port` of `n`, if any.
    pub(super) fn next(&self, n: NodeIx, port: &str) -> Option<NodeIx> {
        self.out[n].iter().find(|(p, _)| *p == port).map(|(_, t)| *t)
    }

    /// Nodes that can run, depth-first from the press trigger then the release trigger, following ports
    /// in their declared order (`true` before `false`, `body` before `done`).
    pub(super) fn execution_order(&self) -> Vec<NodeIx> {
        let mut seen = vec![false; self.nodes.len()];
        let mut order = Vec::new();
        for root in self.roots() {
            let mut stack = vec![root];
            while let Some(n) = stack.pop() {
                if std::mem::replace(&mut seen[n], true) {
                    continue;
                }
                order.push(n);
                stack.extend(self.op(n).ports().iter().rev().filter_map(|p| self.next(n, p)));
            }
        }
        order
    }

    fn roots(&self) -> impl Iterator<Item = NodeIx> + use<> {
        [Some(self.press), self.release].into_iter().flatten()
    }

    /// Immediate post-dominator of every reachable node: the first node that every path from it to the
    /// end of its region must pass through (`None` = the end itself). Successors here are the ports that
    /// continue the current region: a repeat's `body` is a region of its own.
    pub(super) fn post_dominators(&self) -> Vec<Option<NodeIx>> {
        let mut ipdom: Vec<Option<NodeIx>> = vec![None; self.nodes.len()];
        for &n in &self.post_order {
            ipdom[n] = match self.op(n) {
                Op::Stop => None,
                Op::If(_) => {
                    let chain = |start: Option<NodeIx>| std::iter::successors(start, |x| ipdom[*x]).collect::<Vec<_>>();
                    let on_false = chain(self.next(n, "false"));
                    chain(self.next(n, "true")).into_iter().find(|x| on_false.contains(x))
                }
                Op::Repeat(_) => self.next(n, "done"),
                Op::Trigger | Op::Stmt(_) | Op::Script(_) => self.next(n, "out"),
            };
        }
        ipdom
    }

    fn reachable(&self) -> Vec<bool> {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack: Vec<NodeIx> = self.roots().collect();
        while let Some(n) = stack.pop() {
            if !std::mem::replace(&mut seen[n], true) {
                stack.extend(self.out[n].iter().map(|(_, t)| *t));
            }
        }
        seen
    }

    fn check_ports(&self, n: NodeIx, op: &Op) -> Result<(), CompileError> {
        let id = self.id(n);
        for (i, (port, target)) in self.out[n].iter().enumerate() {
            if !op.ports().contains(port) {
                return Err(CompileError::node(id, format!("This node has no \"{port}\" output")));
            }
            if self.out[n][..i].iter().any(|(p, _)| p == port) {
                return Err(CompileError::node(id, format!("The \"{port}\" output can only have one connection")));
            }
            if matches!(self.nodes[*target].kind.as_str(), PRESS_TRIGGER | RELEASE_TRIGGER) {
                return Err(CompileError::node(id, "Triggers can't be connected to: they start the macro"));
            }
        }
        Ok(())
    }

    /// Iterative DFS over the reachable graph. A connection back to a node still on the DFS path is a loop.
    fn acyclic_post_order(&self) -> Result<Vec<NodeIx>, CompileError> {
        #[derive(Clone, Copy, PartialEq)]
        enum Mark {
            New,
            OnPath,
            Done,
        }
        let mut mark = vec![Mark::New; self.nodes.len()];
        let mut order = Vec::new();
        for root in self.roots() {
            let mut stack: Vec<(NodeIx, usize)> = vec![(root, 0)];
            mark[root] = Mark::OnPath;
            while let Some((n, edge)) = stack.last_mut() {
                let n = *n;
                if let Some(&(_, t)) = self.out[n].get(*edge) {
                    *edge += 1;
                    match mark[t] {
                        Mark::OnPath => {
                            return Err(CompileError::node(
                                self.id(n),
                                "This connection makes a loop. Use a Repeat node to run steps more than once",
                            ));
                        }
                        Mark::New => {
                            mark[t] = Mark::OnPath;
                            stack.push((t, 0));
                        }
                        Mark::Done => {}
                    }
                } else {
                    mark[n] = Mark::Done;
                    order.push(n);
                    stack.pop();
                }
            }
        }
        Ok(order)
    }
}

fn single_trigger(nodes: &[Node], kind: &str, duplicate: &str) -> Result<Option<NodeIx>, CompileError> {
    let mut found = nodes.iter().enumerate().filter(|(_, n)| n.kind == kind);
    let first = found.next().map(|(i, _)| i);
    match found.next() {
        Some((_, extra)) => Err(CompileError::node(&extra.id, duplicate)),
        None => Ok(first),
    }
}
