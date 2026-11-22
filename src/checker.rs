//! Names and types of the first language subset (spec R1, R2, R4, R8, R9, R11,
//! R13, R14, R28, R29). Runs only on a module without lexical or syntax errors,
//! so every diagnostic here has its own cause.

use std::collections::HashMap;

use crate::ast::*;
use crate::diag::{Diag, Pos};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty {
    Int,
    Bool,
    Str,
    Void,
    /// Already reported; silences everything that depends on it.
    Err,
}

impl Ty {
    fn name(self) -> &'static str {
        match self {
            Ty::Int => "int",
            Ty::Bool => "bool",
            Ty::Str => "str",
            Ty::Void => "void",
            Ty::Err => "?",
        }
    }
}

/// What is known about a value at compile time (R8: literals and `const`).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Val {
    /// An `int` computed exactly, not yet checked against the range of `int`.
    Int(i128),
    /// Some other value that is fixed at compile time.
    Known,
    Runtime,
}

const LATER_TYPES: &[&str] = &[
    "i8", "i16", "i32", "u8", "u16", "u32", "u64", "f32", "f64", "rune",
];
const BUILTINS: &[&str] = &["print", "range", "repeat", "expect", "assert"];

/// Longest chain of global consts that use each other.
const MAX_CONST_DEPTH: u32 = 256;

pub fn check(file: &str, src: &str, m: &Module) -> Vec<Diag> {
    let mut c = Checker {
        file,
        src,
        m,
        funcs: HashMap::new(),
        consts: HashMap::new(),
        scopes: Vec::new(),
        func: None,
        loops: 0,
        unknown: Vec::new(),
        const_depth: 0,
        const_too_deep: false,
        diags: Vec::new(),
    };
    c.module();
    c.diags
}

/// `void main()` for `dif run`/`dif build` (R1); `dif check` of a library does not need it.
pub fn main_diag(m: &Module) -> Option<Diag> {
    let main = m.decls.iter().find_map(|d| match d {
        Decl::Func(f) if f.name == "main" => Some(f),
        _ => None,
    });
    let hint = "a program starts in void main() { … }";
    match main {
        None => Some(
            Diag::new(
                "R1",
                Pos {
                    line: 1,
                    col: 1,
                    offset: 0,
                },
                "no `main` function",
            )
            .hint(hint),
        ),
        Some(f) if f.ret.name != "void" || !f.params.is_empty() => {
            Some(Diag::new("R1", f.pos, "`main` must be `void main()`").hint(hint))
        }
        Some(_) => None,
    }
}

struct Sig {
    params: Vec<Ty>,
    ret: Ty,
    pos: Pos,
}

enum ConstState<'a> {
    Todo(&'a VarDecl),
    Busy,
    Done(Ty, Val, Pos),
}

struct Local {
    name: String,
    ty: Ty,
    is_const: bool,
    val: Val,
    pos: Pos,
}

struct Checker<'a> {
    file: &'a str,
    src: &'a str,
    m: &'a Module,
    funcs: HashMap<&'a str, Sig>,
    consts: HashMap<&'a str, ConstState<'a>>,
    scopes: Vec<Vec<Local>>,
    /// Name and return type of the function being checked.
    func: Option<(&'a str, Ty)>,
    loops: u32,
    /// Unknown names already reported: each once.
    unknown: Vec<String>,
    const_depth: u32,
    const_too_deep: bool,
    diags: Vec<Diag>,
}

impl<'a> Checker<'a> {
    fn err(&mut self, code: &'static str, pos: Pos, msg: impl Into<String>, hint: Option<String>) {
        let mut d = Diag::new(code, pos, msg);
        d.hint = hint;
        self.diags.push(d);
    }

