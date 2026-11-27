//! Generic declarations become code by copying (R27): the checker asks for an instance (a
//! function `max[int]`, a type `Pair[int,str]` with its methods), this module writes the copy
//! of the declaration with the type parameters replaced, and the checker and the backend see
//! an ordinary declaration. A copy keeps the line and column of its template, so a message
//! points into the generic text; its byte offsets are moved by a multiple of `stride`, so every
//! copy has spans of its own (the checker and the backend key their facts by span).

use crate::ast::*;
use crate::diag::{Diag, Pos};

/// An instance the checker asks for.
#[derive(Debug, Clone)]
pub struct Request {
    /// `true` for a type (with its methods), `false` for a function.
    pub is_type: bool,
    /// The name of the generic declaration.
    pub generic: String,
    /// The name of the instance as the checker looks it up: `max[int]`, `Pair[int,str]`.
    pub key: String,
    /// The name of the instance in messages: `Pair[int, str]`.
    pub display: String,
    /// The type arguments in the spelling of keys.
    pub args: Vec<String>,
    /// Where the instance was asked for.
    pub pos: Pos,
    /// How deep instances nest at this place.
    pub depth: u32,
}

/// The longest chain of instances that ask for further instances (R27).
pub const MAX_DEPTH: u32 = 16;

/// The generic declarations of a module: functions, structs, enums and their methods.
#[derive(Default)]
pub struct Templates {
    pub decls: Vec<Decl>,
}

/// The base name of a type `Pair[int,str]` or `Pair`.
pub fn base_of(name: &str) -> &str {
    name.split('[').next().unwrap_or(name)
}

/// Takes the generic declarations out of a module: a function or type with type parameters
/// and the methods of such a type.
pub fn split(m: Module) -> (Templates, Module) {
    let generic: Vec<String> = m
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Struct(s) if !s.tparams.is_empty() => Some(s.name.clone()),
            Decl::Enum(e) if !e.tparams.is_empty() => Some(e.name.clone()),
            _ => None,
        })
        .collect();
    let (mut t, mut rest) = (Vec::new(), Vec::new());
    for d in m.decls {
        let is_template = match &d {
            Decl::Func(f) => match &f.recv {
                Some(r) => generic.iter().any(|g| g == base_of(&r.name)),
                None => !f.tparams.is_empty(),
            },
            Decl::Struct(s) => !s.tparams.is_empty(),
            Decl::Enum(e) => !e.tparams.is_empty(),
            _ => false,
        };
        if is_template {
            t.push(d)
        } else {
            rest.push(d)
        }
    }
    (Templates { decls: t }, Module { decls: rest })
}

/// The key of the type that stands for a type parameter inside its own declaration: the
/// checker checks a generic body once, with these opaque types (R27). `$` is not a letter of
/// a name, so no program can spell them; one with two `$` is ordered.
pub fn opaque_key(owner: &str, param: &TParam) -> String {
    let ordered = matches!(&param.bound, Some((b, _)) if b == "ordered");
    format!("{}{owner}${}", if ordered { "$$" } else { "$" }, param.name)
}

fn tparams_of(d: &Decl) -> &[TParam] {
    match d {
        Decl::Func(f) => &f.tparams,
        Decl::Struct(s) => &s.tparams,
        Decl::Enum(e) => &e.tparams,
        _ => &[],
    }
}

impl Templates {
    fn generic_type(&self, name: &str) -> Option<&Decl> {
        self.decls.iter().find(|d| match d {
            Decl::Struct(s) => s.name == name,
            Decl::Enum(e) => e.name == name,
            _ => false,
        })
    }

