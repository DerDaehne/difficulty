//! Generics (R27): names and checks of the generic declarations, type arguments, inference
//! at calls, and the requests for instances. A generic body is checked once, with opaque types
//! for its type parameters; an instance is a copy of the declaration that the driver writes
//! when the checker asks for it (`generics.rs`), so the code below never substitutes inside a
//! body.

use super::*;
use crate::generics::{self, Request, MAX_DEPTH};

/// A written type with holes: the type parameters of a generic declaration are `Param(i)`.
#[derive(Clone, Debug)]
enum Pat {
    Param(usize),
    Name,
    List(Box<Pat>),
    Arr(Box<Pat>),
    Map(Box<Pat>, Box<Pat>),
    Opt(Box<Pat>),
    Res(Box<Pat>, Box<Pat>),
    Func(Box<Pat>, Vec<Pat>),
    App(String, Vec<Pat>),
}

struct PatParser<'s> {
    s: &'s str,
    names: &'s [&'s str],
}

impl PatParser<'_> {
    fn eat(&mut self, c: char) -> bool {
        match self.s.strip_prefix(c) {
            Some(r) => {
                self.s = r;
                true
            }
            None => false,
        }
    }

    fn top(&mut self) -> Pat {
        let opt = self.eat('?');
        let mut t = if opt && self.s.starts_with('(') {
            self.eat('(');
            let t = self.top();
            self.eat(')');
            t
        } else {
            self.base()
        };
        if opt {
            t = Pat::Opt(Box::new(t));
        }
        if self.eat('!') {
            let e = self.base();
            t = Pat::Res(Box::new(t), Box::new(e));
        }
        if self.eat('(') {
            let mut ps = Vec::new();
            while !self.s.is_empty() && !self.s.starts_with(')') {
                if let Some(r) = self.s.strip_prefix("mut ") {
                    self.s = r;
                }
                ps.push(self.top());
                self.eat(',');
            }
            self.eat(')');
            t = Pat::Func(Box::new(t), ps);
        }
        t
    }

    fn elem(&mut self) -> Pat {
        if self.eat('(') {
            let t = self.top();
            self.eat(')');
            return t;
        }
        let opt = self.eat('?');
        let t = self.base();
        if opt {
            Pat::Opt(Box::new(t))
        } else {
            t
        }
    }

    fn base(&mut self) -> Pat {
        if self.eat('[') {
            let end = self.s.find(']').unwrap_or(self.s.len());
            let len = &self.s[..end];
            self.s = self.s.get(end + 1..).unwrap_or("");
            let el = Box::new(self.elem());
            return if len.is_empty() {
                Pat::List(el)
            } else {
                Pat::Arr(el)
            };
        }
        if let Some(r) = self.s.strip_prefix("map[") {
            self.s = r;
            let k = self.elem();
            self.eat(']');
            let v = self.elem();
            return Pat::Map(Box::new(k), Box::new(v));
        }
        let end = self
            .s
            .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '.' | '$')))
            .unwrap_or(self.s.len());
        let (n, rest) = self.s.split_at(end);
        self.s = rest;
        if self.eat('[') {
            let mut args = Vec::new();
            while !self.s.is_empty() && !self.s.starts_with(']') {
                args.push(self.top());
                self.eat(',');
            }
            self.eat(']');
            return Pat::App(n.to_string(), args);
        }
        match self.names.iter().position(|x| *x == n) {
            Some(i) => Pat::Param(i),
            None => Pat::Name,
        }
    }
}

fn pat(text: &str, names: &[&str]) -> Pat {
    PatParser { s: text, names }.top()
}

fn mentions(p: &Pat, i: usize) -> bool {
    match p {
        Pat::Param(j) => *j == i,
        Pat::Name => false,
        Pat::List(a) | Pat::Arr(a) | Pat::Opt(a) => mentions(a, i),
        Pat::Map(a, b) | Pat::Res(a, b) => mentions(a, i) || mentions(b, i),
        Pat::Func(r, ps) => mentions(r, i) || ps.iter().any(|q| mentions(q, i)),
        Pat::App(_, ps) => ps.iter().any(|q| mentions(q, i)),
    }
}

/// The arguments of `Pair[int,str]`, or of an instance key: `["int", "str"]`.
fn key_args(key: &str) -> Vec<&str> {
    let Some((_, rest)) = key.split_once('[') else {
        return Vec::new();
    };
    let inner = rest.strip_suffix(']').unwrap_or(rest);
    split_top(inner)
}

/// Splits a type list at the commas outside brackets and parentheses.
fn split_top(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if !s.is_empty() {
        out.push(&s[start..]);
    }
    out
}

/// What the arguments have shown about a type parameter: the type, and the parameter it
/// came from.
type Binds = Vec<Option<(Ty, usize)>>;

/// An argument that has no type of its own: it takes the one of its place (R8).
fn typeless(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::ListLit(v) => v.is_empty(),
        ExprKind::MapLit(v) => v.is_empty(),
        ExprKind::NoneVal | ExprKind::ErrVal(_) | ExprKind::OkVal(_) => true,
        ExprKind::Paren(x) | ExprKind::Mut(x) | ExprKind::Named(_, x) => typeless(x),
        _ => false,
    }
}

