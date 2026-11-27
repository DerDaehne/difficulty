//! Part 2c of the checker: `use std.…`, the types of collections and number widths,
//! literals, index, conversions, builtin methods and `for` over collections and
//! results (R2, R5, R8, R11, R14, R16, R28–R31, §4).

use super::*;

/// The standard modules (R16).
pub const STD_MODULES: &[&str] = &["io", "os"];

/// Signature of a builtin method or function (§4).
struct Sig {
    params: Vec<Ty>,
    ret: Ty,
    /// The method changes `self`: it needs a place before the dot (R7).
    changes: bool,
}

fn sig(params: &[Ty], ret: Ty, changes: bool) -> Sig {
    Sig {
        params: params.to_vec(),
        ret,
        changes,
    }
}

const STR_METHODS: &[&str] = &[
    "len",
    "is_empty",
    "runes",
    "split",
    "lines",
    "trim",
    "trim_start",
    "trim_end",
    "lower",
    "upper",
    "contains",
    "starts_with",
    "ends_with",
    "find",
    "replace",
    "repeat",
    "slice",
    "parse_int",
    "parse_f64",
];
const RUNE_METHODS: &[&str] = &["is_digit", "is_letter", "is_space"];
const INT_METHODS: &[&str] = &[
    "abs", "min", "max", "to_int", "to_i8", "to_i16", "to_i32", "to_u8", "to_u16", "to_u32",
    "to_u64",
];
const F64_METHODS: &[&str] = &[
    "round", "trunc", "floor", "ceil", "sqrt", "abs", "min", "max",
];
const LIST_METHODS: &[&str] = &[
    "len", "is_empty", "push", "pop", "insert", "remove", "first", "last", "contains", "index_of",
    "count", "sort", "reverse", "swap", "slice", "join", "filter", "map", "sort_by", "any", "all",
];
const ARRAY_METHODS: &[&str] = &[
    "len", "is_empty", "first", "last", "contains", "index_of", "count", "sort", "reverse", "swap",
    "slice", "join", "filter", "map", "sort_by", "any", "all",
];
const MAP_METHODS: &[&str] = &["len", "has", "remove", "add", "keys", "values"];
/// Functions of `io` and `os`.
const IO_FUNCS: &[&str] = &[
    "lines",
    "read_line",
    "read_file",
    "write_file",
    "read_lines",
    "write_lines",
    "eprint",
];
const OS_FUNCS: &[&str] = &["args", "exit", "env"];

impl<'a> Checker<'a> {
    // ---- use std.… (R16) ----

    pub(super) fn use_decl(&mut self, u: &'a UseDecl, after_other: bool) {
        self.uses_2c = true;
        let module = u.module.as_str();
        let Some(&known) = STD_MODULES.iter().find(|m| **m == module) else {
            let msg = format!("unknown standard module `{module}`");
            self.err(
                "R16.unknown_module",
                u.name_pos,
                msg,
                Some("available: io, os".into()),
            );
            return;
        };
        if self.imports.contains(&known) {
            if after_other {
                let hint = "move it to the top of the file".to_string();
                self.err(
                    "R16.use_order",
                    u.pos,
                    "`use` comes before all other declarations",
                    Some(hint),
                );
            } else {
                let msg = format!("`std.{known}` is already used");
                self.err("R16.use_twice", u.pos, msg, Some("remove this line".into()));
            }
            return;
        }
        if after_other {
            let hint = "move it to the top of the file".to_string();
            self.err(
                "R16.use_order",
                u.pos,
                "`use` comes before all other declarations",
                Some(hint),
            );
        }
        self.imports.push(known);
        if known == "io" {
            let strs = |names: &[&'static str]| names.iter().map(|n| (Ty::Str, *n)).collect();
            let variants = vec![
                VariantInfo {
                    name: "NotFound",
                    fields: strs(&["path"]),
                },
                VariantInfo {
                    name: "NoPermission",
                    fields: strs(&["path"]),
                },
                VariantInfo {
                    name: "InvalidUtf8",
                    fields: strs(&["path"]),
                },
                VariantInfo {
                    name: "Other",
                    fields: strs(&["path", "reason"]),
                },
            ];
            let id = self.types.len() as u32;
            self.type_ids.insert("io.IoErr", id);
            self.types.push(TypeInfo {
                name: "io.IoErr",
                pos: u.pos,
                kind: Kind::Enum(variants),
                methods: HashMap::new(),
            });
        }
    }

    /// `io` or `os` as a name that is a module here (imported, not shadowed).
    pub(super) fn module_name(&self, e: &Expr) -> Option<&'static str> {
        let ExprKind::Ident(n) = &e.kind else {
            return None;
        };
        let m = STD_MODULES.iter().find(|m| **m == n.as_str())?;
        (self.local(n).is_none() && self.imports.contains(m)).then_some(*m)
    }

    /// The enum a name before `.` stands for: `E`, or `io.IoErr`.
    pub(super) fn enum_of(&self, base: &Expr) -> Option<u32> {
        match &base.kind {
            ExprKind::Ident(tn) if self.local(tn).is_none() => self.enum_id(tn),
            ExprKind::Field(m, n) => {
                let module = self.module_name(m)?;
                self.enum_id(&format!("{module}.{n}"))
            }
            _ => None,
        }
    }

    // ---- types (R2) ----

    /// A written type (`T`, `?T`, `T!E`, `[]T`, `[N]T`, `map[K]V`, R2); `ret` allows
    /// `void` as `T` or `E`.
    pub(super) fn ty(&mut self, t: &TypeRef, ret: bool) -> Ty {
        self.ty_with(t, ret, false)
    }

    /// A parameter type: a function type `R(T1, mut T2)` is allowed here only (R26).
    pub(super) fn param_ty(&mut self, t: &TypeRef) -> Ty {
        self.ty_with(t, false, true)
    }

    fn ty_with(&mut self, t: &TypeRef, ret: bool, fn_ok: bool) -> Ty {
        if t.name == "?" {
            return Ty::Err;
        }
        let mut s = t.name.as_str();
        self.fn_ok = fn_ok;
        let r = self.ty_top(&mut s, t.pos, ret, true);
        self.fn_ok = false;
        if self.is_2c(r) {
            self.uses_2c = true;
        }
        r
    }