    /// The declarations that stand for the generic declarations themselves, with opaque
    /// types for their type parameters; their diagnostics are the ones of the generic text.
    pub fn seed(&self) -> Vec<Decl> {
        let mut out = Vec::new();
        let mut named: Vec<&str> = Vec::new();
        for d in &self.decls {
            // the first declaration of a name stands for it; the checker reports the others
            if !matches!(d, Decl::Func(f) if f.recv.is_some()) {
                if named.contains(&type_name(d)) {
                    continue;
                }
                named.push(type_name(d));
            }
            match d {
                Decl::Func(f) if f.recv.is_none() => {
                    let env: Vec<(String, String)> = f
                        .tparams
                        .iter()
                        .map(|p| (p.name.clone(), opaque_key(&f.name, p)))
                        .collect();
                    opaque_decls(&f.name, &f.tparams, &mut out);
                    let keys: Vec<&str> = env.iter().map(|e| e.1.as_str()).collect();
                    let mut c = copy_func(f, &env, 0);
                    c.name = format!("{}[{}]", f.name, keys.join(","));
                    c.inst = Inst {
                        depth: 0,
                        primary: true,
                    };
                    out.push(Decl::Func(c));
                }
                Decl::Struct(_) | Decl::Enum(_) => {
                    let (name, tps) = (type_name(d), tparams_of(d));
                    opaque_decls(name, tps, &mut out);
                    let args: Vec<String> = tps.iter().map(|p| opaque_key(name, p)).collect();
                    let shown: Vec<&str> = tps.iter().map(|p| p.name.as_str()).collect();
                    let primary = Inst {
                        depth: 0,
                        primary: true,
                    };
                    out.extend(self.type_copy(d, &args, &shown.join(", "), 0, 0, primary));
                }
                _ => {}
            }
        }
        out
    }

    /// The copy of a type and its methods for the type arguments `args` (keys); `shown` is
    /// the argument list of messages.
    fn type_copy(
        &self,
        d: &Decl,
        args: &[String],
        shown: &str,
        depth: u32,
        shift: u32,
        inst: Inst,
    ) -> Vec<Decl> {
        let name = type_name(d);
        let key = format!("{name}[{}]", args.join(","));
        let display = format!("{name}[{shown}]");
        let inst = Inst { depth, ..inst };
        let env: Vec<(String, String)> = tparams_of(d)
            .iter()
            .zip(args)
            .map(|(p, a)| (p.name.clone(), a.clone()))
            .collect();
        let mut out = Vec::new();
        match d {
            Decl::Struct(s) => {
                let mut c = s.clone();
                Walk::new(&env, shift).struct_decl(&mut c);
                c.name = key.clone();
                c.display = Some(display);
                c.tparams.clear();
                c.inst = inst;
                out.push(Decl::Struct(c));
            }
            Decl::Enum(e) => {
                let mut c = e.clone();
                Walk::new(&env, shift).enum_decl(&mut c);
                c.name = key.clone();
                c.display = Some(display);
                c.tparams.clear();
                c.inst = inst;
                out.push(Decl::Enum(c));
            }
            _ => return out,
        }
        for m in &self.decls {
            let Decl::Func(f) = m else { continue };
            let Some(r) = &f.recv else { continue };
            if base_of(&r.name) != name {
                continue;
            }
            // the method names the type parameters itself: `A Pair[A, B].left()`
            let names = recv_params(&r.name);
            let menv: Vec<(String, String)> = names.into_iter().zip(args.iter().cloned()).collect();
            let mut c = copy_func(f, &menv, shift);
            c.recv = Some(TypeRef {
                name: key.clone(),
                pos: r.pos,
            });
            c.inst = inst;
            out.push(Decl::Func(c));
        }
        out
    }

    /// The declarations for one request of the checker.
    pub fn instance(&self, r: &Request, shift: u32) -> Vec<Decl> {
        let inst = Inst {
            depth: r.depth,
            primary: false,
        };
        if r.is_type {
            let Some(d) = self.generic_type(&r.generic) else {
                return Vec::new();
            };
            let shown = r.display.strip_prefix(&format!("{}[", r.generic));
            let shown = shown.and_then(|s| s.strip_suffix(']')).unwrap_or("");
            return self.type_copy(d, &r.args, shown, r.depth, shift, inst);
        }
        let f = self.decls.iter().find_map(|d| match d {
            Decl::Func(f) if f.recv.is_none() && f.name == r.generic => Some(f),
            _ => None,
        });
        let Some(f) = f else { return Vec::new() };
        let env: Vec<(String, String)> = f
            .tparams
            .iter()
            .zip(&r.args)
            .map(|(p, a)| (p.name.clone(), a.clone()))
            .collect();
        let mut c = copy_func(f, &env, shift);
        c.name = r.key.clone();
        c.inst = inst;
        vec![Decl::Func(c)]
    }
}