fn strip(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Mut(x) | ExprKind::Named(_, x) => strip(x),
        _ => e,
    }
}

fn plural(n: usize) -> String {
    format!("{n} type argument{}", if n == 1 { "" } else { "s" })
}

impl<'a> Checker<'a> {
    // ---- the generic declarations ----

    /// The names of the generic declarations, so the other names see them.
    pub(super) fn register_templates(&mut self) {
        let tpls = self.templates;
        for d in tpls {
            match d {
                Decl::Func(f) if f.recv.is_none() => {
                    if self.new_module_name(&f.name, f.pos) {
                        self.gen_funcs.insert(&f.name, f);
                    }
                }
                Decl::Struct(s) if self.new_module_name(&s.name, s.pos) => {
                    let g = GenType {
                        decl: d,
                        tparams: &s.tparams,
                        pos: s.pos,
                        methods: Vec::new(),
                    };
                    self.gen_types.insert(&s.name, g);
                }
                Decl::Enum(e) if self.new_module_name(&e.name, e.pos) => {
                    let g = GenType {
                        decl: d,
                        tparams: &e.tparams,
                        pos: e.pos,
                        methods: Vec::new(),
                    };
                    self.gen_types.insert(&e.name, g);
                }
                _ => {}
            }
        }
        for d in tpls {
            let Decl::Func(f) = d else { continue };
            let Some(r) = &f.recv else { continue };
            let base = generics::base_of(&r.name);
            let prev = self.gen_types.get(base).and_then(|g| {
                let own = g.methods.iter().find(|m| m.name == f.name);
                let field = match g.decl {
                    Decl::Struct(s) => s.fields.iter().find(|x| x.name == f.name).map(|x| x.pos),
                    _ => None,
                };
                own.map(|m| m.pos).or(field)
            });
            if let Some(prev) = prev {
                self.already(&f.name, f.pos, prev);
                continue;
            }
            if let Some(g) = self.gen_types.get_mut(base) {
                g.methods.push(f);
            }
        }
    }

