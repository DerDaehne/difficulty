//! Lost changes (spec R6): a change to a root (variable, parameter, `self`) that no
//! path reads before the root is assigned anew or the function ends. A backward
//! liveness analysis over the structured syntax tree; loops iterate to a fixpoint.

use std::collections::{BTreeSet, HashMap};

use crate::ast::*;
use crate::diag::Pos;

type Set = BTreeSet<String>;

/// Roots a call changes, by the span of the call: `mut` arguments whose parameter the
/// callee changes, and the receiver of a method that changes `self`.
pub type CallChanges = HashMap<(u32, u32), Vec<(String, Pos)>>;

/// The first lost change of each root, in source order. `escape` holds the roots
/// that count as read when the function ends (`mut` parameters, `self`).
pub fn lost(body: &Block, changes: &CallChanges, escape: &[String]) -> Vec<(String, Pos)> {
    let mut l = Live {
        changes,
        escape: escape.iter().cloned().collect(),
        report: true,
        lost: Vec::new(),
        exit: Vec::new(),
        head: Vec::new(),
    };
    let end = l.escape.clone();
    l.block(body, end);
    let mut first: Vec<(String, Pos)> = Vec::new();
    for (root, pos) in l.lost {
        match first.iter_mut().find(|(r, _)| *r == root) {
            Some(f) if pos.offset < f.1.offset => f.1 = pos,
            Some(_) => {}
            None => first.push((root, pos)),
        }
    }
    first.sort_by_key(|(_, p)| p.offset);
    first
}

struct Live<'c> {
    changes: &'c CallChanges,
    escape: Set,
    report: bool,
    lost: Vec<(String, Pos)>,
    /// Live sets after the innermost loops (for `break`) and at their heads (`continue`).
    exit: Vec<Set>,
    head: Vec<Set>,
}

fn root(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Ident(n) => Some(n.clone()),
        ExprKind::SelfRef => Some("self".into()),
        ExprKind::Field(b, _) | ExprKind::Index(b, _, _) => root(b),
        _ => None,
    }
}