pub fn type_name(d: &Decl) -> &str {
    match d {
        Decl::Struct(s) => &s.name,
        Decl::Enum(e) => &e.name,
        Decl::Func(f) => &f.name,
        _ => "",
    }
}

/// The names a method gives the type parameters: `["A", "B"]` for `Pair[A,B]`.
pub fn recv_params(recv: &str) -> Vec<String> {
    match recv.split_once('[') {
        Some((_, rest)) => rest
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().to_string())
            .collect(),
        None => Vec::new(),
    }
}

/// The empty struct that stands for each type parameter in the generic text.
fn opaque_decls(owner: &str, tps: &[TParam], out: &mut Vec<Decl>) {
    for p in tps {
        let key = opaque_key(owner, p);
        if out
            .iter()
            .any(|d| matches!(d, Decl::Struct(s) if s.name == key))
        {
            continue;
        }
        out.push(Decl::Struct(StructDecl {
            name: opaque_key(owner, p),
            pos: p.pos,
            fields: Vec::new(),
            tparams: Vec::new(),
            display: Some(p.name.clone()),
            inst: Inst::default(),
        }));
    }
}

fn copy_func(f: &Func, env: &[(String, String)], shift: u32) -> Func {
    let mut c = f.clone();
    Walk::new(env, shift).func(&mut c);
    c.tparams.clear();
    c
}

/// The type parameters that a generic declaration mentions in its types (for R27's unused
/// parameters), found by walking a copy.
pub fn mentioned(d: &Decl) -> Vec<String> {
    let mut w = Walk::new(&[], 0);
    w.collect = true;
    match d {
        Decl::Func(f) => w.func(&mut f.clone()),
        Decl::Struct(s) => w.struct_decl(&mut s.clone()),
        Decl::Enum(e) => w.enum_decl(&mut e.clone()),
        _ => {}
    }
    w.seen
}

/// The names in a type text; a name after a `.` belongs to a module and is left out.
pub fn idents(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_alphabetic() || chars[i] == '_' || chars[i] == '$' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
            {
                i += 1;
            }
            if start == 0 || chars[start - 1] != '.' {
                out.push(chars[start..i].iter().collect());
            }
        } else {
            i += 1;
        }
    }
    out
}

/// A type text with its type parameters replaced; a result or function type that lands next
/// to `]` or `?` gets parentheses.
pub fn subst_text(s: &str, env: &[(String, String)]) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let after_dot = start > 0 && chars[start - 1] == '.';
            match env.iter().find(|(n, _)| *n == word) {
                Some((_, a)) if !after_dot => {
                    let prev = out.chars().last();
                    let wrap = (a.contains('!') || a.contains('('))
                        && (matches!(prev, Some(']') | Some('?')) || out.ends_with("map["));
                    if wrap {
                        out.push('(');
                        out.push_str(a);
                        out.push(')');
                    } else {
                        out.push_str(a);
                    }
                }
                _ => out.push_str(&word),
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// A copy of the syntax tree with the type parameters replaced and every byte offset moved.
struct Walk<'e> {
    env: &'e [(String, String)],
    shift: u32,
    /// Only collect the names the types mention.
    collect: bool,
    seen: Vec<String>,
}

impl<'e> Walk<'e> {
    fn new(env: &'e [(String, String)], shift: u32) -> Self {
        Walk {
            env,
            shift,
            collect: false,
            seen: Vec::new(),
        }
    }

    fn pos(&self, p: &mut Pos) {
        p.offset += self.shift;
    }

    fn opt_pos(&self, p: &mut Option<Pos>) {
        if let Some(p) = p {
            self.pos(p);
        }
    }

    fn ty(&mut self, t: &mut TypeRef) {
        self.pos(&mut t.pos);
        if self.collect {
            self.seen.extend(idents(&t.name));
        } else {
            t.name = subst_text(&t.name, self.env);
        }
    }

    fn param(&mut self, p: &mut Param) {
        self.ty(&mut p.ty);
        self.pos(&mut p.pos);
        self.opt_pos(&mut p.mut_pos);
        if let Some(d) = &mut p.default {
            self.expr(d);
        }
    }