    fn ty_top(&mut self, s: &mut &str, pos: Pos, ret: bool, top: bool) -> Ty {
        let opt = s.starts_with('?');
        if opt {
            *s = &s[1..];
        }
        // `void(…)` is a function type: `void` is fine there, and only there
        let ret0 = ret;
        let ret = ret || (top && self.fn_ok);
        let mut ty = self.ty_base(s, pos, ret);
        if opt {
            ty = self.opt_of(ty, pos);
        }
        if let Some(r) = s.strip_prefix('!') {
            *s = r;
            let et = self.ty_base(s, pos, ret);
            ty = self.res(ty, et);
        }
        if s.starts_with('(') {
            *s = &s[1..];
            let allowed = top && self.fn_ok;
            let mut ps = Vec::new();
            while !s.is_empty() && !s.starts_with(')') {
                let m = s.strip_prefix("mut ");
                if let Some(r) = m {
                    *s = r;
                }
                let t = self.ty_top(s, pos, false, false);
                ps.push((t, m.is_some()));
                *s = s.strip_prefix(',').unwrap_or(s);
            }
            *s = s.strip_prefix(')').unwrap_or(s);
            if !allowed {
                let msg = "a function type is only allowed for a parameter";
                self.err("R26.function_type_place", pos, msg, None);
                return Ty::Err;
            }
            return self.func_ty(ty, ps);
        }
        if ty == Ty::Void && !ret0 {
            self.err(
                "R8.void_not_type",
                pos,
                "`void` is only a return type",
                None,
            );
            return Ty::Err;
        }
        ty
    }

    fn opt_of(&mut self, ty: Ty, pos: Pos) -> Ty {
        if ty == Ty::Void {
            let msg = "?void has no value that could be missing";
            self.err("R12.opt_void", pos, msg, Some("use void!E".into()));
            return Ty::Err;
        }
        self.opt(ty)
    }

    fn ty_elem(&mut self, s: &mut &str, pos: Pos) -> Ty {
        if let Some(r) = s.strip_prefix('(') {
            *s = r;
            let t = self.ty_top(s, pos, false, false);
            *s = s.strip_prefix(')').unwrap_or(s);
            return t;
        }
        let opt = s.starts_with('?');
        if opt {
            *s = &s[1..];
        }
        let t = self.ty_base(s, pos, false);
        if opt {
            self.opt_of(t, pos)
        } else {
            t
        }
    }

    fn ty_base(&mut self, s: &mut &str, pos: Pos, ret: bool) -> Ty {
        if let Some(r) = s.strip_prefix('[') {
            let end = r.find(']').unwrap_or(r.len());
            let len = &r[..end];
            *s = r.get(end + 1..).unwrap_or("");
            let el = self.ty_elem(s, pos);
            if len.is_empty() {
                return self.list(el);
            }
            return match self.array_len(len, pos) {
                Some(n) => self.arr(el, n),
                None => Ty::Err,
            };
        }
        if let Some(r) = s.strip_prefix("map[") {
            *s = r;
            let k = self.ty_elem(s, pos);
            *s = s.strip_prefix(']').unwrap_or(s);
            let v = self.ty_elem(s, pos);
            return self.map(k, v);
        }
        let end = s
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
            .unwrap_or(s.len());
        let (n, rest) = s.split_at(end);
        *s = rest;
        match n {
            "int" => Ty::Int,
            "bool" => Ty::Bool,
            "str" => Ty::Str,
            "rune" => Ty::Rune,
            "void" if ret => Ty::Void,
            "void" => {
                self.err(
                    "R8.void_not_type",
                    pos,
                    "`void` is only a return type",
                    None,
                );
                Ty::Err
            }
            "f32" => {
                self.err(
                    "S1.f32_unavailable",
                    pos,
                    "`f32` is not available in v0",
                    Some("use f64".into()),
                );
                Ty::Err
            }
            n if Num::from_name(n).is_some() => Ty::Num(Num::from_name(n).unwrap_or(Num::F64)),
            n => match self.type_ids.get(n) {
                Some(&id) => Ty::Named(id),
                None => {
                    if let Some((m, _)) = n.split_once('.') {
                        if STD_MODULES.contains(&m) && !self.imports.contains(&m) {
                            self.not_imported(m, pos);
                            return Ty::Err;
                        }
                    }
                    let prims = ["int", "bool", "str", "rune", "f64", "u8", "i32"];
                    let names = prims.into_iter().chain(self.type_ids.keys().copied());
                    let hint = closest(n, names).map(|c| format!("did you mean {c}?"));
                    self.err("R4.unknown_type", pos, format!("unknown type `{n}`"), hint);
                    Ty::Err
                }
            },
        }
    }

    /// The length of `[N]T`: a literal or a `const int` of at least 1 (R5).
    fn array_len(&mut self, text: &str, pos: Pos) -> Option<u64> {
        let n = match text.parse::<u64>() {
            Ok(n) => Some(n as i128),
            Err(_) => match self.local(text) {
                Some(l) if l.is_const && l.ty == Ty::Int => match l.val {
                    Val::Int(n) => Some(n),
                    _ => None,
                },
                _ => self.const_int(text, 0),
            },
        };
        match n.filter(|&n| (1..=i64::MAX as i128).contains(&n)) {
            Some(n) => Some(n as u64),
            None => {
                let msg = "the length of an array is a constant of at least 1";
                self.err("R5.array_length", pos, msg, None);
                None
            }
        }
    }

    /// The value of a global `const int` made of literals, for array lengths.
    fn const_int(&self, name: &str, depth: u32) -> Option<i128> {
        let v = self.m.decls.iter().find_map(|d| match d {
            Decl::Const(v) if v.name == name && v.ty.name == "int" => Some(v),
            _ => None,
        })?;
        self.lit_int(&v.value, depth)
    }

