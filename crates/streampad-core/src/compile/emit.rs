//! JavaScript emission for a validated [`Graph`].
//!
//! Structured form: chains are walked statement by statement; an `if` emits both branches up to its
//! immediate post-dominator (the join) and continues from there, so re-converging branches share the
//! join's code instead of duplicating it. When some node would still be emitted twice (two paths meet
//! somewhere that is not a join of a common `if`, e.g. a repeat body running into the code after the
//! loop), the whole graph is emitted in function-per-node form, which is correct for any acyclic graph.

use super::graph::{Graph, NodeIx};
use super::js::{Writer, string_lit};
use super::lower::{Condition, Op};
use super::{CompileOptions, CompiledMacro};

pub(super) fn emit(g: &Graph, opts: &CompileOptions) -> CompiledMacro {
    let release_entry = g.release.and_then(|r| g.next(r, "out"));
    let code = Structured::new(g, opts.trace)
        .emit(release_entry)
        .unwrap_or_else(|Irregular| function_per_node(g, opts.trace, release_entry));
    CompiledMacro { code, has_release: release_entry.is_some() }
}

/// The graph's shape needs the function-per-node form.
struct Irregular;

struct Structured<'g, 'a> {
    g: &'g Graph<'a>,
    trace: bool,
    ipdom: Vec<Option<NodeIx>>,
    emitted: Vec<bool>,
    loop_depth: usize,
    w: Writer,
}

impl<'g, 'a> Structured<'g, 'a> {
    fn new(g: &'g Graph<'a>, trace: bool) -> Self {
        Structured {
            g,
            trace,
            ipdom: g.post_dominators(),
            emitted: vec![false; g.nodes.len()],
            loop_depth: 0,
            w: Writer::default(),
        }
    }

    fn emit(mut self, release_entry: Option<NodeIx>) -> Result<String, Irregular> {
        let press = self.g.press;
        trace_line(&mut self.w, self.trace, self.g, press);
        self.chain(self.g.next(press, "out"), None)?;
        if let (Some(release), Some(entry)) = (self.g.release, release_entry) {
            self.w.blank();
            self.w.open("function onRelease(event) {");
            trace_line(&mut self.w, self.trace, self.g, release);
            self.chain(Some(entry), None)?;
            self.w.close("}");
        }
        Ok(self.w.finish())
    }

    /// Emits the nodes from `start` until `join` (exclusive) or the end of the region.
    fn chain(&mut self, start: Option<NodeIx>, join: Option<NodeIx>) -> Result<(), Irregular> {
        let mut cur = start;
        while let Some(n) = cur.filter(|n| Some(*n) != join) {
            self.claim(n)?;
            trace_line(&mut self.w, self.trace, self.g, n);
            cur = match self.g.op(n) {
                Op::Trigger => self.g.next(n, "out"),
                Op::Stmt(stmt) => {
                    self.w.line(stmt);
                    self.g.next(n, "out")
                }
                Op::Script(code) => {
                    if !code.trim().is_empty() {
                        self.w.code_block(code);
                    }
                    self.g.next(n, "out")
                }
                Op::Stop => {
                    self.w.line("return;");
                    None
                }
                Op::If(cond) => {
                    let join = self.ipdom[n];
                    self.if_else(n, cond, join)?;
                    join
                }
                Op::Repeat(count) => {
                    let var = loop_var(self.loop_depth);
                    self.w.open(&format!("for (let {var} = 0; {var} < {count}; {var}++) {{"));
                    self.loop_depth += 1;
                    self.chain(self.g.next(n, "body"), None)?;
                    self.loop_depth -= 1;
                    self.w.close("}");
                    self.g.next(n, "done")
                }
            };
        }
        Ok(())
    }