    /// The header of a generic declaration as it stands in the source, for hints; `bound`
    /// names the type parameter that gets `ordered`.
    fn header_text(&self, d: &Decl, bound: Option<&str>) -> String {
        let tps = |ps: &[TParam]| -> String {
            let list: Vec<String> = ps
                .iter()
                .map(|p| match &p.bound {
                    Some((b, _)) if b == "ordered" => format!("ordered {}", p.name),
                    _ if Some(p.name.as_str()) == bound => format!("ordered {}", p.name),
                    _ => p.name.clone(),
                })
                .collect();
            list.join(", ")
        };
        match d {
            Decl::Func(f) => {
                let ps: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| {
                        let m = if p.mut_pos.is_some() { "mut " } else { "" };
                        format!("{m}{} {}", p.ty.name, p.name)
                    })
                    .collect();
                format!(
                    "{} {}[{}]({})",
                    f.ret.name,
                    f.name,
                    tps(&f.tparams),
                    ps.join(", ")
                )
            }
            Decl::Struct(s) => format!("struct {}[{}]", s.name, tps(&s.tparams)),
            Decl::Enum(e) => format!("enum {}[{}]", e.name, tps(&e.tparams)),
            _ => String::new(),
        }
    }

    /// Checks what the declaration of a generic function, struct or enum says about its type
    /// parameters (R27), and the receivers of the methods of generic types.
    pub(super) fn check_templates(&mut self) {
        let tpls = self.templates;
        for d in tpls {
            let (name, tps): (&str, &[TParam]) = match d {
                Decl::Func(f) if f.recv.is_none() => (&f.name, &f.tparams),
                Decl::Struct(s) => (&s.name, &s.tparams),
                Decl::Enum(e) => (&e.name, &e.tparams),
                _ => continue,
            };
            let used = generics::mentioned(d);
            let mut seen: Vec<(&str, Pos)> = Vec::new();
            for p in tps {
                if let Some((_, prev)) = seen.iter().find(|(n, _)| *n == p.name) {
                    let prev = *prev;
                    self.already(&p.name, p.pos, prev);
                } else if !self.predefined(&p.name, p.pos) {
                    if let Some(prev) = self.module_pos(&p.name) {
                        self.already(&p.name, p.pos, prev);
                    }
                }
                seen.push((&p.name, p.pos));
                if let Some((word, wpos)) = &p.bound {
                    self.constraint(d, p, word, *wpos);
                }
                if !used.contains(&p.name) {
                    let msg = format!("type parameter `{}` is never used", p.name);
                    self.err(
                        "R27.type_param_unused",
                        p.pos,
                        msg,
                        Some("remove it".into()),
                    );
                }
            }
            let _ = name;
        }
        let list: Vec<(&str, &Func)> = self
            .gen_types
            .iter()
            .flat_map(|(n, g)| g.methods.iter().map(move |m| (*n, *m)))
            .collect();
        for (n, m) in list {
            let (Some(r), Some(g)) = (&m.recv, self.gen_types.get(n)) else {
                continue;
            };
            let want = g.tparams.len();
            self.method_tparams(m);
            let found = generics::recv_params(&r.name).len();
            if want != found {
                let msg = format!("`{n}` takes {}, found {found}", plural(want));
                self.err("R27.type_arg_count", r.pos, msg, None);
            }
        }
    }

    /// The word before a type parameter: only `ordered` is a constraint.
    fn constraint(&mut self, d: &Decl, p: &TParam, word: &str, wpos: Pos) {
        match word {
            "ordered" => {}
            "equal" | "hashable" | "text" => {
                let hint = format!("remove it: {}", self.header_text_plain(d));
                let msg = format!("`{word}` is not a constraint: every type has it");
                let fix = Diag::new("R27.constraint_every_type", wpos, msg)
                    .hint(hint)
                    .fix(wpos.offset, p.pos.offset, "");
                self.diags.push(fix);
            }
            _ => {
                let msg = format!("`{word}` is not a constraint");
                let hint = "the only constraint is ordered".to_string();
                self.err("R27.constraint_unknown", wpos, msg, Some(hint));
            }
        }
    }

    /// The header without constraints, as `remove it` shows it.
    fn header_text_plain(&self, d: &Decl) -> String {
        let h = self.header_text(d, None);
        h.replace("ordered ", "")
    }

    /// The types that exist only for the check of generic text.
    pub(super) fn hidden_types(&self) -> HashSet<Ty> {
        let mut out = HashSet::new();
        let all = (0..self.types.len() as u32)
            .map(Ty::Named)
            .chain((0..self.opts.len() as u32).map(Ty::Opt))
            .chain((0..self.ress.len() as u32).map(Ty::Res))
            .chain((0..self.lists.len() as u32).map(Ty::List))
            .chain((0..self.arrs.len() as u32).map(Ty::Arr))
            .chain((0..self.maps.len() as u32).map(Ty::Map))
            .chain((0..self.ftys.len() as u32).map(Ty::Func));
        for t in all {
            if self.is_hidden(t, 0) {
                out.insert(t);
            }
        }
        out
    }

    fn is_hidden(&self, t: Ty, depth: u32) -> bool {
        if depth > 64 {
            return false;
        }
        let d = depth + 1;
        match t {
            Ty::Named(id) => self.types[id as usize].key.contains('$'),
            Ty::Opt(i) => self.is_hidden(self.opts[i as usize], d),
            Ty::Res(i) => {
                let (a, b) = self.ress[i as usize];
                self.is_hidden(a, d) || self.is_hidden(b, d)
            }
            Ty::List(i) => self.is_hidden(self.lists[i as usize], d),
            Ty::Arr(i) => self.is_hidden(self.arrs[i as usize].0, d),
            Ty::Map(i) => {
                let (k, v) = self.maps[i as usize];
                self.is_hidden(k, d) || self.is_hidden(v, d)
            }
            Ty::Func(i) => {
                let (r, ps) = &self.ftys[i as usize];
                self.is_hidden(*r, d) || ps.iter().any(|p| self.is_hidden(p.0, d))
            }
            _ => false,
        }
    }

    // ---- types ----

    /// The type as an instance key spells it: no spaces, the keys of the types.
    pub(super) fn tk(&self, t: Ty) -> String {
        match t {
            Ty::Named(id) => self.types[id as usize].key.to_string(),
            Ty::Opt(i) => match self.opts[i as usize] {
                Ty::Func(_) => format!("?({})", self.tk(self.opts[i as usize])),
                inner => format!("?{}", self.tk(inner)),
            },
            Ty::Res(i) => {
                let (v, e) = self.ress[i as usize];
                format!("{}!{}", self.tk(v), self.tk(e))
            }
            Ty::List(i) => format!("[]{}", self.tk_elem(self.lists[i as usize])),
            Ty::Arr(i) => {
                let (v, n) = self.arrs[i as usize];
                format!("[{n}]{}", self.tk_elem(v))
            }
            Ty::Map(i) => {
                let (k, v) = self.maps[i as usize];
                format!("map[{}]{}", self.tk_elem(k), self.tk_elem(v))
            }
            Ty::Func(i) => {
                let (r, ps) = &self.ftys[i as usize];
                let ps: Vec<String> = ps
                    .iter()
                    .map(|(t, m)| format!("{}{}", if *m { "mut " } else { "" }, self.tk(*t)))
                    .collect();
                format!("{}({})", self.tk(*r), ps.join(","))
            }
            t => self.tn(t),
        }
    }

    fn tk_elem(&self, t: Ty) -> String {
        match t {
            Ty::Res(_) | Ty::Func(_) => format!("({})", self.tk(t)),
            _ => self.tk(t),
        }
    }

    /// A type written in the spelling of keys, without reporting anything.
    fn ty_text(&mut self, s: &str) -> Ty {
        let (d, h) = (self.diags.len(), self.hints.len());
        let mut rest = s;
        let t = self.ty_top(&mut rest, Pos::default(), false, true);
        self.diags.truncate(d);
        self.hints.truncate(h);
        t
    }

    /// Can `<` compare values of this type: numbers, `rune`, `str` and `ordered` type parameters.
    pub(super) fn is_ordered(&self, t: Ty) -> bool {
        t.is_number()
            || matches!(t, Ty::Rune | Ty::Str | Ty::Err)
            || matches!(t, Ty::Named(id) if self.types[id as usize].opaque == Some(true))
    }

    /// An opaque type that is not ordered: `<` on it is R28.
    pub(super) fn unordered_param(&self, t: Ty) -> bool {
        matches!(t, Ty::Named(id) if self.types[id as usize].opaque == Some(false))
    }

    /// A generic type written with type arguments: `Pair[int, str]`; `s` is at its `[`.
    pub(super) fn generic_application(&mut self, n: &'a str, s: &mut &str, pos: Pos) -> Ty {
        let mut raw = n.to_string();
        let mut args: Vec<String> = Vec::new();
        if s.starts_with('[') {
            let mut depth = 0i32;
            let mut end = s.len();
            for (i, c) in s.char_indices() {
                match c {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let inner = &s[1..end.max(1)];
            args = split_top(inner).into_iter().map(String::from).collect();
            raw = format!("{n}[{inner}]");
            *s = s.get(end + 1..).unwrap_or("");
        }
        let tys: Vec<Ty> = args
            .iter()
            .map(|a| {
                let mut t = a.as_str();
                self.ty_top(&mut t, pos, false, true)
            })
            .collect();
        self.generic_type(n, tys, &raw, pos)
    }

    /// The instance `n[tys]` of a generic type: its type, or `Err` while the driver still has
    /// to write it.
    pub(super) fn generic_type(&mut self, n: &str, tys: Vec<Ty>, raw: &str, pos: Pos) -> Ty {
        let Some(g) = self.gen_types.get(n) else {
            return Ty::Err;
        };
        let tps = g.tparams;
        if tys.len() != tps.len() {
            let msg = format!("`{n}` takes {}, found {}", plural(tps.len()), tys.len());
            self.err("R27.type_arg_count", pos, msg, None);
            return Ty::Err;
        }
        if tys.contains(&Ty::Err) {
            return Ty::Err;
        }
        for (p, t) in tps.iter().zip(&tys) {
            let ordered = matches!(&p.bound, Some((b, _)) if b == "ordered");
            if ordered && !self.is_ordered(*t) {
                self.ordered_missing(n, &p.name, *t, pos);
                return Ty::Err;
            }
        }
        let args: Vec<String> = tys.iter().map(|t| self.tk(*t)).collect();
        let key = format!("{n}[{}]", args.join(","));
        if raw != key {
            self.aliases.insert(raw.to_string(), key.clone());
        }
        if let Some(&id) = self.type_ids.get(key.as_str()) {
            return Ty::Named(id);
        }
        let shown: Vec<String> = tys.iter().map(|t| self.tn(*t)).collect();
        let display = format!("{n}[{}]", shown.join(", "));
        self.want(true, n, key, display, args, pos);
        Ty::Err
    }

    fn ordered_missing(&mut self, name: &str, param: &str, found: Ty, pos: Pos) {
        let msg = format!(
            "`{name}` needs `{param}` to be ordered, found {}",
            self.tn(found)
        );
        let hint = "only numbers, rune and str are ordered".to_string();
        self.err("R28.ordered_missing", pos, msg, Some(hint));
    }

    /// Asks the driver for an instance.
    fn want(
        &mut self,
        is_type: bool,
        generic: &str,
        key: String,
        display: String,
        args: Vec<String>,
        pos: Pos,
    ) {
        if !self.requested.insert(key.clone()) {
            return;
        }
        let depth = self.depth + 1;
        if depth > MAX_DEPTH {
            let msg = format!("the instances of `{generic}` nest deeper than {MAX_DEPTH} levels");
            self.err("R27.instance_depth", pos, msg, None);
            return;
        }
        self.requests.push(Request {
            is_type,
            generic: generic.to_string(),
            key,
            display,
            args,
            pos,
            depth,
        });
    }

    // ---- calls ----

    /// The type of an argument for inference, without anything it reports.
    fn peek_ty(&mut self, a: &'a Expr) -> Option<Ty> {
        let inner = strip(a);
        if typeless(inner) {
            return None;
        }
        let (d, h) = (self.diags.len(), self.hints.len());
        let (t, _) = self.expr(inner);
        self.diags.truncate(d);
        self.hints.truncate(h);
        Some(t)
    }

    fn bind(&self, i: usize, t: Ty, from: usize, binds: &mut Binds) -> Result<(), (Ty, usize)> {
        if matches!(t, Ty::Err | Ty::Void) {
            return Ok(());
        }
        let Some((b, f)) = binds[i] else {
            binds[i] = Some((t, from));
            return Ok(());
        };
        if b == t {
            return Ok(());
        }
        let fits = |lit: Ty, to: Ty| match lit {
            Ty::IntLit => to.is_number(),
            Ty::FloatLit => to.is_float(),
            _ => false,
        };
        if t.is_lit() && (fits(t, b) || b.is_lit()) {
            if b == Ty::IntLit && t == Ty::FloatLit {
                binds[i] = Some((t, f));
            }
            Ok(())
        } else if b.is_lit() && fits(b, t) {
            binds[i] = Some((t, from));
            Ok(())
        } else {
            Err((b, f))
        }
    }

    /// Matches a written parameter type with the type of an argument.
    fn unify(
        &mut self,
        p: &Pat,
        t: Ty,
        from: usize,
        binds: &mut Binds,
    ) -> Result<(), (usize, Ty, usize)> {
        let wrap = |i: usize, r: Result<(), (Ty, usize)>| r.map_err(|(b, f)| (i, b, f));
        match p {
            Pat::Param(i) => wrap(*i, self.bind(*i, t, from, binds)),
            Pat::Name => Ok(()),
            Pat::List(q) => match t {
                Ty::List(i) => self.unify(q, self.lists[i as usize], from, binds),
                _ => Ok(()),
            },
            Pat::Arr(q) => match t {
                Ty::Arr(i) => self.unify(q, self.arrs[i as usize].0, from, binds),
                _ => Ok(()),
            },
            Pat::Map(k, v) => match t {
                Ty::Map(i) => {
                    let (kt, vt) = self.maps[i as usize];
                    self.unify(k, kt, from, binds)?;
                    self.unify(v, vt, from, binds)
                }
                _ => Ok(()),
            },
            // a `T` fits `?T` and `T!E` (R31)
            Pat::Opt(q) => match t {
                Ty::Opt(i) => self.unify(q, self.opts[i as usize], from, binds),
                t => self.unify(q, t, from, binds),
            },
            Pat::Res(v, e) => match t {
                Ty::Res(i) => {
                    let (vt, et) = self.ress[i as usize];
                    self.unify(v, vt, from, binds)?;
                    self.unify(e, et, from, binds)
                }
                t => self.unify(v, t, from, binds),
            },
            Pat::Func(r, ps) => match t {
                Ty::Func(i) => {
                    let (rt, pts) = self.ftys[i as usize].clone();
                    self.unify(r, rt, from, binds)?;
                    for (q, (pt, _)) in ps.iter().zip(pts) {
                        self.unify(q, pt, from, binds)?;
                    }
                    Ok(())
                }
                _ => Ok(()),
            },
            Pat::App(name, qs) => match t {
                Ty::Named(id) if generics::base_of(self.types[id as usize].key) == name => {
                    let key = self.types[id as usize].key;
                    for (q, a) in qs.iter().zip(key_args(key)) {
                        let at = self.ty_text(a);
                        self.unify(q, at, from, binds)?;
                    }
                    Ok(())
                }
                _ => Ok(()),
            },
        }
    }

    /// `(parameter, argument)` pairs of a call by position and name.
    fn arg_pairs(params: &[Param], args: &'a [Expr]) -> Vec<(usize, &'a Expr)> {
        let mut out = Vec::new();
        for (k, a) in args.iter().enumerate() {
            match &a.kind {
                ExprKind::Named(n, v) => {
                    if let Some(j) = params.iter().position(|p| p.name == *n) {
                        out.push((j, &**v));
                    }
                }
                _ if k < params.len() => out.push((k, a)),
                _ => {}
            }
        }
        out
    }

    /// Infers the type parameters from the arguments: `want[i]` says whether parameter `i`
    /// can be inferred at all (it is written in some parameter type). Reports a conflict.
    fn infer(
        &mut self,
        tps: &[TParam],
        pats: &[Pat],
        pairs: &[(usize, &'a Expr)],
        names: &[&str],
        binds: &mut Binds,
    ) -> bool {
        for (j, a) in pairs {
            let Some(t) = self.peek_ty(a) else { continue };
            if let Err((i, b, from)) = self.unify(&pats[*j], t, *j, binds) {
                let msg = format!(
                    "`{}` is {} from `{}`, found {}",
                    tps[i].name,
                    self.tn(b),
                    names[from],
                    self.tn(t)
                );
                self.err("R27.infer_conflict", strip(a).pos, msg, None);
                return false;
            }
        }
        true
    }

    /// The source of the type arguments `[...]` after a name, as the user wrote them.
    fn targs_span(&self, base: &Expr, app: &Expr) -> &'a str {
        self.span(base.end, app.end)
    }

    /// A call of a generic function: `max(3, 5)`, `decode[Config](s)`.
    pub(super) fn generic_call(
        &mut self,
        e: &'a Expr,
        f: &'a Expr,
        name: &'a str,
        targs: Option<&'a [TypeRef]>,
        args: &'a [Expr],
        place: Ty,
    ) -> (Ty, Val) {
        let Some(&g) = self.gen_funcs.get(name) else {
            return (Ty::Err, Val::Runtime);
        };
        let names: Vec<&str> = g.tparams.iter().map(|p| p.name.as_str()).collect();
        let pnames: Vec<&str> = g.params.iter().map(|p| p.name.as_str()).collect();
        let pats: Vec<Pat> = g.params.iter().map(|p| pat(&p.ty.name, &names)).collect();
        let ret = pat(&g.ret.name, &names);
        let inferable: Vec<bool> = (0..names.len())
            .map(|i| pats.iter().any(|p| mentions(p, i)))
            .collect();
        let need: Vec<usize> = (0..names.len()).filter(|i| !inferable[*i]).collect();
        let base = match &f.kind {
            ExprKind::TypeApp(b, _) => &**b,
            _ => f,
        };
        let n = names.len();
        // what the arguments tell
        let mut binds: Binds = vec![None; n];
        let pairs = Self::arg_pairs(&g.params, args);
        if !self.infer(&g.tparams, &pats, &pairs, &pnames, &mut binds) {
            return (Ty::Err, Val::Runtime);
        }
        let told: Vec<bool> = binds.iter().map(|b| b.is_some()).collect();
        // the types written in `[…]`
        if let Some(ts) = targs {
            let tys: Vec<Ty> = ts.iter().map(|t| self.ty(t, false)).collect();
            if tys.len() == n && need.len() != n {
                // every parameter written: the ones the arguments tell must not be (R27)
                if let Some(i) = (0..n).find(|i| told[*i]) {
                    let shown = self.targs_span(base, f);
                    let msg = format!(
                        "`{}` is inferred from the arguments; remove `{shown}`",
                        names[i]
                    );
                    let rest: Vec<&str> = (0..n)
                        .filter(|k| !told[*k])
                        .map(|k| {
                            let (p, len) = (ts[k].pos.offset, ts[k].name.len() as u32);
                            self.span(p, p + len)
                        })
                        .collect();
                    let callee = self.span(f.pos.offset, base.end);
                    let tail = self.span(f.end, e.end);
                    let list = if rest.is_empty() {
                        String::new()
                    } else {
                        format!("[{}]", rest.join(", "))
                    };
                    let hint = format!("{callee}{list}{tail}");
                    let at = self.pos_at(base.end);
                    let d = Diag::new("R27.type_arg_inferred", at, msg)
                        .hint(hint)
                        .fix(base.end, f.end, list);
                    self.diags.push(d);
                    return (Ty::Err, Val::Runtime);
                }
                for (k, t) in tys.iter().enumerate() {
                    binds[k] = Some((*t, usize::MAX));
                }
            } else if tys.len() == need.len() {
                for (k, t) in need.iter().zip(&tys) {
                    binds[*k] = Some((*t, usize::MAX));
                }
            } else {
                let msg = format!("`{name}` takes {}, found {}", plural(need.len()), tys.len());
                self.err("R27.type_arg_count", base.pos, msg, None);
                return (Ty::Err, Val::Runtime);
            }
            if tys.contains(&Ty::Err) {
                return (Ty::Err, Val::Runtime);
            }
        }
        if let Some(i) = binds.iter().position(|b| b.is_none()) {
            // a place can tell the type; the message shows it
            let mut hint_binds: Binds = vec![None; n];
            if place != Ty::Err {
                let _ = self.unify(&ret, place, 0, &mut hint_binds);
            }
            for (k, b) in binds.iter().enumerate() {
                if b.is_some() {
                    hint_binds[k] = *b;
                }
            }
            // the parameters to write: those no argument tells, or all of them if one is inferable
            let shown_ps: Vec<usize> = if (0..n).any(|k| inferable[k]) {
                (0..n).collect()
            } else {
                need.clone()
            };
            let ts: Vec<String> = shown_ps
                .iter()
                .map(|k| match hint_binds[*k] {
                    Some((t, _)) => self.tn(self.lit_default(t)),
                    None => "…".to_string(),
                })
                .collect();
            let call_args = match (args.first(), args.last()) {
                (Some(a), Some(b)) => self.span(a.pos.offset, b.end),
                _ => "",
            };
            let msg = format!(
                "cannot infer `{}` for `{name}`: it only appears in the return type",
                names[i]
            );
            let hint = format!("write it: {name}[{}]({call_args})", ts.join(", "));
            self.err("R27.type_arg_missing", base.pos, msg, Some(hint));
            return (Ty::Err, Val::Runtime);
        }
        let tys: Vec<Ty> = binds
            .iter()
            .map(|b| self.lit_default(b.map_or(Ty::Err, |b| b.0)))
            .collect();
        for (p, t) in g.tparams.iter().zip(&tys) {
            let ordered = matches!(&p.bound, Some((b, _)) if b == "ordered");
            if ordered && !self.is_ordered(*t) {
                self.ordered_missing(name, &p.name, *t, base.pos);
                return (Ty::Err, Val::Runtime);
            }
        }
        let keys: Vec<String> = tys.iter().map(|t| self.tk(*t)).collect();
        let key = format!("{name}[{}]", keys.join(","));
        let Some(&id) = self.funcs.get(key.as_str()) else {
            // the arguments may still ask for instances of their own
            for a in args {
                self.peek_ty(a);
            }
            let shown: Vec<String> = tys.iter().map(|t| self.tn(*t)).collect();
            let display = format!("{name}[{}]", shown.join(", "));
            self.want(false, name, key, display, keys, base.pos);
            return (Ty::Err, Val::Runtime);
        };
        self.callees.insert((e.pos.offset, e.end), key);
        let ret = self.fns[id].ret;
        if !self.args(e, id, args, Vec::new()) {
            return (Ty::Err, Val::Runtime);
        }
        self.effect_in_default(e, Some(id));
        self.last_callee = Some(id);
        self.last_effect = None;
        (ret, Val::Runtime)
    }

    /// A literal that settled nothing is an `int` or an `f64`.
    fn lit_default(&self, t: Ty) -> Ty {
        match t {
            Ty::IntLit => Ty::Int,
            Ty::FloatLit => Ty::Num(Num::F64),
            t => t,
        }
    }

    /// The generic enum a name or `Tree[int]` stands for.
    fn generic_enum_name(&self, base: &'a Expr) -> Option<&'a str> {
        let b = match &base.kind {
            ExprKind::TypeApp(b, _) => &**b,
            _ => base,
        };
        let ExprKind::Ident(n) = &b.kind else {
            return None;
        };
        let g = self.gen_types.get(n.as_str())?;
        (matches!(g.decl, Decl::Enum(_)) && self.local(n).is_none()).then_some(n.as_str())
    }

    /// `Tree.Leaf(3)` and `Tree[int].Leaf(3)`: a variant of a generic enum.
    pub(super) fn generic_variant(
        &mut self,
        e: &'a Expr,
        base: &'a Expr,
        m: &'a str,
        args: Option<&'a [Expr]>,
        place: Ty,
    ) -> Option<(Ty, Val)> {
        let name = self.generic_enum_name(base)?;
        let g = &self.gen_types[name];
        let (Decl::Enum(en), tps) = (g.decl, g.tparams) else {
            return None;
        };
        let names: Vec<&str> = tps.iter().map(|p| p.name.as_str()).collect();
        let Some(v) = en.variants.iter().find(|v| v.name == m) else {
            let vs: Vec<&str> = en.variants.iter().map(|v| v.name.as_str()).collect();
            let hint = closest(m, vs.into_iter()).map(|c| format!("did you mean {name}.{c}?"));
            let msg = format!("`{name}` has no variant `{m}`");
            self.err("R4.no_variant", e.pos, msg, hint);
            return Some((Ty::Err, Val::Runtime));
        };
        let pats: Vec<Pat> = v.fields.iter().map(|p| pat(&p.ty.name, &names)).collect();
        let inferable: Vec<bool> = (0..names.len())
            .map(|i| pats.iter().any(|p| mentions(p, i)))
            .collect();
        let (ename, bspan) = (name, (base.pos.offset, base.end));
        let mut binds: Binds = vec![None; names.len()];
        let tys: Vec<Ty> = if let ExprKind::TypeApp(b, ts) = &base.kind {
            let tys: Vec<Ty> = ts.iter().map(|t| self.ty(t, false)).collect();
            if tys.len() != names.len() {
                let msg = format!(
                    "`{ename}` takes {}, found {}",
                    plural(names.len()),
                    tys.len()
                );
                self.err("R27.type_arg_count", b.pos, msg, None);
                return Some((Ty::Err, Val::Runtime));
            }
            if args.is_some() && inferable.iter().all(|x| *x) {
                let shown = self.targs_span(b, base);
                let msg = format!(
                    "`{}` is inferred from the values; remove `{shown}`",
                    names[0]
                );
                let hint = format!("{ename}{}", self.span(base.end, e.end));
                let at = self.pos_at(b.end);
                let d = Diag::new("R27.type_arg_inferred_variant", at, msg)
                    .hint(hint)
                    .fix(b.end, base.end, "");
                self.diags.push(d);
                return Some((Ty::Err, Val::Runtime));
            }
            tys
        } else {
            let pnames: Vec<&str> = v.fields.iter().map(|p| p.name.as_str()).collect();
            if let Some(args) = args {
                let pairs: Vec<(usize, &'a Expr)> = args
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| *k < pats.len())
                    .collect();
                if !self.infer(tps, &pats, &pairs, &pnames, &mut binds) {
                    return Some((Ty::Err, Val::Runtime));
                }
            }
            if let Some(i) = binds.iter().position(|b| b.is_none()) {
                let mut hint_binds: Binds = vec![None; names.len()];
                if let Ty::Named(id) = self.peeled(place) {
                    if generics::base_of(self.types[id as usize].key) == ename {
                        let key = self.types[id as usize].key;
                        for (k, a) in key_args(key).into_iter().enumerate().take(names.len()) {
                            let t = self.ty_text(a);
                            hint_binds[k] = Some((t, 0));
                        }
                    }
                }
                let ts: Vec<String> = hint_binds
                    .iter()
                    .map(|b| b.map_or("…".to_string(), |b| self.tn(b.0)))
                    .collect();
                let call = args.map_or(String::new(), |_| {
                    self.span(e.pos.offset, e.end)
                        [self.span(e.pos.offset, e.end).find('(').unwrap_or(0)..]
                        .to_string()
                });
                let msg = format!(
                    "cannot infer `{}` for `{ename}.{m}`: it only appears in the return type",
                    names[i]
                );
                let hint = format!("write it: {ename}[{}].{m}{call}", ts.join(", "));
                self.err("R27.type_arg_missing", base.pos, msg, Some(hint));
                return Some((Ty::Err, Val::Runtime));
            }
            binds
                .iter()
                .map(|b| self.lit_default(b.map_or(Ty::Err, |b| b.0)))
                .collect()
        };
        let raw = format!(
            "{ename}[{}]",
            tys.iter()
                .map(|t| self.tk(*t))
                .collect::<Vec<_>>()
                .join(",")
        );
        let t = self.generic_type(ename, tys, &raw, base.pos);
        let Ty::Named(eid) = t else {
            if let Some(args) = args {
                for a in args {
                    self.peek_ty(a);
                }
            }
            return Some((Ty::Err, Val::Runtime));
        };
        self.enum_bases.insert(bspan, eid);
        Some(self.variant(e, eid, m, args))
    }

    /// The type a struct literal builds: the place says which instance of a generic struct.
    pub(super) fn generic_struct_place(
        &mut self,
        e: &Expr,
        name: &str,
        inits: &'a [FieldInit],
        place: Ty,
    ) -> Option<u32> {
        if let Ty::Named(id) = self.peeled(place) {
            if generics::base_of(self.types[id as usize].key) == name
                && self.types[id as usize].key != name
            {
                return Some(id);
            }
        }
        let msg = format!("the type arguments of `{name}` come from the place on the left");
        // the values tell which types the place would write
        let g = self.gen_types.get(name)?;
        let (decl, tps) = (g.decl, g.tparams);
        let names: Vec<&str> = tps.iter().map(|p| p.name.as_str()).collect();
        let mut binds: Binds = vec![None; names.len()];
        if let Decl::Struct(s) = decl {
            for i in inits {
                let Some(k) = s.fields.iter().position(|f| f.name == i.name) else {
                    continue;
                };
                let p = pat(&s.fields[k].ty.name, &names);
                if let Some(t) = self.peek_ty(&i.value) {
                    let _ = self.unify(&p, t, k, &mut binds);
                }
            }
        }
        let ts: Vec<String> = binds
            .iter()
            .map(|b| b.map_or("…".to_string(), |b| self.tn(self.lit_default(b.0))))
            .collect();
        let hint = format!(
            "store it in a variable first: {name}[{}] p = {name}{{…}}",
            ts.join(", ")
        );
        self.err("R27.struct_needs_type", e.pos, msg, Some(hint));
        None
    }

    /// `T` is a generic enum named in a pattern (`Tree.Leaf(x)`): the type of the `switch`.
    pub(super) fn generic_enum_of(&self, name: &str, over: Ty) -> Option<u32> {
        match over {
            Ty::Named(id) if generics::base_of(self.types[id as usize].key) == name => Some(id),
            _ => None,
        }
    }

    /// `declare it: …`: the declaration of the opaque type `t` with `ordered` added (R28).
    pub(super) fn ordered_needed(&mut self, t: Ty, what: &str, pos: Pos) {
        let Ty::Named(id) = t else { return };
        let info = &self.types[id as usize];
        let (shown, key, tpos) = (info.name, info.key, info.pos);
        let owner = key.trim_start_matches('$').split('$').next().unwrap_or("");
        let decl: Option<&Decl> = match self.gen_funcs.get(owner) {
            Some(&f) => self
                .templates
                .iter()
                .find(|d| matches!(d, Decl::Func(g) if std::ptr::eq(g, f))),
            None => self.gen_types.get(owner).map(|g| g.decl),
        };
        let hint = decl.map(|d| format!("declare it: {}", self.header_text(d, Some(shown))));
        let msg = format!("`{what}` needs `{shown}` to be ordered");
        let mut d = Diag::new("R28.ordered_needed", pos, msg);
        d.hint = hint;
        let d = d.fix(tpos.offset, tpos.offset, "ordered ");
        self.diags.push(d);
    }
}

impl<'a> Checker<'a> {
    /// The generic declaration a function or method of an instance was copied from; the
    /// function itself if it is no copy.
    pub(super) fn template_of(&self, f: &'a Func) -> &'a Func {
        match &f.recv {
            Some(r) if r.name.contains('[') => {
                let base = generics::base_of(&r.name);
                let found = self
                    .gen_types
                    .get(base)
                    .and_then(|g| g.methods.iter().find(|m| m.name == f.name));
                found.copied().unwrap_or(f)
            }
            None if f.name.contains('[') => self
                .gen_funcs
                .get(generics::base_of(&f.name))
                .copied()
                .unwrap_or(f),
            _ => f,
        }
    }
}

impl<'a> Checker<'a> {
    /// A method takes the type parameters of its type and has none of its own (R27).
    pub(super) fn method_tparams(&mut self, m: &Func) {
        if let Some(p) = m.tparams.first() {
            let msg = "a method has no type parameters of its own";
            let hint = "write a function with the type parameter instead".to_string();
            self.err("R27.method_type_params", p.pos, msg, Some(hint));
        }
    }
}