    fn lit_int(&self, e: &Expr, depth: u32) -> Option<i128> {
        if depth > 16 {
            return None;
        }
        match &e.kind {
            ExprKind::Int(v) => Some(*v as i128),
            ExprKind::Paren(x) => self.lit_int(x, depth + 1),
            ExprKind::Unary("-", x) => self.lit_int(x, depth + 1).map(|v| -v),
            ExprKind::Ident(n) => self.const_int(n, depth + 1),
            ExprKind::Binary(op, l, r, _) => {
                let (a, b) = (self.lit_int(l, depth + 1)?, self.lit_int(r, depth + 1)?);
                match *op {
                    "+" => a.checked_add(b),
                    "-" => a.checked_sub(b),
                    "*" => a.checked_mul(b),
                    "/" if b != 0 => Some(a / b),
                    "%" if b != 0 => Some(a % b),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(super) fn not_imported(&mut self, m: &str, pos: Pos) {
        if self.unknown.iter().any(|n| n == m) {
            return;
        }
        self.unknown.push(m.to_string());
        let hint = format!("add use std.{m} at the top of the file");
        self.err(
            "R16.not_imported",
            pos,
            format!("`{m}` is not imported"),
            Some(hint),
        );
    }

    // ---- literals and their types (R8) ----

    /// The number type a literal at place `want` takes: through `?` and `!E`.
    pub(super) fn peeled(&self, mut w: Ty) -> Ty {
        while let Some(i) = self.inner(w) {
            w = i;
        }
        w
    }

    /// `t` is a literal expression: give it the type of its place (R8).
    pub(super) fn adopt(&mut self, want: Ty, t: Ty, v: Val, e: &Expr) -> (Ty, Val) {
        let target = self.peeled(want);
        let (ty, v) = match (t, v) {
            (Ty::IntLit, Val::Int(n)) if target.is_float() => {
                (Ty::Num(Num::F64), Val::Float(n as f64))
            }
            (Ty::IntLit, Val::Int(n)) => {
                let ty = if target.is_int() { target } else { Ty::Int };
                if !self.lit_fits(ty, n, e) {
                    return (Ty::Err, Val::Runtime);
                }
                (ty, Val::Int(n))
            }
            (Ty::FloatLit, v) => (Ty::Num(Num::F64), v),
            (t, v) => (t, v),
        };
        self.exprs.insert((e.pos.offset, e.end), ty);
        (ty, v)
    }

    /// Does the constant `n` fit the integer type `ty`? Reports R8 if not.
    pub(super) fn lit_fits(&mut self, ty: Ty, n: i128, e: &Expr) -> bool {
        let Some((lo, hi)) = ty.range() else {
            return true;
        };
        if n < lo || n > hi {
            if ty == Ty::Int || ty == Ty::IntLit {
                self.out_of_range(e.pos);
            } else {
                let msg = format!("expected {}, found {n}", self.tn(ty));
                let hint = format!("the value does not fit in {}", self.tn(ty));
                self.err("R8.type_mismatch", e.pos, msg, Some(hint));
            }
            return false;
        }
        self.folded.insert((e.pos.offset, e.end), n as i64);
        true
    }

    /// A value read where its type is settled: a literal without place becomes `int` or
    /// `f64`, a typed constant must fit its type (R8). `want` is the type of the place.
    pub(super) fn finish(&mut self, t: Ty, v: Val, e: &Expr, want: Option<Ty>) -> (Ty, Val) {
        match (t, v) {
            (Ty::IntLit | Ty::FloatLit, _) => self.adopt(want.unwrap_or(Ty::Int), t, v, e),
            (_, Val::Int(n)) if t.range().is_some() => {
                if !self.lit_fits(t, n, e) {
                    return (Ty::Err, Val::Runtime);
                }
                (t, v)
            }
            _ => (t, v),
        }
    }

    /// The read of a value for a place `want`: literals take its type.
    pub(super) fn settle(&mut self, want: Ty, t: Ty, v: Val, e: &Expr) -> (Ty, Val) {
        let (t, v) = self.finish(t, v, e, Some(want));
        self.expect(want, t, e);
        (t, v)
    }

    /// The hint for a value of the wrong number type: the conversion (R31).
    pub(super) fn conv_hint(&self, want: Ty, got: Ty, e: &Expr) -> Option<String> {
        let ok = (want.is_int() && (got.is_number() || got == Ty::Rune))
            || (want.is_float() && got.is_int())
            || (want == Ty::Rune && got.is_int());
        ok.then(|| format!("{}({})", self.tn(want), self.text(e)))
    }

    /// `f64` text form (R29): shortest digits, no exponent from 1e-6 to below 1e21.
    pub(super) fn f64_text(x: f64) -> String {
        if x.is_nan() {
            return "NaN".into();
        }
        if x.is_infinite() {
            return if x < 0.0 { "-inf" } else { "inf" }.into();
        }
        if x == 0.0 {
            return if x.is_sign_negative() { "-0" } else { "0" }.into();
        }
        let sign = if x < 0.0 { "-" } else { "" };
        let s = format!("{:e}", x.abs());
        let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
        let exp: i32 = exp.parse().unwrap_or(0);
        let digits: String = mant.chars().filter(|c| *c != '.').collect();
        if !(-6..=20).contains(&exp) {
            return format!("{sign}{mant}e{exp}");
        }
        if exp >= 0 {
            let int_len = exp as usize + 1;
            let padded = format!("{digits:0<int_len$}");
            let (a, b) = padded.split_at(int_len);
            if b.is_empty() {
                format!("{sign}{a}")
            } else {
                format!("{sign}{a}.{b}")
            }
        } else {
            format!("{sign}0.{}{digits}", "0".repeat((-exp - 1) as usize))
        }
    }

    pub(super) fn float_lit(&mut self, e: &Expr, text: &str) -> (Ty, Val) {
        self.uses_2c = true;
        let f: f64 = text.parse().unwrap_or(0.0);
        let mantissa = text.split(['e', 'E']).next().unwrap_or("");
        let nonzero = mantissa.chars().any(|c| ('1'..='9').contains(&c));
        if f.is_infinite() || (f == 0.0 && nonzero) {
            let msg = format!("expected f64, found {text}");
            let hint = "the value does not fit in f64".to_string();
            self.err("R8.type_mismatch", e.pos, msg, Some(hint));
            return (Ty::Err, Val::Runtime);
        }
        (Ty::FloatLit, Val::Float(f))
    }

    pub(super) fn list_lit(&mut self, e: &'a Expr, items: &'a [Expr], place: Ty) -> (Ty, Val) {
        self.uses_2c = true;
        let known = |vs: &[(Ty, Val)]| {
            if vs.iter().all(|(_, v)| *v != Val::Runtime) {
                Val::Known
            } else {
                Val::Runtime
            }
        };
        match place {
            Ty::List(i) => {
                let t = self.lists[i as usize];
                let vs: Vec<_> = items.iter().map(|x| self.at_place(t, x, None)).collect();
                (place, known(&vs))
            }
            Ty::Arr(i) => {
                let (t, n) = self.arrs[i as usize];
                if items.len() as u64 != n {
                    let msg = format!(
                        "{} needs {n} elements, found {}",
                        self.tn(place),
                        items.len()
                    );
                    let hint = format!(
                        "use []{} for a list that can have any length",
                        self.tn_elem(t)
                    );
                    self.err("R8.array_length_mismatch", e.pos, msg, Some(hint));
                    return (Ty::Err, Val::Runtime);
                }
                let vs: Vec<_> = items.iter().map(|x| self.at_place(t, x, None)).collect();
                (place, known(&vs))
            }
            _ if items.is_empty() => {
                let msg = "`[]` needs a place of type []T";
                self.err("R8.empty_list_place", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
            _ => {
                let (t, vs) = self.common(items);
                (self.list(t), known(&vs))
            }
        }
    }

    /// Items without a place get one type, whatever their order (R5, Teil 2c): the type
    /// of the first that is not a literal, else `f64` if one is a float literal, else `int`.
    fn common(&mut self, items: &'a [Expr]) -> (Ty, Vec<(Ty, Val)>) {
        let raw: Vec<(Ty, Val)> = items.iter().map(|x| self.expr(x)).collect();
        let concrete = raw
            .iter()
            .map(|r| r.0)
            .find(|t| !t.is_lit() && *t != Ty::Err);
        let t = match concrete {
            Some(t) => t,
            None if raw.iter().any(|r| r.0 == Ty::FloatLit) => Ty::Num(Num::F64),
            None if raw.iter().any(|r| r.0 == Ty::IntLit) => Ty::Int,
            None => return (Ty::Err, raw),
        };
        let vs = items
            .iter()
            .zip(raw)
            .map(|(x, (xt, xv))| self.settle(t, xt, xv, x))
            .collect();
        (t, vs)
    }

    /// A key that is a literal or a `const`, for the duplicate check (R30).
    fn key_text(&self, k: &Expr, v: Val) -> Option<String> {
        match (&k.kind, v) {
            (ExprKind::Str(segs), _) => {
                let mut s = String::from("s");
                for seg in segs {
                    match seg {
                        StrSeg::Lit(t) => s.push_str(t),
                        _ => return None,
                    }
                }
                Some(s)
            }
            (_, Val::Int(n)) => Some(format!("n{n}")),
            (ExprKind::Bool(b), _) => Some(format!("b{b}")),
            _ => None,
        }
    }

    pub(super) fn map_lit(
        &mut self,
        e: &'a Expr,
        entries: &'a [(Expr, Expr)],
        place: Ty,
    ) -> (Ty, Val) {
        self.uses_2c = true;
        let (kt, vt, ty) = match place {
            Ty::Map(i) => {
                let (k, v) = self.maps[i as usize];
                (Some(k), Some(v), Some(place))
            }
            _ if entries.is_empty() => {
                let msg = "`{}` needs a place of type map[K]V";
                self.err("R8.empty_map_place", e.pos, msg, None);
                return (Ty::Err, Val::Runtime);
            }
            _ => (None, None, None),
        };
        let mut known = true;
        let mut seen: Vec<(String, Pos)> = Vec::new();
        let dup = |c: &mut Self, k: &Expr, v: Val, seen: &mut Vec<(String, Pos)>| {
            let Some(key) = c.key_text(k, v) else { return };
            match seen.iter().find(|(s, _)| *s == key) {
                Some((_, first)) => {
                    let msg = format!("duplicate key {}", c.text(k));
                    let hint = format!("first used at {}", c.at(*first));
                    c.err("R30.duplicate_key", k.pos, msg, Some(hint));
                }
                None => seen.push((key, k.pos)),
            }
        };
        let ty = match (kt, vt, ty) {
            (Some(kt), Some(vt), Some(ty)) => {
                for (k, v) in entries {
                    let (_, kv) = self.at_place(kt, k, None);
                    let (_, vv) = self.at_place(vt, v, None);
                    dup(self, k, kv, &mut seen);
                    known &= kv != Val::Runtime && vv != Val::Runtime;
                }
                ty
            }
            _ => {
                let keys: Vec<(Ty, Val)> = entries.iter().map(|(k, _)| self.expr(k)).collect();
                let vals: Vec<(Ty, Val)> = entries.iter().map(|(_, v)| self.expr(v)).collect();
                let pick = |raw: &[(Ty, Val)]| {
                    let concrete = raw
                        .iter()
                        .map(|r| r.0)
                        .find(|t| !t.is_lit() && *t != Ty::Err);
                    match concrete {
                        Some(t) => t,
                        None if raw.iter().any(|r| r.0 == Ty::FloatLit) => Ty::Num(Num::F64),
                        None if raw.iter().any(|r| r.0 == Ty::IntLit) => Ty::Int,
                        None => Ty::Err,
                    }
                };
                let (kt, vt) = (pick(&keys), pick(&vals));
                for ((k, v), ((ktt, kvv), (vtt, vvv))) in
                    entries.iter().zip(keys.into_iter().zip(vals))
                {
                    let (_, kv) = self.settle(kt, ktt, kvv, k);
                    let (_, vv) = self.settle(vt, vtt, vvv, v);
                    dup(self, k, kv, &mut seen);
                    known &= kv != Val::Runtime && vv != Val::Runtime;
                }
                self.map(kt, vt)
            }
        };
        (ty, if known { Val::Known } else { Val::Runtime })
    }

    // ---- index (R5, R6, R30) ----

    /// `base[idx]` with the type of `base` known; `bp` is the position of `[`.
    pub(super) fn index_of(&mut self, e: &'a Expr, bt: Ty, idx: &'a Expr, bp: Pos) -> (Ty, Val) {
        self.uses_2c = true;
        match bt {
            Ty::Err => {
                self.rvalue(idx);
                (Ty::Err, Val::Runtime)
            }
            Ty::List(i) => {
                self.at_place(Ty::Int, idx, None);
                (self.lists[i as usize], Val::Runtime)
            }
            Ty::Arr(i) => {
                let (t, n) = self.arrs[i as usize];
                let (it, iv) = self.at_place(Ty::Int, idx, None);
                if let (Ty::Int, Val::Int(k)) = (it, iv) {
                    if k < 0 || k >= n as i128 {
                        let msg = format!("index {k} out of bounds for length {n}");
                        self.err("R14.index_bounds", bp, msg, None);
                        return (Ty::Err, Val::Runtime);
                    }
                }
                (t, Val::Runtime)
            }
            Ty::Map(i) => {
                let (k, v) = self.maps[i as usize];
                self.at_place(k, idx, None);
                (self.opt(v), Val::Runtime)
            }
            Ty::Str => {
                self.rvalue(idx);
                let hint = "use s.slice(a, b) or s.runes()".to_string();
                self.err("R8.index_str", e.pos, "cannot index str", Some(hint));
                (Ty::Err, Val::Runtime)
            }
            t => {
                self.rvalue(idx);
                let msg = format!("cannot index {}", self.tn(t));
                self.err("R8.index_type", e.pos, msg, None);
                (Ty::Err, Val::Runtime)
            }
        }
    }

    /// Is `e` an entry of a map, `m[k]` or a part of one (`m[k].f`)? Not a place (R30).
    pub(super) fn map_entry(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Index(b, _, _) => {
                matches!(self.exprs.get(&(b.pos.offset, b.end)), Some(Ty::Map(_)))
                    || self.map_entry(b)
            }
            ExprKind::Field(b, _) | ExprKind::Paren(b) => self.map_entry(b),
            _ => false,
        }
    }

    pub(super) fn map_place_error(&mut self, place: &Expr) {
        let msg = "a map entry is changed by assigning it";
        let hint = "read it, change the copy, and store it: m[k] = v";
        self.err("R30.map_entry_change", place.pos, msg, Some(hint.into()));
    }

    // ---- conversions T(x) (R31) ----

    pub(super) fn convert(&mut self, e: &'a Expr, name: &str, args: &'a [Expr]) -> (Ty, Val) {
        self.uses_2c = true;
        let target = match name {
            "int" => Ty::Int,
            "rune" => Ty::Rune,
            "str" => Ty::Str,
            "bool" => Ty::Bool,
            n => Num::from_name(n).map_or(Ty::Err, Ty::Num),
        };
        let user = target == Ty::Err;
        if args.len() != 1 {
            let msg = format!("`{name}` takes 1 argument, found {}", args.len());
            for a in args {
                self.rvalue(a);
            }
            self.err("R8.arg_count", e.pos, msg, None);
            return (Ty::Err, Val::Runtime);
        }
        let a = &args[0];
        let (st, sv) = self.rvalue(a);
        if st == Ty::Err {
            return (Ty::Err, Val::Runtime);
        }
        if st == target {
            return (target, sv);
        }
        let ok = match (target, st) {
            (t, s) if t.is_int() && (s.is_int() || s.is_float() || s == Ty::Rune) => true,
            (t, s) if t.is_float() && s.is_int() => true,
            (Ty::Rune, s) if s.is_int() => true,
            (Ty::Str, Ty::Rune) => true,
            _ => false,
        };
        let tname = if user {
            name.to_string()
        } else {
            self.tn(target)
        };
        if !ok || user {
            let text = self.text(a);
            let hint = match (target, st) {
                (t, Ty::Str) if t.is_int() => Some(format!("use {text}.parse_int()")),
                (t, Ty::Str) if t.is_float() => Some(format!("use {text}.parse_f64()")),
                (Ty::Str, _) => Some(format!("build text with interpolation: \"{{{text}}}\"")),
                (Ty::Bool, _) => Some(format!("compare explicitly: {text} != 0")),
                (_, Ty::Bool) => Some(format!("use if {text} {{ 1 }} else {{ 0 }}")),
                _ => None,
            };
            let msg = format!("cannot convert {} to {tname}", self.tn(st));
            self.err("R31.convert", e.pos, msg, hint);
            return (Ty::Err, Val::Runtime);
        }
        // known at compile time: the conversion is done here, a failure is R14
        let round = "round explicitly first with round(), trunc(), floor() or ceil()";
        let abort = |c: &mut Self, id: &'static str, msg: String, hint: Option<&str>| {
            c.err(id, e.pos, msg, hint.map(String::from));
            (Ty::Err, Val::Runtime)
        };
        match (target, sv) {
            (t, Val::Int(n)) if t.is_int() => {
                let (lo, hi) = t.range().unwrap_or((0, 0));
                if n < lo || n > hi {
                    return abort(
                        self,
                        "R14.conv_range",
                        format!("{n} does not fit in {tname}"),
                        None,
                    );
                }
                (t, Val::Int(n))
            }
            (t, Val::Float(f)) if t.is_int() => {
                let (lo, hi) = t.range().unwrap_or((0, 0));
                let x = Self::f64_text(f);
                if !f.is_finite() || f.fract() != 0.0 {
                    return abort(
                        self,
                        "R14.conv_inexact",
                        format!("{x} is not an exact {tname}"),
                        Some(round),
                    );
                }
                if f.abs() >= 1.0e38 || (f as i128) < lo || (f as i128) > hi {
                    return abort(
                        self,
                        "R14.conv_range",
                        format!("{x} does not fit in {tname}"),
                        Some(round),
                    );
                }
                (t, Val::Int(f as i128))
            }
            (Ty::Num(Num::F64), Val::Int(n)) => {
                if (n as f64) as i128 != n {
                    return abort(
                        self,
                        "R14.conv_inexact",
                        format!("{n} is not an exact f64"),
                        None,
                    );
                }
                (target, Val::Float(n as f64))
            }
            (Ty::Rune, Val::Int(n)) => {
                if !(0..=0x10FFFF).contains(&n) || (0xD800..=0xDFFF).contains(&n) {
                    return abort(
                        self,
                        "R14.conv_rune",
                        format!("{n} is not a valid rune"),
                        None,
                    );
                }
                (target, Val::Int(n))
            }
            (t, v) => (t, if v == Val::Runtime { v } else { Val::Known }),
        }
    }

    // ---- builtin methods (§4) ----

    fn method_names(bt: Ty) -> &'static [&'static str] {
        match bt {
            Ty::Str => STR_METHODS,
            Ty::Rune => RUNE_METHODS,
            Ty::Num(Num::F64) => F64_METHODS,
            t if t.is_int() => INT_METHODS,
            Ty::List(_) => LIST_METHODS,
            Ty::Arr(_) => ARRAY_METHODS,
            Ty::Map(_) => MAP_METHODS,
            _ => &[],
        }
    }

    fn builtin_sig(&mut self, bt: Ty, m: &str) -> Option<Sig> {
        let (int, boolean, st) = (Ty::Int, Ty::Bool, Ty::Str);
        match bt {
            Ty::Str => Some(match m {
                "len" => sig(&[], int, false),
                "is_empty" => sig(&[], boolean, false),
                "runes" => sig(&[], self.list(Ty::Rune), false),
                "split" => sig(&[st], self.list(st), false),
                "lines" => sig(&[], self.list(st), false),
                "trim" | "trim_start" | "trim_end" | "lower" | "upper" => sig(&[], st, false),
                "contains" | "starts_with" | "ends_with" => sig(&[st], boolean, false),
                "find" => sig(&[st], self.opt(int), false),
                "replace" => sig(&[st, st], st, false),
                "repeat" => sig(&[int], st, false),
                "slice" => sig(&[int, int], st, false),
                "parse_int" => sig(&[], self.opt(int), false),
                "parse_f64" => sig(&[], self.opt(Ty::Num(Num::F64)), false),
                _ => return None,
            }),
            Ty::Rune => Some(match m {
                "is_digit" | "is_letter" | "is_space" => sig(&[], boolean, false),
                _ => return None,
            }),
            Ty::Num(Num::F64) => Some(match m {
                "round" | "trunc" | "floor" | "ceil" | "sqrt" | "abs" => sig(&[], bt, false),
                "min" | "max" => sig(&[bt], bt, false),
                _ => return None,
            }),
            t if t.is_int() => {
                if let Some(target) = m.strip_prefix("to_") {
                    let to = if target == "int" {
                        Ty::Int
                    } else {
                        Ty::Num(Num::from_name(target).filter(|n| *n != Num::F64)?)
                    };
                    return Some(sig(&[], self.opt(to), false));
                }
                Some(match m {
                    "abs" if t.signed() => sig(&[], bt, false),
                    "min" | "max" => sig(&[bt], bt, false),
                    _ => return None,
                })
            }
            Ty::List(_) | Ty::Arr(_) => {
                let (el, list) = match bt {
                    Ty::List(i) => (self.lists[i as usize], true),
                    Ty::Arr(i) => (self.arrs[i as usize].0, false),
                    _ => return None,
                };
                Some(match m {
                    "len" => sig(&[], int, false),
                    "is_empty" => sig(&[], boolean, false),
                    "push" if list => sig(&[el], Ty::Void, true),
                    "pop" if list => sig(&[], self.opt(el), true),
                    "insert" if list => sig(&[int, el], Ty::Void, true),
                    "remove" if list => sig(&[int], el, true),
                    "first" | "last" => sig(&[], self.opt(el), false),
                    "contains" => sig(&[el], boolean, false),
                    "index_of" => sig(&[el], self.opt(int), false),
                    "count" => sig(&[el], int, false),
                    "sort" | "reverse" => sig(&[], Ty::Void, true),
                    "swap" => sig(&[int, int], Ty::Void, true),
                    "slice" => sig(&[int, int], self.list(el), false),
                    "join" if el == st => sig(&[st], st, false),
                    "filter" | "any" | "all" => {
                        let f = self.func_ty(boolean, vec![(el, false)]);
                        let r = if m == "filter" {
                            self.list(el)
                        } else {
                            boolean
                        };
                        sig(&[f], r, false)
                    }
                    "sort_by" => {
                        let f = self.func_ty(boolean, vec![(el, false), (el, false)]);
                        sig(&[f], Ty::Void, true)
                    }
                    // the result type comes from the literal: see `builtin_method`
                    "map" => sig(&[], Ty::Err, false),
                    _ => return None,
                })
            }
            Ty::Map(i) => {
                let (k, v) = self.maps[i as usize];
                Some(match m {
                    "len" => sig(&[], int, false),
                    "has" => sig(&[k], boolean, false),
                    "remove" => sig(&[k], boolean, true),
                    "add" => sig(&[k, v], Ty::Void, true),
                    "keys" => sig(&[], self.list(k), false),
                    "values" => sig(&[], self.list(v), false),
                    _ => return None,
                })
            }
            _ => None,
        }
    }

    /// A method of a builtin type (§4).
    pub(super) fn builtin_method(
        &mut self,
        e: &'a Expr,
        base: &'a Expr,
        bt: Ty,
        m: &'a str,
        args: &'a [Expr],
    ) -> (Ty, Val) {
        self.uses_2c = true;
        let Some(mut s) = self.builtin_sig(bt, m) else {
            for a in args {
                self.rvalue(a);
            }
            let names = Self::method_names(bt);
            let msg = format!("`{}` has no method `{m}`", self.tn(bt));
            let hint =
                if matches!(bt, Ty::Arr(_)) && matches!(m, "push" | "pop" | "insert" | "remove") {
                    let el = match bt {
                        Ty::Arr(i) => self.arrs[i as usize].0,
                        _ => Ty::Err,
                    };
                    Some(format!("use a list: []{}", self.tn_elem(el)))
                } else {
                    closest(m, names.iter().copied()).map(|c| format!("did you mean {c}?"))
                };
            self.err("R4.no_method", e.pos, msg, hint);
            return (Ty::Err, Val::Runtime);
        };
        if m == "map" && matches!(bt, Ty::List(_) | Ty::Arr(_)) {
            // no generics: the result type is the return type of the literal (§4.7)
            let el = match bt {
                Ty::List(i) => self.lists[i as usize],
                Ty::Arr(i) => self.arrs[i as usize].0,
                _ => Ty::Err,
            };
            let r = if args.len() == 1 {
                self.literal_ret(&args[0])
            } else {
                None
            };
            let f = match r {
                Some(r) => self.func_ty(r, vec![(el, false)]),
                None => Ty::Err,
            };
            s.params = vec![f];
            let at = base.end + self.src[base.end as usize..].find(m).unwrap_or(0) as u32;
            let pos = self.pos_at(at);
            self.name_pos.insert((e.pos.offset, e.end), pos);
            s.ret = match r {
                Some(r) if r != Ty::Void && r != Ty::Err => self.list(r),
                _ => Ty::Err,
            };
            if r == Some(Ty::Void) && f != Ty::Err {
                let msg = format!(
                    "`map` needs a literal that returns a value, found {}",
                    self.tn(f)
                );
                let hint = "use a for loop to do something for each element".to_string();
                self.err("R8.map_needs_value", args[0].pos, msg, Some(hint));
            }
        }
        let mut changeable = false;
        if s.changes {
            let tname = format!("{}.{m}", self.tn(bt));
            if self.map_entry(base) {
                self.map_place_error(base);
            } else {
                match self.root_of(base) {
                    Some((root, false)) if is_place(base) => {
                        self.call_changes
                            .entry((e.pos.offset, e.end))
                            .or_default()
                            .push((root, e.pos));
                        // the result alone is wanted: say how to get it without the change
                        let text = self.text(base);
                        let hint = match m {
                            "pop" => Some(format!(
                                "use {text}.last() if the list is not needed afterwards"
                            )),
                            "remove" if args.len() == 1 => Some(format!(
                                "use {text}[{}] if the list is not needed afterwards",
                                self.text(&args[0])
                            )),
                            _ => None,
                        };
                        if let Some(h) = hint {
                            self.lost_hints.insert(e.pos.offset, h);
                        }
                        self.changed(base);
                        changeable = true;
                    }
                    Some((root, true)) if is_place(base) => {
                        let msg = format!("cannot change const `{root}`");
                        let hint = format!("`{tname}` changes self");
                        self.err("R4.change_const", base.pos, msg, Some(hint));
                    }
                    _ => {
                        let msg = format!("`{tname}` changes self and needs a variable");
                        let hint = "store the value in a variable first".to_string();
                        self.err("R7.changes_self_needs_var", e.pos, msg, Some(hint));
                    }
                }
            }
        }
        if args.len() != s.params.len() {
            for a in args {
                self.rvalue(a);
            }
            let n = s.params.len();
            let plural = if n == 1 { "" } else { "s" };
            let msg = format!("`{m}` takes {n} argument{plural}, found {}", args.len());
            self.err("R8.arg_count", e.pos, msg, None);
            return (Ty::Err, Val::Runtime);
        }
        for (a, p) in args.iter().zip(&s.params) {
            if matches!(p, Ty::Func(_)) {
                self.arg_value(e, m, *p, a);
            } else {
                self.at_place(*p, a, None);
            }
        }
        self.preset_ret = None;
        if let (Ty::Map(i), "add") = (bt, m) {
            let v = self.maps[i as usize].1;
            if !v.is_number() && v != Ty::Err {
                let msg = format!("`add` needs number values, found {}", self.tn(v));
                let hint = match args {
                    [k, x] => format!(
                        "use {}[{}] = {}",
                        self.text(base),
                        self.text(k),
                        self.text(x)
                    ),
                    _ => String::new(),
                };
                self.err("R8.add_needs_number", e.pos, msg, Some(hint));
            }
        }
        if m == "sort" {
            let el = match bt {
                Ty::List(i) => self.lists[i as usize],
                Ty::Arr(i) => self.arrs[i as usize].0,
                _ => Ty::Err,
            };
            let ordered = el.is_number() || matches!(el, Ty::Rune | Ty::Str | Ty::Err);
            if !ordered {
                let pos = self
                    .pos_at(base.end + self.src[base.end as usize..].find(m).unwrap_or(0) as u32);
                let msg = format!(
                    "`sort` needs an ordered element type, found {}",
                    self.tn(el)
                );
                let hint = "only numbers, rune and str can be sorted".to_string();
                self.err("R28.sort_unordered", pos, msg, Some(hint));
            }
        }
        self.last_callee = None;
        self.last_effect = Some(changeable);
        (s.ret, Val::Runtime)
    }

    /// The return type of the function value `a` passed to `map`, before it is checked: that of
    /// a literal, a named function, a function parameter or a variant constructor.
    fn literal_ret(&mut self, a: &'a Expr) -> Option<Ty> {
        match &a.kind {
            ExprKind::FuncLit(l) if !l.block && l.alt.is_none() => {
                let r = self.ty(&l.ret, true);
                self.preset_ret = Some(r);
                Some(r)
            }
            ExprKind::Ident(n) => match self.local(n).map(|l| l.ty) {
                Some(Ty::Func(i)) => Some(self.ftys[i as usize].0),
                Some(_) => None,
                None => self.funcs.get(n.as_str()).map(|&id| self.fns[id].ret),
            },
            ExprKind::Field(base, _) => self.enum_of(base).map(Ty::Named),
            _ => None,
        }
    }

    /// Line and column of a byte offset (for diagnostics only).
    pub(super) fn pos_at(&self, offset: u32) -> Pos {
        let (mut line, mut col) = (1, 1);
        for (i, c) in self.src.char_indices() {
            if i as u32 >= offset {
                break;
            }
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        Pos { line, col, offset }
    }

    // ---- functions of `io` and `os` (§4.6) ----

    pub(super) fn module_call(
        &mut self,
        e: &'a Expr,
        module: &'static str,
        name: &'a str,
        args: &'a [Expr],
    ) -> (Ty, Val) {
        self.uses_2c = true;
        let funcs = if module == "io" { IO_FUNCS } else { OS_FUNCS };
        let (int, st) = (Ty::Int, Ty::Str);
        let io_err = self
            .type_ids
            .get("io.IoErr")
            .map_or(Ty::Err, |&i| Ty::Named(i));
        // (parameters, result, effect)
        let spec: Option<(Vec<Ty>, Ty, bool)> = match (module, name) {
            ("io", "lines") => {
                let hint = "for str line in try io.lines() { … }".to_string();
                let msg = "`io.lines()` can only be the source of a for loop";
                self.err("R11.io_lines_outside_for", e.pos, msg, Some(hint));
                return (Ty::Err, Val::Runtime);
            }
            ("io", "read_line") => {
                let o = self.opt(st);
                Some((vec![], self.res(o, io_err), true))
            }
            ("io", "read_file") => Some((vec![st], self.res(st, io_err), false)),
            ("io", "write_file") => Some((vec![st, st], self.res(Ty::Void, io_err), true)),
            ("io", "read_lines") => {
                let l = self.list(st);
                Some((vec![st], self.res(l, io_err), false))
            }
            ("io", "write_lines") => {
                let l = self.list(st);
                Some((vec![st, l], self.res(Ty::Void, io_err), true))
            }
            ("io", "eprint") => None,
            ("os", "args") => Some((vec![], self.list(st), false)),
            ("os", "exit") => Some((vec![int], Ty::Void, true)),
            ("os", "env") => Some((vec![st], self.opt(st), false)),
            _ => {
                for a in args {
                    self.rvalue(a);
                }
                let msg = format!("`{module}` has no function `{name}`");
                let hint =
                    closest(name, funcs.iter().copied()).map(|c| format!("did you mean {c}?"));
                self.err("R4.no_function", e.pos, msg, hint);
                return (Ty::Err, Val::Runtime);
            }
        };
        let full = format!("{module}.{name}");
        if name == "eprint" {
            if args.len() != 1 {
                let msg = format!("`{full}` takes 1 argument, found {}", args.len());
                self.err("R8.arg_count", e.pos, msg, None);
                return (Ty::Void, Val::Runtime);
            }
            let (t, _) = self.rvalue(&args[0]);
            if t == Ty::Void {
                let msg = format!("`{}` has no value to print", self.text(&args[0]));
                self.err("R28.no_print_value", args[0].pos, msg, None);
            }
        } else if let Some((params, ret, effect)) = spec {
            if args.len() != params.len() {
                for a in args {
                    self.rvalue(a);
                }
                let n = params.len();
                let plural = if n == 1 { "" } else { "s" };
                let msg = format!("`{full}` takes {n} argument{plural}, found {}", args.len());
                self.err("R8.arg_count", e.pos, msg, None);
                return (Ty::Err, Val::Runtime);
            }
            for (a, p) in args.iter().zip(&params) {
                self.at_place(*p, a, None);
            }
            return self.module_effect(e, ret, effect);
        }
        self.module_effect(e, Ty::Void, true)
    }

    fn module_effect(&mut self, e: &Expr, ret: Ty, effect: bool) -> (Ty, Val) {
        if effect {
            if let Some(id) = self.cur {
                self.facts[id].prints = true;
            }
            self.effect_in_default(e, None);
        }
        self.last_callee = None;
        self.last_effect = Some(effect);
        (ret, Val::Runtime)
    }

    // ---- `for` over collections and results (R11) ----

    pub(super) fn for_stmt(&mut self, vars: &'a [Param], iter: &'a Expr, body: &'a Block) {
        let (src, tried) = match &iter.kind {
            ExprKind::Try(x, or) => (&**x, Some(or.as_ref())),
            _ => (iter, None),
        };
        let range_args = match &src.kind {
            ExprKind::Call(f, args) if matches!(&f.kind, ExprKind::Ident(n) if n == "range") => {
                Some(args)
            }
            _ => None,
        };
        let is_lines = matches!(&src.kind, ExprKind::Call(f, args)
            if args.is_empty() && matches!(&f.kind, ExprKind::Field(b, n)
                if n == "lines" && self.module_name(b) == Some("io")));
        // the types of the loop variables as the source gives them
        let mut elem: Option<Ty> = None;
        let mut pair: Option<(Ty, Ty)> = None;
        match range_args {
            Some(args) => {
                if args.is_empty() || args.len() > 2 {
                    let msg = format!("`range` takes 1 or 2 arguments, found {}", args.len());
                    self.err("R8.range_arity", src.pos, msg, None);
                }
                for a in args {
                    self.at_place(Ty::Int, a, None);
                }
                if vars.len() == 2 {
                    let msg = "`range` gives one value per step; use one loop variable";
                    self.err("R8.range_vars", vars[1].pos, msg, None);
                }
            }
            None if is_lines => {
                self.uses_2c = true;
                if let Some(id) = self.cur {
                    self.facts[id].prints = true;
                }
                let io_err = self
                    .type_ids
                    .get("io.IoErr")
                    .map_or(Ty::Err, |&i| Ty::Named(i));
                elem = Some(self.res(Ty::Str, io_err));
            }
            None => {
                let (t, _) = self.rvalue(src);
                match t {
                    Ty::List(i) => elem = Some(self.lists[i as usize]),
                    Ty::Arr(i) => elem = Some(self.arrs[i as usize].0),
                    Ty::Map(i) => pair = Some(self.maps[i as usize]),
                    Ty::Err => {}
                    t => {
                        let hint = match t {
                            Ty::Int => Some("for int i in range(n)".to_string()),
                            Ty::Str => Some("for rune r in s.runes()".to_string()),
                            _ => None,
                        };
                        let msg = format!("cannot loop over {}", self.tn(t));
                        self.err("R11.not_iterable", src.pos, msg, hint);
                    }
                }
                self.uses_2c = true;
            }
        }
        // `for T x in try xs`: every element is unpacked like `try` (R12)
        if let (Some(el), Some(or)) = (elem, tried) {
            elem = Some(self.try_with(iter, el, None, or).0);
        }
        let mut types: Vec<Ty> = vars.iter().map(|v| self.ty(&v.ty, false)).collect();
        if let Some(el) = elem {
            let want: Vec<Ty> = if vars.len() == 2 {
                vec![Ty::Int, el]
            } else {
                vec![el]
            };
            for (k, v) in vars.iter().enumerate() {
                self.loop_var(v, types[k], want.get(k).copied(), src, tried.is_some());
            }
        }
        if let Some((kt, vt)) = pair {
            if vars.len() == 1 {
                let k = vars[0].name.as_str();
                let v = if k == "v" { "value" } else { "v" };
                let hint = format!(
                    "for {} {k}, {} {v} in {}",
                    self.tn(kt),
                    self.tn(vt),
                    self.text(src)
                );
                let msg = "a loop over a map needs key and value";
                self.err("R11.map_loop_vars", src.pos, msg, Some(hint));
            } else {
                for (k, v) in vars.iter().enumerate() {
                    let want = if k == 0 { kt } else { vt };
                    self.loop_var(v, types[k], Some(want), src, false);
                }
            }
        }
        if range_args.is_some() {
            for (v, t) in vars.iter().zip(types.iter_mut()) {
                if *t != Ty::Int && *t != Ty::Err {
                    let msg = format!("the values of `range` are int, not {}", self.tn(*t));
                    self.err("R8.range_value_type", v.ty.pos, msg, None);
                }
            }
        }
        self.scopes.push(Vec::new());
        for (v, t) in vars.iter().zip(types) {
            self.declare(&v.name, v.pos, t, false, Val::Runtime, None);
        }
        self.ctx.push(Ctx::Loop);
        self.block(body);
        self.ctx.pop();
        self.scopes.pop();
    }

    /// A loop variable of declared type `have` where the source gives `want`.
    fn loop_var(&mut self, v: &Param, have: Ty, want: Option<Ty>, src: &Expr, tried: bool) {
        let Some(want) = want else { return };
        if have == want || have == Ty::Err || want == Ty::Err {
            return;
        }
        let msg = format!("expected {}, found {}", self.tn(have), self.tn(want));
        let hint = (!tried && self.inner(want) == Some(have))
            .then(|| format!("for {} {} in try {}", self.tn(have), v.name, self.text(src)));
        self.err("R8.type_mismatch", v.pos, msg, hint);
    }
}

impl Num {
    pub fn from_name(n: &str) -> Option<Num> {
        Num::ALL.into_iter().find(|x| x.name() == n)
    }
}

impl<'a> Checker<'a> {
    /// Does the builtin method `m` of `bt` change `self`?
    pub(super) fn builtin_changes(&mut self, bt: Ty, m: &str) -> bool {
        self.builtin_sig(bt, m).is_some_and(|s| s.changes)
    }
}