    fn tparam(&self, p: &mut TParam) {
        self.pos(&mut p.pos);
        if let Some((_, bp)) = &mut p.bound {
            self.pos(bp);
        }
    }

    fn func(&mut self, f: &mut Func) {
        self.ty(&mut f.ret);
        if let Some(r) = &mut f.recv {
            self.ty(r);
        }
        self.pos(&mut f.pos);
        self.opt_pos(&mut f.mut_self);
        for p in &mut f.params {
            self.param(p);
        }
        self.block(&mut f.body);
        for p in &mut f.tparams {
            self.tparam(p);
        }
    }

    fn struct_decl(&mut self, s: &mut StructDecl) {
        self.pos(&mut s.pos);
        for f in &mut s.fields {
            self.ty(&mut f.ty);
            self.pos(&mut f.pos);
            if let Some(d) = &mut f.default {
                self.expr(d);
            }
        }
        for p in &mut s.tparams {
            self.tparam(p);
        }
    }

    fn enum_decl(&mut self, e: &mut EnumDecl) {
        self.pos(&mut e.pos);
        for v in &mut e.variants {
            self.pos(&mut v.pos);
            for f in &mut v.fields {
                self.param(f);
            }
        }
        for p in &mut e.tparams {
            self.tparam(p);
        }
    }

    fn block(&mut self, b: &mut Block) {
        self.pos(&mut b.pos);
        for s in &mut b.stmts {
            self.stmt(s);
        }
    }

    fn value_block(&mut self, b: &mut ValueBlock) {
        self.pos(&mut b.pos);
        for s in &mut b.stmts {
            self.stmt(s);
        }
        if let Some(v) = &mut b.value {
            self.expr(v);
        }
    }

    fn var(&mut self, v: &mut VarDecl) {
        self.ty(&mut v.ty);
        self.pos(&mut v.pos);
        self.expr(&mut v.value);
    }

    fn stmt(&mut self, s: &mut Stmt) {
        match s {
            Stmt::Var(v) => self.var(v),
            Stmt::Assign {
                place, value, pos, ..
            } => {
                self.expr(place);
                self.expr(value);
                self.pos(pos);
            }
            Stmt::If {
                branches,
                otherwise,
                pos,
            } => {
                for (c, b) in branches {
                    self.expr(c);
                    self.block(b);
                }
                if let Some(b) = otherwise {
                    self.block(b);
                }
                self.pos(pos);
            }
            Stmt::While { cond, body, pos } => {
                self.expr(cond);
                self.block(body);
                self.pos(pos);
            }
            Stmt::For {
                vars,
                iter,
                body,
                pos,
            } => {
                for v in vars {
                    self.param(v);
                }
                self.expr(iter);
                self.block(body);
                self.pos(pos);
            }
            Stmt::Switch {
                subject,
                cases,
                default,
                pos,
            } => {
                self.expr(subject);
                for c in cases {
                    for p in &mut c.pats {
                        self.pattern(p);
                    }
                    self.block(&mut c.body);
                    self.pos(&mut c.pos);
                }
                if let Some((b, p)) = default {
                    self.block(b);
                    self.pos(p);
                }
                self.pos(pos);
            }
            Stmt::Break(p) | Stmt::Continue(p) => self.pos(p),
            Stmt::Return(v, p) => {
                if let Some(v) = v {
                    self.expr(v);
                }
                self.pos(p);
            }
            Stmt::Call(e) => self.expr(e),
            Stmt::Discard(e, p) => {
                self.expr(e);
                self.pos(p);
            }
        }
    }

    fn pattern(&mut self, p: &mut Pattern) {
        match p {
            Pattern::Int(_, p) | Pattern::Str(_, p) | Pattern::Rune(_, p) | Pattern::Name(_, p) => {
                self.pos(p)
            }
            Pattern::Variant { binds, pos, .. } => {
                for b in binds.iter_mut().flatten().flatten() {
                    self.param(b);
                }
                self.pos(pos);
            }
            Pattern::Result { bind, pos, .. } => {
                if let Some(Some(b)) = bind {
                    self.param(b);
                }
                self.pos(pos);
            }
        }
    }

    fn lit(&mut self, l: &mut FuncLit) {
        self.ty(&mut l.ret);
        for p in &mut l.params {
            self.param(p);
        }
        self.block(&mut l.body);
        if let Some(a) = &mut l.alt {
            self.expr(a);
        }
    }