impl Live<'_> {
    fn change(&mut self, root: &str, pos: Pos, live: &Set) {
        if self.report && !live.contains(root) {
            self.lost.push((root.to_string(), pos));
        }
    }

    fn block(&mut self, b: &Block, mut live: Set) -> Set {
        for s in b.stmts.iter().rev() {
            live = self.stmt(s, live);
        }
        live
    }

    fn stmt(&mut self, s: &Stmt, mut live: Set) -> Set {
        match s {
            Stmt::Var(v) => {
                live.remove(&v.name);
                self.expr(&v.value, live)
            }
            Stmt::Assign {
                place, op, value, ..
            } => {
                if let Some(r) = root(place) {
                    self.change(&r, place.pos, &live);
                    let whole = matches!(place.kind, ExprKind::Ident(_) | ExprKind::SelfRef);
                    if whole && *op == "=" {
                        live.remove(&r);
                    }
                    if *op != "=" {
                        live.insert(r);
                    }
                }
                // the place is evaluated first, then the value; index expressions in the
                // place are read too (`xs[i][j] = v`)
                live = self.expr(value, live);
                let mut p = place;
                loop {
                    match &p.kind {
                        ExprKind::Index(b, i, _) => {
                            live = self.expr(i, live);
                            p = b;
                        }
                        ExprKind::Field(b, _) => p = b,
                        _ => break,
                    }
                }
                live
            }
            Stmt::If {
                branches,
                otherwise,
                ..
            } => {
                let mut acc = match otherwise {
                    Some(o) => self.block(o, live.clone()),
                    None => live.clone(),
                };
                for (c, b) in branches.iter().rev() {
                    let inner = self.block(b, live.clone());
                    acc.extend(inner);
                    acc = self.expr(c, acc);
                }
                acc
            }
            Stmt::While { cond, body, .. } => {
                let exit = live;
                self.fixpoint(&exit, |l, head| {
                    let body_in = l.block(body, head.clone());
                    let mut h = exit.clone();
                    h.extend(body_in);
                    l.expr(cond, h)
                })
            }
            Stmt::For {
                vars, iter, body, ..
            } => {
                let exit = live;
                let head = self.fixpoint(&exit, |l, head| {
                    let mut body_in = l.block(body, head.clone());
                    for v in vars {
                        body_in.remove(&v.name);
                    }
                    let mut h = exit.clone();
                    h.extend(body_in);
                    h
                });
                self.expr(iter, head)
            }
            Stmt::Switch {
                subject,
                cases,
                default,
                ..
            } => {
                let mut acc = match default {
                    Some((b, _)) => self.block(b, live.clone()),
                    None => live.clone(),
                };
                for c in cases {
                    let mut inner = self.block(&c.body, live.clone());
                    for p in &c.pats {
                        match p {
                            Pattern::Variant {
                                binds: Some(bs), ..
                            } => {
                                for b in bs.iter().flatten() {
                                    inner.remove(&b.name);
                                }
                            }
                            Pattern::Result {
                                bind: Some(Some(b)),
                                ..
                            } => {
                                inner.remove(&b.name);
                            }
                            _ => {}
                        }
                    }
                    acc.extend(inner);
                }
                self.expr(subject, acc)
            }
            Stmt::Break(_) => self.exit.last().cloned().unwrap_or_default(),
            Stmt::Continue(_) => self.head.last().cloned().unwrap_or_default(),
            Stmt::Return(v, _) => {
                let end = self.escape.clone();
                match v {
                    Some(e) => self.expr(e, end),
                    None => end,
                }
            }
            Stmt::Call(e) | Stmt::Discard(e, _) => self.expr(e, live),
        }
    }

    /// Live set at the head of a loop: iterate silently until stable, then once more
    /// with reporting, so a change is judged with the final sets.
    fn fixpoint(&mut self, exit: &Set, step: impl Fn(&mut Self, &Set) -> Set) -> Set {
        let report = self.report;
        self.report = false;
        let mut head = exit.clone();
        loop {
            self.exit.push(exit.clone());
            self.head.push(head.clone());
            let next = step(self, &head);
            self.exit.pop();
            self.head.pop();
            if next == head {
                break;
            }
            head = next;
        }
        self.report = report;
        self.exit.push(exit.clone());
        self.head.push(head.clone());
        let head = step(self, &head);
        self.exit.pop();
        self.head.pop();
        head
    }

    fn expr(&mut self, e: &Expr, mut live: Set) -> Set {
        match &e.kind {
            ExprKind::Ident(n) => {
                live.insert(n.clone());
                live
            }
            ExprKind::SelfRef => {
                live.insert("self".into());
                live
            }
            ExprKind::Call(f, args) => {
                if let Some(chs) = self.changes.get(&(e.pos.offset, e.end)) {
                    for (r, p) in chs.clone() {
                        self.change(&r, p, &live);
                    }
                }
                for a in args.iter().rev() {
                    live = self.expr(a, live);
                }
                self.expr(f, live)
            }
            ExprKind::IfExpr(branches, last) => {
                let mut acc = self.value_block(last, live.clone());
                for (c, vb) in branches.iter().rev() {
                    let inner = self.value_block(vb, live.clone());
                    acc.extend(inner);
                    acc = self.expr(c, acc);
                }
                acc
            }
            ExprKind::Str(segs) => {
                for s in segs.iter().rev() {
                    if let StrSeg::Expr(x) | StrSeg::Fmt(x, _) = s {
                        live = self.expr(x, live);
                    }
                }
                live
            }
            ExprKind::StructLit(_, inits) => {
                for i in inits.iter().rev() {
                    live = self.expr(&i.value, live);
                }
                live
            }
            ExprKind::ListLit(items) => {
                for x in items.iter().rev() {
                    live = self.expr(x, live);
                }
                live
            }
            ExprKind::MapLit(entries) => {
                for (k, v) in entries.iter().rev() {
                    live = self.expr(v, live);
                    live = self.expr(k, live);
                }
                live
            }
            ExprKind::Unary(_, x)
            | ExprKind::Paren(x)
            | ExprKind::Mut(x)
            | ExprKind::SomeVal(x)
            | ExprKind::ErrVal(x)
            | ExprKind::OkVal(Some(x)) => self.expr(x, live),
            // `try` may leave the function right after `x`: the escaping roots are read
            // there (like `return`); the `or` value is computed only on that path.
            ExprKind::Try(x, or) => {
                let mut after = live;
                after.extend(self.escape.iter().cloned());
                if let Some((_, alt)) = or {
                    let end = self.escape.clone();
                    after.extend(self.expr(alt, end));
                }
                self.expr(x, after)
            }
            ExprKind::Field(b, _) => self.expr(b, live),
            ExprKind::Binary(_, l, r, _) | ExprKind::Index(l, r, _) => {
                let live = self.expr(r, live);
                self.expr(l, live)
            }
            _ => live,
        }
    }

    fn value_block(&mut self, vb: &ValueBlock, live: Set) -> Set {
        let mut live = self.expr(&vb.value, live);
        for s in vb.stmts.iter().rev() {
            live = self.stmt(s, live);
        }
        live
    }
}
