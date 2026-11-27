//! Lost changes (spec R6): a change to a place (`p`, `p.x`, `p.inner.y`: a root and
//! fields; an index stays with the indexed container) that no path reads before the
//! place is assigned anew or the function ends. A read of the place, a part of it or a
//! whole containing it counts. A backward liveness analysis over the structured syntax
//! tree; loops iterate to a fixpoint.

use std::collections::{BTreeSet, HashMap};

use crate::ast::*;
use crate::diag::Pos;

type Set = BTreeSet<String>;

/// Roots a call changes, by the span of the call: `mut` arguments whose parameter the
/// callee changes, and the receiver of a method that changes `self`.
pub type CallChanges = HashMap<(u32, u32), Vec<(String, Pos)>>;

/// The first lost change of each place (a dotted path), in source order. `escape` holds the roots
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

/// The path of a place and whether an index cut it (`p.xs[i].f` is `p.xs`).
fn path(e: &Expr) -> Option<(String, bool)> {
    match &e.kind {
        ExprKind::Ident(n) => Some((n.clone(), false)),
        ExprKind::SelfRef => Some(("self".into(), false)),
        ExprKind::Field(b, f) => path(b).map(|(s, cut)| {
            if cut {
                (s, true)
            } else {
                (format!("{s}.{f}"), false)
            }
        }),
        ExprKind::Index(b, _, _) => path(b).map(|(s, _)| (s, true)),
        _ => None,
    }
}

/// Drop `p` and everything below it (`p.x`) from the set.
fn kill(live: &mut Set, p: &str) {
    let below = format!("{p}.");
    live.retain(|l| l != p && !l.starts_with(&below));
}

/// Is `c` read by a live path: the same, a part of it, or a whole containing it?
fn read(live: &Set, c: &str) -> bool {
    live.iter().any(|l| {
        l == c
            || l.strip_prefix(c).is_some_and(|r| r.starts_with('.'))
            || c.strip_prefix(l.as_str())
                .is_some_and(|r| r.starts_with('.'))
    })
}

impl Live<'_> {
    fn change(&mut self, place: &str, pos: Pos, live: &Set) {
        if self.report && !read(live, place) {
            self.lost.push((place.to_string(), pos));
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
                kill(&mut live, &v.name);
                self.expr(&v.value, live)
            }
            Stmt::Assign {
                place, op, value, ..
            } => {
                if let Some((r, cut)) = path(place) {
                    self.change(&r, place.pos, &live);
                    if !cut && *op == "=" {
                        kill(&mut live, &r);
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
                        kill(&mut body_in, &v.name);
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
                                    kill(&mut inner, &b.name);
                                }
                            }
                            Pattern::Result {
                                bind: Some(Some(b)),
                                ..
                            } => {
                                kill(&mut inner, &b.name);
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
            ExprKind::Ident(_) | ExprKind::SelfRef | ExprKind::Field(..) | ExprKind::Index(..) => {
                self.place(e, "", live)
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
                // a method call reads its whole receiver
                match &f.kind {
                    ExprKind::Field(b, _) => self.expr(b, live),
                    _ => self.expr(f, live),
                }
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
            ExprKind::Binary(_, l, r, _) => {
                let live = self.expr(r, live);
                self.expr(l, live)
            }
            _ => live,
        }
    }

    /// Read a field chain: `p.a.b` reads the path `p.a.b`; an index reads the container
    /// (the suffix after it is dropped) and the index expression.
    fn place(&mut self, e: &Expr, suffix: &str, mut live: Set) -> Set {
        match &e.kind {
            ExprKind::Ident(n) => {
                live.insert(format!("{n}{suffix}"));
                live
            }
            ExprKind::SelfRef => {
                live.insert(format!("self{suffix}"));
                live
            }
            ExprKind::Field(b, f) => self.place(b, &format!(".{f}{suffix}"), live),
            ExprKind::Index(b, i, _) => {
                live = self.expr(i, live);
                self.place(b, "", live)
            }
            _ => self.expr(e, live),
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