    fn expr(&mut self, e: &mut Expr) {
        self.pos(&mut e.pos);
        e.end += self.shift;
        match &mut e.kind {
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Rune(_)
            | ExprKind::Bool(_)
            | ExprKind::Ident(_)
            | ExprKind::SelfRef
            | ExprKind::NoneVal
            | ExprKind::Missing => {}
            ExprKind::Str(segs) => {
                for s in segs {
                    match s {
                        StrSeg::Lit(_) => {}
                        StrSeg::Expr(x) | StrSeg::Fmt(x, _) => self.expr(x),
                    }
                }
            }
            ExprKind::Unary(_, x)
            | ExprKind::Paren(x)
            | ExprKind::Named(_, x)
            | ExprKind::Mut(x)
            | ExprKind::SomeVal(x)
            | ExprKind::ErrVal(x)
            | ExprKind::Field(x, _) => self.expr(x),
            ExprKind::Binary(_, l, r, p) => {
                self.expr(l);
                self.expr(r);
                self.pos(p);
            }
            ExprKind::Call(f, args) => {
                self.expr(f);
                for a in args {
                    self.expr(a);
                }
            }
            ExprKind::Index(b, i, p) => {
                self.expr(b);
                self.expr(i);
                self.pos(p);
            }
            ExprKind::StructLit(_, inits) => {
                for i in inits {
                    self.pos(&mut i.pos);
                    self.expr(&mut i.value);
                }
            }
            ExprKind::IfExpr(branches, last) => {
                for (c, b) in branches {
                    self.expr(c);
                    self.value_block(b);
                }
                self.value_block(last);
            }
            ExprKind::OkVal(x) => {
                if let Some(x) = x {
                    self.expr(x);
                }
            }
            ExprKind::Try(x, or) => {
                self.expr(x);
                if let Some(o) = or {
                    self.pos(&mut o.pos);
                    if let Some(b) = &mut o.bind {
                        self.param(b);
                    }
                    self.expr(&mut o.alt);
                }
            }
            ExprKind::ListLit(items) => {
                for i in items {
                    self.expr(i);
                }
            }
            ExprKind::MapLit(entries) => {
                for (k, v) in entries {
                    self.expr(k);
                    self.expr(v);
                }
            }
            ExprKind::FuncLit(l) => self.lit(l),
            ExprKind::TypeApp(b, targs) => {
                self.expr(b);
                for t in targs {
                    self.ty(t);
                }
            }
        }
    }
}

/// The diagnostics of the copies, mapped back to the text they were made from: a copy that
/// stands for its generic declaration counts, the others only where they report something the
/// declaration did not (the same place and kind is reported once). `stride` is the distance of
/// the offsets of two copies; `primary[k]` says whether copy `k` (counting from 1) is one that
/// stands for its declaration.
pub fn map_diags(diags: Vec<Diag>, stride: u32) -> Vec<Diag> {
    let back = |off: u32| off % stride;
    let any_own = diags.iter().any(|d| d.pos.offset < stride);
    let mut keep: Vec<(bool, Diag)> = diags
        .into_iter()
        .map(|mut d| {
            let k = (d.pos.offset / stride) as usize;
            let own = k == 0;
            d.pos.offset = back(d.pos.offset);
            if let Some(f) = &mut d.fix {
                f.offset = back(f.offset);
                f.end = back(f.end);
            }
            (own, d)
        })
        .collect();
    // a copy only repeats what the generic text reports; a message only a copy has is a fault
    // of the compiler, never the program's
    if any_own {
        keep.retain(|(own, _)| *own);
    } else if let Some((_, d)) = keep.iter().find(|(_, d)| d.id != "R27.instance_depth") {
        panic!(
            "internal error: an instance of generic text reports {}: {}",
            d.id, d.msg
        );
    }
    keep.sort_by_key(|(own, _)| !own);
    let mut out: Vec<Diag> = Vec::new();
    for (_, d) in keep {
        if !out
            .iter()
            .any(|o| o.id == d.id && o.pos.offset == d.pos.offset)
        {
            out.push(d);
        }
    }
    out.sort_by_key(|d| d.pos.offset);
    out
}