    /// `if (…) {…} else if (…) {…} else {…}`; every branch stops at `join`.
    fn if_else(&mut self, n: NodeIx, cond: &Condition, join: Option<NodeIx>) -> Result<(), Irregular> {
        let branch = |port: &str| self.g.next(n, port).filter(|t| Some(*t) != join);
        let (on_true, mut on_false) = (branch("true"), branch("false"));
        let Some(on_true) = on_true else {
            match on_false {
                Some(f) => {
                    self.w.open(&format!("if ({}) {{", cond.negated));
                    self.chain(Some(f), join)?;
                    self.w.close("}");
                }
                None => self.w.line(&format!("if ({}) {{}}", cond.expr)),
            }
            return Ok(());
        };
        self.w.open(&format!("if ({}) {{", cond.expr));
        self.chain(Some(on_true), join)?;
        // A false branch that is itself an `if` re-joining at the same place reads best as `else if`.
        while let Some(f) = on_false {
            let Op::If(next_cond) = self.g.op(f) else { break };
            let next_true = self.g.next(f, "true").filter(|t| Some(*t) != join);
            if self.trace || self.ipdom[f] != join || next_true.is_none() {
                break;
            }
            self.claim(f)?;
            self.w.reopen(&format!("}} else if ({}) {{", next_cond.expr));
            self.chain(next_true, join)?;
            on_false = self.g.next(f, "false").filter(|t| Some(*t) != join);
        }
        if let Some(f) = on_false {
            self.w.reopen("} else {");
            self.chain(Some(f), join)?;
        }
        self.w.close("}");
        Ok(())
    }

    fn claim(&mut self, n: NodeIx) -> Result<(), Irregular> {
        if std::mem::replace(&mut self.emitted[n], true) { Err(Irregular) } else { Ok(()) }
    }
}

/// Loop counters for nested repeats: `i`, `j`, `k`, then `i3`, `i4`, ….
fn loop_var(depth: usize) -> String {
    match depth {
        0 => "i".into(),
        1 => "j".into(),
        2 => "k".into(),
        d => format!("i{d}"),
    }
}

fn trace_line(w: &mut Writer, trace: bool, g: &Graph, n: NodeIx) {
    if trace {
        w.line(&format!("__node({});", string_lit(g.id(n))));
    }
}

/// Every node becomes `function stepN()` that runs the node and returns the next step's result; a stop
/// returns `true`, which repeat loops check so that stopping inside a loop ends the whole macro.
fn function_per_node(g: &Graph, trace: bool, release_entry: Option<NodeIx>) -> String {
    let steps: Vec<NodeIx> = g.execution_order().into_iter().filter(|n| !matches!(g.op(*n), Op::Trigger)).collect();
    let mut names = vec![String::new(); g.nodes.len()];
    for (i, n) in steps.iter().enumerate() {
        names[*n] = format!("step{}", i + 1);
    }
    let call = |n: NodeIx| format!("{}()", names[n]);

    let mut w = Writer::default();
    w.line("// Steps of this flow are shared by paths that plain if/else can't express, so each step is a");
    w.line("// function that runs one node and then the next step. A step returns true to stop the macro.");
    trace_line(&mut w, trace, g, g.press);
    if let Some(first) = g.next(g.press, "out") {
        w.line(&format!("{};", call(first)));
    }
    if let (Some(release), Some(entry)) = (g.release, release_entry) {
        w.blank();
        w.open("function onRelease(event) {");
        trace_line(&mut w, trace, g, release);
        w.line(&format!("{};", call(entry)));
        w.close("}");
    }
    for n in steps {
        let continue_with = |w: &mut Writer, port: &str| {
            if let Some(next) = g.next(n, port) {
                w.line(&format!("return {};", call(next)));
            }
        };
        w.blank();
        w.open(&format!("function {}() {{", names[n]));
        trace_line(&mut w, trace, g, n);
        match g.op(n) {
            Op::Trigger => {}
            Op::Stmt(stmt) => {
                w.line(stmt);
                continue_with(&mut w, "out");
            }
            Op::Script(code) => {
                if !code.trim().is_empty() {
                    w.code_block(code);
                }
                continue_with(&mut w, "out");
            }
            Op::Stop => w.line("return true;"),
            Op::If(cond) => match (g.next(n, "true"), g.next(n, "false")) {
                (Some(t), on_false) => {
                    w.open(&format!("if ({}) {{", cond.expr));
                    w.line(&format!("return {};", call(t)));
                    w.close("}");
                    if let Some(f) = on_false {
                        w.line(&format!("return {};", call(f)));
                    }
                }
                (None, Some(f)) => {
                    w.open(&format!("if ({}) {{", cond.negated));
                    w.line(&format!("return {};", call(f)));
                    w.close("}");
                }
                (None, None) => w.line(&format!("if ({}) {{}}", cond.expr)),
            },
            Op::Repeat(count) => {
                w.open(&format!("for (let i = 0; i < {count}; i++) {{"));
                if let Some(body) = g.next(n, "body") {
                    w.line(&format!("if ({}) return true;", call(body)));
                }
                w.close("}");
                continue_with(&mut w, "done");
            }
        }
        w.close("}");
    }
    w.finish()
}