    fn text(&self, e: &Expr) -> &'a str {
        self.src
            .get(e.pos.offset as usize..e.end as usize)
            .unwrap_or("")
    }

    fn at(&self, p: Pos) -> String {
        format!("{}:{}:{}", self.file, p.line, p.col)
    }

    // ---- module ----

    fn module(&mut self) {
        let m = self.m;
        // first the names of the module, so the order of declarations does not matter
        let mut first = Vec::new();
        for d in &m.decls {
            let (name, pos) = match d {
                Decl::Func(f) => (f.name.as_str(), f.pos),
                Decl::Const(v) => (v.name.as_str(), v.pos),
            };
            let dup = self.predefined(name, pos) || {
                let prev = self.module_pos(name);
                if let Some(prev) = prev {
                    self.already(name, pos, prev);
                }
                prev.is_some()
            };
            first.push(!dup);
            if dup {
                continue;
            }
            match d {
                Decl::Func(f) => {
                    let params = f.params.iter().map(|p| self.ty(&p.ty, false)).collect();
                    let ret = self.ty(&f.ret, true);
                    self.funcs.insert(name, Sig { params, ret, pos });
                }
                Decl::Const(v) => {
                    self.consts.insert(name, ConstState::Todo(v));
                }
            }
        }
        for (d, first) in m.decls.iter().zip(first) {
            match d {
                Decl::Const(v) if first => {
                    let _ = self.global_const(&v.name);
                }
                Decl::Func(f) if first => self.func(f),
                _ => {}
            }
        }
    }

    fn cycle(&mut self, name: &str, pos: Pos) {
        let msg = format!("`{name}` depends on itself");
        self.err(
            "R4",
            pos,
            msg,
            Some("give it a value without itself".into()),
        );
    }

    fn module_pos(&self, name: &str) -> Option<Pos> {
        if let Some(s) = self.funcs.get(name) {
            return Some(s.pos);
        }
        match self.consts.get(name) {
            Some(ConstState::Todo(v)) => Some(v.pos),
            Some(ConstState::Done(_, _, p)) => Some(*p),
            _ => None,
        }
    }

    fn predefined(&mut self, name: &str, pos: Pos) -> bool {
        let predefined = BUILTINS.contains(&name)
            || LATER_TYPES.contains(&name)
            || matches!(name, "int" | "bool" | "str");
        if predefined {
            let hint = "choose another name".to_string();
            self.err(
                "R4",
                pos,
                format!("`{name}` is a predefined name"),
                Some(hint),
            );
        }
        predefined
    }

    fn already(&mut self, name: &str, pos: Pos, prev: Pos) {
        let msg = format!("`{name}` is already declared at {}", self.at(prev));
        let hint = "names are not redeclared inside their block; choose another name".to_string();
        self.err("R4", pos, msg, Some(hint));
    }

    fn ty(&mut self, t: &TypeRef, ret: bool) -> Ty {
        match t.name.as_str() {
            "int" => Ty::Int,
            "bool" => Ty::Bool,
            "str" => Ty::Str,
            "void" if ret => Ty::Void,
            "void" => {
                let msg = "`void` is only a return type";
                self.err("R8", t.pos, msg, None);
                Ty::Err
            }
            "?" => Ty::Err,
            n if LATER_TYPES.contains(&n) => {
                let msg = format!("`{n}` is not available yet; use int");
                self.err("R8", t.pos, msg, None);
                Ty::Err
            }
            n => {
                let hint = closest(n, ["int", "bool", "str"].into_iter())
                    .map(|c| format!("did you mean {c}?"));
                self.err("R4", t.pos, format!("unknown type `{n}`"), hint);
                Ty::Err
            }
        }
    }

    /// A global `const`, checked on first use so the order of declarations does not
    /// matter; `None` while it is being checked (it depends on itself).
    fn global_const(&mut self, name: &str) -> Option<(Ty, Val)> {
        let (key, state) = self.consts.remove_entry(name)?;
        match state {
            ConstState::Done(t, v, p) => {
                self.consts.insert(key, ConstState::Done(t, v, p));
                Some((t, v))
            }
            ConstState::Busy => {
                self.consts.insert(key, ConstState::Busy);
                None
            }
            ConstState::Todo(d) if self.const_depth >= MAX_CONST_DEPTH => {
                // each const checks the ones it uses first: a long chain is deep recursion
                self.consts.insert(key, ConstState::Todo(d));
                if !self.const_too_deep {
                    self.const_too_deep = true;
                    let msg =
                        format!("`{key}` depends on more than {MAX_CONST_DEPTH} consts in a row");
                    self.err("R4", d.pos, msg, Some("compute it in fewer steps".into()));
                }
                Some((Ty::Err, Val::Known))
            }
            ConstState::Todo(d) => {
                self.consts.insert(key, ConstState::Busy);
                let scopes = std::mem::take(&mut self.scopes);
                let ty = self.ty(&d.ty, false);
                self.const_depth += 1;
                let (t, v) = self.rvalue(&d.value);
                self.const_depth -= 1;
                self.scopes = scopes;
                let mut v = v;
                if t == Ty::Err {
                    v = Val::Known; // reported; its users need no second message
                } else if v == Val::Runtime {
                    let msg = "a global const needs a value known at compile time";
                    self.err("R4", d.value.pos, msg, None);
                    v = Val::Known;
                }
                self.expect(ty, t, &d.value);
                self.consts.insert(key, ConstState::Done(ty, v, d.pos));
                Some((ty, v))
            }
        }
    }

    fn func(&mut self, f: &'a Func) {
        let Some(sig) = self.funcs.get(f.name.as_str()) else {
            return;
        };
        let params = sig.params.clone();
        let ret = sig.ret;
        self.func = Some((f.name.as_str(), ret));
        self.scopes.push(Vec::new());
        for (p, t) in f.params.iter().zip(params) {
            self.declare(&p.name, p.pos, t, false, Val::Runtime);
        }
        self.block(&f.body);
        self.scopes.pop();
        if ret != Ty::Void && ret != Ty::Err && !terminates(&f.body) {
            let msg = format!("`{}` does not return a value on every path", f.name);
            let hint = format!("end it with return, e.g. return {}", zero(ret));
            self.err("R8", f.pos, msg, Some(hint));
        }
        self.func = None;
    }

    // ---- names ----

    fn declare(&mut self, name: &str, pos: Pos, ty: Ty, is_const: bool, val: Val) {
        if self.predefined(name, pos) {
            return;
        }
        let prev = self
            .scopes
            .iter()
            .flatten()
            .find(|l| l.name == name)
            .map(|l| l.pos)
            .or_else(|| self.module_pos(name));
        if let Some(prev) = prev {
            self.already(name, pos, prev);
            return;
        }
        let local = Local {
            name: name.to_string(),
            ty,
            is_const,
            val: if is_const { val } else { Val::Runtime },
            pos,
        };
        if let Some(s) = self.scopes.last_mut() {
            s.push(local);
        }
    }

    fn local(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().flatten().find(|l| l.name == name)
    }

    /// All names visible here, for "did you mean".
    fn visible(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .scopes
            .iter()
            .flatten()
            .map(|l| l.name.as_str())
            .collect();
        v.extend(self.funcs.keys().copied());
        v.extend(self.consts.keys().copied());
        v.push("print");
        v
    }

    fn unknown_name(&mut self, name: &str, pos: Pos) {
        if self.unknown.iter().any(|n| n == name) {
            return;
        }
        self.unknown.push(name.to_string());
        let hint = closest(name, self.visible().into_iter()).map(|c| format!("did you mean {c}?"));
        self.err("R4", pos, format!("unknown name `{name}`"), hint);
    }

    fn lookup(&mut self, name: &str, pos: Pos) -> (Ty, Val) {
        if let Some(l) = self.local(name) {
            return (l.ty, l.val);
        }
        if self.consts.contains_key(name) {
            return self.global_const(name).unwrap_or_else(|| {
                self.cycle(name, pos);
                (Ty::Err, Val::Runtime)
            });
        }
        if self.funcs.contains_key(name) || BUILTINS.contains(&name) {
            let msg = format!("`{name}` is a function; call it: {name}(…)");
            self.err("R8", pos, msg, None);
            return (Ty::Err, Val::Runtime);
        }
        self.unknown_name(name, pos);
        (Ty::Err, Val::Runtime)
    }

    // ---- statements ----

    fn block(&mut self, b: &'a Block) {
        self.scopes.push(Vec::new());
        for s in &b.stmts {
            self.stmt(s);
        }
        self.scopes.pop();
    }

    fn stmt(&mut self, s: &'a Stmt) {
        match s {
            Stmt::Var(v) => {
                let ty = self.ty(&v.ty, false);
                let (t, val) = self.rvalue(&v.value);
                self.expect(ty, t, &v.value);
                self.declare(&v.name, v.pos, ty, v.is_const, val);
            }
            Stmt::Assign {
                place,
                op,
                value,
                pos,
            } => self.assign(place, op, value, *pos),
            Stmt::If {
                branches,
                otherwise,
                ..
            } => {
                for (k, (c, b)) in branches.iter().enumerate() {
                    self.cond(c, if k == 0 { "if" } else { "else if" });
                    self.block(b);
                }
                if let Some(b) = otherwise {
                    self.block(b);
                }
            }
            Stmt::While { cond, body, .. } => {
                self.cond(cond, "while");
                self.loops += 1;
                self.block(body);
                self.loops -= 1;
            }
            Stmt::For {
                vars, iter, body, ..
            } => self.for_stmt(vars, iter, body),
            Stmt::Break(p) | Stmt::Continue(p) => {
                if self.loops == 0 {
                    let word = if matches!(s, Stmt::Break(_)) {
                        "break"
                    } else {
                        "continue"
                    };
                    self.err("R11", *p, format!("`{word}` outside a loop"), None);
                }
            }
            Stmt::Return(value, p) => self.ret(value.as_ref(), *p),
            Stmt::Call(e) => {
                let (t, _) = self.expr(e);
                if t != Ty::Void && t != Ty::Err {
                    let text = self.text(e);
                    let msg = format!("result of `{text}` is never used");
                    let hint = format!("use it, or discard it explicitly with `_ = {text}`");
                    self.err("R13", e.pos, msg, Some(hint));
                }
            }
            Stmt::Discard(e, _) => {
                let (t, _) = self.rvalue(e);
                if t == Ty::Void {
                    let msg = format!("`{}` has no result to discard", self.text(e));
                    self.err("R8", e.pos, msg, Some("call it without `_ =`".into()));
                }
            }
        }
    }

    fn assign(&mut self, place: &'a Expr, op: &str, value: &'a Expr, pos: Pos) {
        let ExprKind::Ident(name) = &place.kind else {
            self.expr(place); // fields and indexes come in a later part
            return;
        };
        let target = self.local(name).map(|l| (l.ty, l.is_const, l.pos));
        let (want, is_const, decl) = match target {
            Some(t) => t,
            None if self.consts.contains_key(name.as_str()) => {
                let p = self.module_pos(name).unwrap_or(pos);
                (Ty::Err, true, p)
            }
            None if self.funcs.contains_key(name.as_str()) => {
                self.err(
                    "R8",
                    place.pos,
                    format!("cannot assign to function `{name}`"),
                    None,
                );
                return;
            }
            None => {
                let (t, _) = self.rvalue(value);
                let ty = if t == Ty::Err { "T" } else { t.name() };
                let hint = format!("declare it with a type: {ty} {name} = {}", self.text(value));
                self.err(
                    "R2",
                    place.pos,
                    format!("`{name}` is not declared"),
                    Some(hint),
                );
                return;
            }
        };
        if is_const {
            let msg = format!("cannot assign to const `{name}`");
            let hint = format!("it is declared const at {}", self.at(decl));
            self.err("R4", place.pos, msg, Some(hint));
            return;
        }
        let (t, _) = self.rvalue(value);
        if op == "=" {
            self.expect(want, t, value);
            return;
        }
        let ok = match op {
            "+=" => (want == Ty::Int && t == Ty::Int) || (want == Ty::Str && t == Ty::Str),
            _ => want == Ty::Int && t == Ty::Int,
        };
        if !ok && want != Ty::Err && t != Ty::Err {
            let msg = format!("cannot apply `{op}` to {} and {}", want.name(), t.name());
            self.err("R8", pos, msg, None);
        }
    }

    fn for_stmt(&mut self, vars: &'a [Param], iter: &'a Expr, body: &'a Block) {
        let range_args = match &iter.kind {
            ExprKind::Call(f, args) if matches!(&f.kind, ExprKind::Ident(n) if n == "range") => {
                Some(args)
            }
            _ => None,
        };
        match range_args {
            Some(args) => {
                if args.is_empty() || args.len() > 2 {
                    let msg = format!("`range` takes 1 or 2 arguments, found {}", args.len());
                    self.err("R8", iter.pos, msg, None);
                }
                for a in args {
                    let (t, _) = self.rvalue(a);
                    self.expect(Ty::Int, t, a);
                }
                if vars.len() == 2 {
                    let msg = "`range` gives one value per step; use one loop variable";
                    self.err("R8", vars[1].pos, msg, None);
                }
            }
            None => {
                let (t, _) = self.rvalue(iter);
                if t != Ty::Err {
                    let msg = format!("cannot loop over {}", t.name());
                    let hint = "loop over a range: for int i in range(n)".to_string();
                    self.err("R8", iter.pos, msg, Some(hint));
                }
            }
        }
        self.scopes.push(Vec::new());
        for v in vars {
            let t = self.ty(&v.ty, false);
            if range_args.is_some() && t != Ty::Int && t != Ty::Err {
                let msg = format!("the values of `range` are int, not {}", t.name());
                self.err("R8", v.ty.pos, msg, None);
            }
            self.declare(&v.name, v.pos, t, false, Val::Runtime);
        }
        self.loops += 1;
        self.block(body);
        self.loops -= 1;
        self.scopes.pop();
    }

    fn ret(&mut self, value: Option<&'a Expr>, p: Pos) {
        let Some((name, want)) = self.func else {
            return;
        };
        match value {
            None if want != Ty::Void && want != Ty::Err => {
                let msg = format!("`{name}` returns {}; give `return` a value", want.name());
                self.err("R8", p, msg, Some(format!("return {}", zero(want))));
            }
            Some(e) => {
                let (t, _) = self.rvalue(e);
                if want == Ty::Void {
                    let msg = format!("`{name}` returns nothing; remove the value");
                    self.err("R8", e.pos, msg, None);
                } else {
                    self.expect(want, t, e);
                }
            }
            None => {}
        }
    }

    // ---- expressions ----

    /// The type the context wants versus the type found (R8).
    fn expect(&mut self, want: Ty, got: Ty, e: &Expr) {
        if want != got && want != Ty::Err && got != Ty::Err {
            let msg = format!("expected {}, found {}", want.name(), got.name());
            self.err("R8", e.pos, msg, None);
        }
    }

    /// A condition (R9); returns what is known about its value.
    fn cond(&mut self, e: &'a Expr, ctx: &str) -> Val {
        let (t, v) = self.rvalue(e);
        if t == Ty::Bool || t == Ty::Err {
            return v;
        }
        let text = self.text(e);
        let cmp = match t {
            Ty::Int => Some(format!("{text} != 0")),
            Ty::Str => Some(format!("{text} != \"\"")),
            _ => None,
        };
        let (msg, hint) = match ctx {
            "if" | "else if" | "while" => (
                format!("condition is {}, not bool", t.name()),
                cmp.map(|c| format!("compare explicitly: {ctx} {c}")),
            ),
            op => (
                format!("operand of `{op}` is {}, not bool", t.name()),
                cmp.map(|c| format!("compare explicitly: {c}")),
            ),
        };
        self.err("R9", e.pos, msg, hint);
        Val::Runtime
    }

    /// A value in a place where it is used: a constant `int` must fit (R8).
    fn rvalue(&mut self, e: &'a Expr) -> (Ty, Val) {
        let (t, v) = self.expr(e);
        if let Val::Int(n) = v {
            if !fits(n) {
                self.out_of_range(e.pos);
                return (Ty::Err, Val::Runtime);
            }
        }
        (t, v)
    }

    fn out_of_range(&mut self, pos: Pos) {
        let hint = "the value does not fit in int".to_string();
        self.err("R8", pos, "integer constant out of range", Some(hint));
    }

    fn expr(&mut self, e: &'a Expr) -> (Ty, Val) {
        match &e.kind {
            ExprKind::Missing => (Ty::Err, Val::Runtime),
            ExprKind::Int(v) => (Ty::Int, Val::Int(*v as i128)),
            ExprKind::Float(_) => {
                let msg = "floating-point numbers are not available yet";
                self.err("R8", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
            ExprKind::Bool(_) => (Ty::Bool, Val::Known),
            ExprKind::Str(segs) => {
                let mut known = true;
                for s in segs {
                    if let StrSeg::Expr(x) = s {
                        known = false;
                        let (t, _) = self.rvalue(x);
                        if t == Ty::Void {
                            let msg = format!("`{}` has no value to put into text", self.text(x));
                            self.err("R28", x.pos, msg, None);
                        }
                    }
                }
                (Ty::Str, if known { Val::Known } else { Val::Runtime })
            }
            ExprKind::Ident(n) => self.lookup(n, e.pos),
            ExprKind::Paren(x) => self.expr(x),
            ExprKind::Unary(op, x) => self.unary(op, x, e.pos),
            ExprKind::Binary(op, l, r, opos) => self.binary(op, l, r, *opos),
            ExprKind::Call(f, args) => self.call(e, f, args),
            ExprKind::Index(..) | ExprKind::Field(..) => {
                let msg = format!(
                    "`{}`: fields and indexes come in a later part",
                    self.text(e)
                );
                self.err("R8", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
        }
    }

    fn unary(&mut self, op: &str, x: &'a Expr, pos: Pos) -> (Ty, Val) {
        if op == "!" {
            let v = self.cond(x, "!");
            return (Ty::Bool, if v == Val::Runtime { v } else { Val::Known });
        }
        if op == "try" {
            self.err(
                "R8",
                pos,
                "`try` needs a result type T!E (a later part)",
                None,
            );
            return (Ty::Err, Val::Runtime);
        }
        let (t, v) = self.expr(x);
        if t == Ty::Err {
            return (t, Val::Runtime);
        }
        if t != Ty::Int {
            self.err(
                "R8",
                pos,
                format!("cannot apply `{op}` to {}", t.name()),
                None,
            );
            return (Ty::Err, Val::Runtime);
        }
        match (op, v) {
            ("-", Val::Int(n)) => (Ty::Int, Val::Int(-n)),
            ("~", Val::Int(n)) if fits(n) => (Ty::Int, Val::Int(!(n as i64) as i128)),
            ("~", Val::Int(_)) => {
                self.out_of_range(x.pos);
                (Ty::Err, Val::Runtime)
            }
            _ => (Ty::Int, Val::Runtime),
        }
    }

    fn binary(&mut self, op: &'static str, l: &'a Expr, r: &'a Expr, opos: Pos) -> (Ty, Val) {
        if op == "&&" || op == "||" {
            let known = self.cond(l, op) != Val::Runtime;
            let known = self.cond(r, op) != Val::Runtime && known;
            return (Ty::Bool, if known { Val::Known } else { Val::Runtime });
        }
        let (lt, lv) = self.expr(l);
        let (rt, rv) = self.expr(r);
        if lt == Ty::Err || rt == Ty::Err {
            return (Ty::Err, Val::Runtime);
        }
        let ty = match op {
            "+" if lt == Ty::Str && rt == Ty::Str => Ty::Str,
            "+" | "-" | "*" | "/" | "%" | "+%" | "-%" | "*%" | "&" | "|" | "^" | "<<" | ">>"
                if lt == Ty::Int && rt == Ty::Int =>
            {
                Ty::Int
            }
            "==" | "!=" if lt == rt && lt != Ty::Void => Ty::Bool,
            "<" | "<=" | ">" | ">=" if lt == rt && (lt == Ty::Int || lt == Ty::Str) => Ty::Bool,
            "??" => {
                let msg = "`??` needs an optional value ?T (a later part)";
                self.err("R8", opos, msg, None);
                return (Ty::Err, Val::Runtime);
            }
            _ => {
                let msg = format!("cannot apply `{op}` to {} and {}", lt.name(), rt.name());
                let hint = (op == "+" && (lt == Ty::Str || rt == Ty::Str))
                    .then(|| "build text with interpolation: \"{a}{b}\"".to_string());
                self.err("R8", opos, msg, hint);
                return (Ty::Err, Val::Runtime);
            }
        };
        if let (Val::Int(a), Val::Int(b)) = (lv, rv) {
            if ty == Ty::Int {
                return match self.fold(op, a, b, opos, (l, r)) {
                    Some(v) => (Ty::Int, Val::Int(v)),
                    None => (Ty::Err, Val::Runtime),
                };
            }
        }
        // a constant operand next to a runtime one is used at its place: it must fit
        for (v, e) in [(lv, l), (rv, r)] {
            if let Val::Int(n) = v {
                if !fits(n) {
                    self.out_of_range(e.pos);
                    return (Ty::Err, Val::Runtime);
                }
            }
        }
        let known = lv != Val::Runtime && rv != Val::Runtime;
        (ty, if known { Val::Known } else { Val::Runtime })
    }

    /// Exact arithmetic on constants (R8, R29); a certain abort is R14.
    fn fold(
        &mut self,
        op: &str,
        a: i128,
        b: i128,
        opos: Pos,
        (l, r): (&Expr, &Expr),
    ) -> Option<i128> {
        let bits = matches!(op, "+%" | "-%" | "*%" | "&" | "|" | "^" | "<<" | ">>");
        if bits {
            for (n, e) in [(a, l), (b, r)] {
                if !fits(n) {
                    self.out_of_range(e.pos);
                    return None;
                }
            }
        }
        let (x, y) = (a as i64, b as i64);
        let v = match op {
            "+" => a.checked_add(b),
            "-" => a.checked_sub(b),
            "*" => a.checked_mul(b),
            "/" | "%" if b == 0 => {
                self.err("R14", opos, "division by zero", None);
                return None;
            }
            "/" => Some(a / b),
            "%" => Some(a % b),
            "+%" => Some(x.wrapping_add(y) as i128),
            "-%" => Some(x.wrapping_sub(y) as i128),
            "*%" => Some(x.wrapping_mul(y) as i128),
            "&" => Some((x & y) as i128),
            "|" => Some((x | y) as i128),
            "^" => Some((x ^ y) as i128),
            "<<" | ">>" if !(0..64).contains(&b) => {
                self.err(
                    "R14",
                    opos,
                    format!("shift by {b} out of range 0..63"),
                    None,
                );
                return None;
            }
            "<<" => Some((x << y) as i128),
            _ => Some((x >> y) as i128),
        };
        if v.is_none() {
            self.out_of_range(opos);
        }
        v
    }

    fn call(&mut self, e: &'a Expr, f: &'a Expr, args: &'a [Expr]) -> (Ty, Val) {
        let ExprKind::Ident(name) = &f.kind else {
            self.err(
                "R8",
                f.pos,
                format!("`{}` cannot be called", self.text(f)),
                None,
            );
            return (Ty::Err, Val::Runtime);
        };
        if let Some(l) = self.local(name) {
            let msg = format!("`{name}` is {}, not a function", l.ty.name());
            self.err("R8", f.pos, msg, None);
            return (Ty::Err, Val::Runtime);
        }
        match name.as_str() {
            "print" => {
                if args.len() != 1 {
                    let msg = format!("`print` takes 1 argument, found {}", args.len());
                    self.err("R8", e.pos, msg, None);
                }
                for a in args {
                    let (t, _) = self.rvalue(a);
                    if t == Ty::Void {
                        let msg = format!("`{}` has no value to print", self.text(a));
                        self.err("R28", a.pos, msg, None);
                    }
                }
                return (Ty::Void, Val::Runtime);
            }
            "range" => {
                let hint = "for int i in range(n) { … }".to_string();
                let msg = "`range` can only be the source of a for loop";
                self.err("R11", e.pos, msg, Some(hint));
                return (Ty::Err, Val::Runtime);
            }
            "repeat" | "expect" | "assert" => {
                self.err("R8", f.pos, format!("`{name}` is not available yet"), None);
                return (Ty::Err, Val::Runtime);
            }
            _ => {}
        }
        let Some(sig) = self.funcs.get(name.as_str()) else {
            if self.consts.contains_key(name.as_str()) {
                self.err(
                    "R8",
                    f.pos,
                    format!("`{name}` is a const, not a function"),
                    None,
                );
            } else {
                self.unknown_name(name, f.pos);
            }
            return (Ty::Err, Val::Runtime);
        };
        let (params, ret) = (sig.params.clone(), sig.ret);
        if args.len() != params.len() {
            let msg = format!(
                "`{name}` takes {} argument{}, found {}",
                params.len(),
                if params.len() == 1 { "" } else { "s" },
                args.len()
            );
            self.err("R8", e.pos, msg, None);
            return (Ty::Err, Val::Runtime);
        }
        for (a, p) in args.iter().zip(params) {
            let (t, _) = self.rvalue(a);
            self.expect(p, t, a);
        }
        (ret, Val::Runtime)
    }
}

fn fits(n: i128) -> bool {
    (i64::MIN as i128..=i64::MAX as i128).contains(&n)
}

fn zero(t: Ty) -> &'static str {
    match t {
        Ty::Bool => "false",
        Ty::Str => "\"\"",
        _ => "0",
    }
}

/// Does every path through the block end in `return`?
fn terminates(b: &Block) -> bool {
    match b.stmts.last() {
        Some(Stmt::Return(..)) => true,
        Some(Stmt::If {
            branches,
            otherwise: Some(o),
            ..
        }) => branches.iter().all(|(_, b)| terminates(b)) && terminates(o),
        Some(Stmt::While { cond, body, .. }) => {
            matches!(cond.kind, ExprKind::Bool(true)) && !breaks(body)
        }
        _ => false,
    }
}

/// Does a `break` leave this loop body (not one of an inner loop)?
fn breaks(b: &Block) -> bool {
    b.stmts.iter().any(|s| match s {
        Stmt::Break(_) => true,
        Stmt::If {
            branches,
            otherwise,
            ..
        } => branches.iter().any(|(_, b)| breaks(b)) || otherwise.as_ref().is_some_and(breaks),
        _ => false,
    })
}

/// The candidate with the smallest edit distance, if it is close enough.
fn closest<'b>(name: &str, candidates: impl Iterator<Item = &'b str>) -> Option<&'b str> {
    let limit = (name.chars().count() / 3).max(1);
    candidates
        .filter(|c| *c != name)
        .map(|c| (distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min()
        .map(|(_, c)| c)
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}
