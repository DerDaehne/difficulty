//! C11 output for a checked module: one C file per program (spec §6, R14, R29).
//! Names get the prefix `d_`; `#line` points every statement at the `.dif` source.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use crate::ast::*;
use crate::checker::{Info, Ty, TypeBody};
use crate::diag::Pos;

const RUNTIME: &str = r#"#include <inttypes.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Helpers are `static inline`; clang warns about unused ones in the main file.
#pragma GCC diagnostic ignored "-Wunused-function"
typedef struct { const char *p; int64_t n; } dif_str;
typedef struct { char *p; int64_t n, cap; } dif_buf;

__attribute__((noinline, cold)) static _Noreturn void dif_panic(const char *msg, int line, int col) {
    fflush(stdout);
    fprintf(stderr, "panic: %s (%s:%d:%d)\n", msg, dif_file, line, col);
    exit(2);
}
static inline int64_t dif_add(int64_t a, int64_t b, int l, int c) {
    int64_t r;
    if (__builtin_add_overflow(a, b, &r)) dif_panic("integer overflow in +", l, c);
    return r;
}
static inline int64_t dif_sub(int64_t a, int64_t b, int l, int c) {
    int64_t r;
    if (__builtin_sub_overflow(a, b, &r)) dif_panic("integer overflow in -", l, c);
    return r;
}
static inline int64_t dif_mul(int64_t a, int64_t b, int l, int c) {
    int64_t r;
    if (__builtin_mul_overflow(a, b, &r)) dif_panic("integer overflow in *", l, c);
    return r;
}
static inline int64_t dif_neg(int64_t a, int l, int c) {
    if (a == INT64_MIN) dif_panic("integer overflow in -", l, c);
    return -a;
}
static inline int64_t dif_div(int64_t a, int64_t b, int l, int c) {
    if (b == 0) dif_panic("division by zero", l, c);
    if (a == INT64_MIN && b == -1) dif_panic("integer overflow in /", l, c);
    return a / b;
}
static inline int64_t dif_mod(int64_t a, int64_t b, int l, int c) {
    if (b == 0) dif_panic("division by zero", l, c);
    return b == -1 ? 0 : a % b;
}
static inline void dif_shift(int64_t b, int l, int c) {
    if (b < 0 || b > 63) {
        char m[64];
        snprintf(m, sizeof m, "shift by %" PRId64 " out of range 0..63", b);
        dif_panic(m, l, c);
    }
}
static inline int64_t dif_shl(int64_t a, int64_t b, int l, int c) {
    dif_shift(b, l, c);
    return (int64_t)((uint64_t)a << b);
}
static inline int64_t dif_shr(int64_t a, int64_t b, int l, int c) {
    dif_shift(b, l, c);
    return a >> b;
}
static inline int64_t dif_wadd(int64_t a, int64_t b) { return (int64_t)((uint64_t)a + (uint64_t)b); }
static inline int64_t dif_wsub(int64_t a, int64_t b) { return (int64_t)((uint64_t)a - (uint64_t)b); }
static inline int64_t dif_wmul(int64_t a, int64_t b) { return (int64_t)((uint64_t)a * (uint64_t)b); }
static inline void dif_put(dif_buf *b, const char *p, int64_t n) {
    if (n == 0) return;
    if (b->n + n > b->cap) {
        b->cap = (b->n + n) * 2 + 16;
        b->p = realloc(b->p, (size_t)b->cap);
        if (!b->p) abort();
    }
    memcpy(b->p + b->n, p, (size_t)n);
    b->n += n;
}
static inline dif_buf dif_buf_new(void) { return (dif_buf){0, 0, 0}; }
static inline void dif_put_str(dif_buf *b, dif_str s) { dif_put(b, s.p, s.n); }
static inline void dif_put_int(dif_buf *b, int64_t v) {
    char t[24];
    int n = snprintf(t, sizeof t, "%" PRId64, v);
    dif_put(b, t, n);
}
static inline void dif_put_bool(dif_buf *b, bool v) { dif_put(b, v ? "true" : "false", v ? 4 : 5); }
static inline dif_str dif_take(dif_buf *b) { return (dif_str){b->p ? b->p : "", b->n}; }
static inline dif_str dif_cat(dif_str a, dif_str b) {
    dif_buf r = dif_buf_new();
    dif_put_str(&r, a);
    dif_put_str(&r, b);
    return dif_take(&r);
}
static inline bool dif_eq(dif_str a, dif_str b) {
    return a.n == b.n && (a.n == 0 || memcmp(a.p, b.p, (size_t)a.n) == 0);
}
static inline int dif_cmp(dif_str a, dif_str b) {
    int64_t n = a.n < b.n ? a.n : b.n;
    int r = n ? memcmp(a.p, b.p, (size_t)n) : 0;
    return r ? r : (a.n > b.n) - (a.n < b.n);
}
static inline void dif_print_str(dif_str s) {
    fwrite(s.p, 1, (size_t)s.n, stdout);
    putchar('\n');
}
static inline void dif_print_int(int64_t v) { printf("%" PRId64 "\n", v); }
static inline void dif_print_bool(bool v) { puts(v ? "true" : "false"); }
static inline void dif_put_quoted(dif_buf *b, dif_str s) {
    dif_put(b, "\"", 1);
    for (int64_t i = 0; i < s.n; i++) {
        switch (s.p[i]) {
        case '"': dif_put(b, "\\\"", 2); break;
        case '\\': dif_put(b, "\\\\", 2); break;
        case '\n': dif_put(b, "\\n", 2); break;
        case '\t': dif_put(b, "\\t", 2); break;
        case '\r': dif_put(b, "\\r", 2); break;
        case '\0': dif_put(b, "\\0", 2); break;
        case '{': dif_put(b, "\\{", 2); break;
        case '}': dif_put(b, "\\}", 2); break;
        default: dif_put(b, &s.p[i], 1);
        }
    }
    dif_put(b, "\"", 1);
}
"#;

// Design notes (part 2a).
// - Types come from the checker (`Info`): the type of every expression by span, the
//   type table, and which methods change `self`. The emitter derives nothing.
// - Values: a `struct` is a C struct, an `enum` a struct with `tag` and a union of the
//   variants' values; assignment, parameters and returns copy, as in C.
// - `mut` parameters and `self` of a method that changes it are pointers; every use of
//   such a name is written `(*d_p)`, so a place is always a C lvalue and `mut p` is `&p`.
//   Arguments without `mut` are copied before the callee runs, which is the snapshot of R6.
// - Expressions that need statements (struct literals, enum values, `if` as a value) use
//   GNU statement expressions `({ ... })`. gcc and clang accept them under
//   `-std=c11 -Wall -Wextra -Werror` without a warning (tested in tests/backend.rs), and
//   they keep every expression a string, so `&&`, `||` and the choice of one branch of `if`
//   need no lowering into temporaries before the statement.
// - Order (R29): C leaves the order of arguments and of initializers open, so literals
//   assign field by field, and a call with several operands puts them into temporaries
//   when any of them calls something.

/// C source for a module that passed the checker.
pub fn emit(file: &str, m: &Module, info: &Info) -> String {
    if info.part2b {
        return "#error \"error types, optionals and tests (part 2b) are not supported by the C backend yet\"\n".into();
    }
    let mut e = Emitter {
        file: c_string(file.as_bytes()),
        info,
        funcs: HashMap::new(),
        methods: HashMap::new(),
        structs: HashMap::new(),
        consts: HashMap::new(),
        derefs: HashSet::new(),
        temps: Vec::new(),
        out: String::new(),
        nested: 0,
        uid: 0,
        lows: HashMap::new(),
        steady: HashSet::new(),
    };
    e.module(m)
}

struct Emitter<'a> {
    file: String,
    info: &'a Info,
    funcs: HashMap<&'a str, &'a Func>,
    methods: HashMap<(&'a str, &'a str), &'a Func>,
    structs: HashMap<&'a str, &'a StructDecl>,
    consts: HashMap<&'a str, Ty>,
    /// Names of the current function that are pointers (`mut` parameters, `self`).
    derefs: HashSet<&'a str>,
    /// Temporaries of the current function: C type and name.
    temps: Vec<(String, String)>,
    out: String,
    /// Depth of statements inside an expression (no `#line` there).
    nested: usize,
    uid: usize,
    /// Proven lower bounds of `int` parameters that the function never changes: after
    /// `if n < K { return }` the rest of the block knows `n >= K`, so `n - c` cannot overflow.
    lows: HashMap<&'a str, i64>,
    /// The `int` parameters of the current function that are never changed.
    steady: HashSet<&'a str>,
}

/// Does `n` get changed in `stmts`: assigned, or passed as `mut`?
fn changes(stmts: &[Stmt], n: &str) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Var(v) => sets(&v.value, n),
        Stmt::Assign { place, value, .. } => {
            matches!(&place.kind, ExprKind::Ident(p) if p == n) || sets(value, n)
        }
        Stmt::If {
            branches,
            otherwise,
            ..
        } => {
            branches
                .iter()
                .any(|(c, b)| sets(c, n) || changes(&b.stmts, n))
                || otherwise.as_ref().is_some_and(|b| changes(&b.stmts, n))
        }
        Stmt::While { cond, body, .. } => sets(cond, n) || changes(&body.stmts, n),
        Stmt::For { iter, body, .. } => sets(iter, n) || changes(&body.stmts, n),
        Stmt::Switch {
            subject,
            cases,
            default,
            ..
        } => {
            sets(subject, n)
                || cases.iter().any(|c| changes(&c.body.stmts, n))
                || default.as_ref().is_some_and(|d| changes(&d.0.stmts, n))
        }
        Stmt::Return(e, _) => e.as_ref().is_some_and(|e| sets(e, n)),
        Stmt::Call(e) | Stmt::Discard(e, _) => sets(e, n),
        Stmt::Break(_) | Stmt::Continue(_) => false,
    })
}

fn sets(e: &Expr, n: &str) -> bool {
    match &e.kind {
        ExprKind::Mut(x) => matches!(&x.kind, ExprKind::Ident(p) if p == n) || sets(x, n),
        ExprKind::Unary(_, x)
        | ExprKind::Paren(x)
        | ExprKind::Field(x, _)
        | ExprKind::SomeVal(x)
        | ExprKind::ErrVal(x)
        | ExprKind::OkVal(Some(x)) => sets(x, n),
        ExprKind::Try(x, or) => sets(x, n) || or.as_ref().is_some_and(|(_, e)| sets(e, n)),
        ExprKind::Binary(_, l, r, _) | ExprKind::Index(l, r) => sets(l, n) || sets(r, n),
        ExprKind::Call(f, args) => sets(f, n) || args.iter().any(|a| sets(a, n)),
        ExprKind::StructLit(_, inits) => inits.iter().any(|i| sets(&i.value, n)),
        ExprKind::Str(segs) => segs
            .iter()
            .any(|s| matches!(s, StrSeg::Expr(x) if sets(x, n))),
        ExprKind::IfExpr(branches, last) => {
            let vb = |b: &ValueBlock| changes(&b.stmts, n) || sets(&b.value, n);
            branches.iter().any(|(c, b)| sets(c, n) || vb(b)) || vb(last)
        }
        // no `_` arm: a new expression kind must be classified here (it could change `n`)
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Ident(_)
        | ExprKind::SelfRef
        | ExprKind::NoneVal
        | ExprKind::OkVal(None)
        | ExprKind::Missing => false,
    }
}

fn c_ty_prim(t: Ty) -> &'static str {
    match t {
        Ty::Int => "int64_t",
        Ty::Bool => "bool",
        Ty::Str => "dif_str",
        _ => "void",
    }
}

/// A C string literal; bytes outside printable ASCII as octal escapes.
fn c_string(bytes: &[u8]) -> String {
    let mut s = String::from("\"");
    for &b in bytes {
        match b {
            b'"' => s.push_str("\\\""),
            b'\\' => s.push_str("\\\\"),
            b'?' => s.push_str("\\?"), // no trigraphs
            0x20..=0x7e => s.push(b as char),
            _ => {
                let _ = write!(s, "\\{b:03o}");
            }
        }
    }
    s.push('"');
    s
}

fn c_int(v: i64) -> String {
    if v == i64::MIN {
        "INT64_MIN".into()
    } else {
        format!("INT64_C({v})")
    }
}

fn has_call(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Call(..) | ExprKind::IfExpr(..) => true,
        ExprKind::Unary(_, x) | ExprKind::Paren(x) | ExprKind::Mut(x) | ExprKind::Field(x, _) => {
            has_call(x)
        }
        ExprKind::Binary(_, l, r, _) => has_call(l) || has_call(r),
        ExprKind::StructLit(_, inits) => inits.iter().any(|i| has_call(&i.value)),
        ExprKind::Str(segs) => segs
            .iter()
            .any(|s| matches!(s, StrSeg::Expr(x) if has_call(x))),
        _ => false,
    }
}

/// An operand of a call: its value, or the address of a place (`mut`, a receiver
/// that is changed).
#[derive(Clone, Copy)]
enum Arg<'a> {
    Val(&'a Expr),
    Addr(&'a Expr),
}

impl<'a> Emitter<'a> {
    // ---- types ----

    fn ty_of(&self, t: &TypeRef) -> Ty {
        match t.name.as_str() {
            "int" => Ty::Int,
            "bool" => Ty::Bool,
            "str" => Ty::Str,
            "void" => Ty::Void,
            n => self
                .info
                .types
                .iter()
                .position(|d| d.name == n)
                .map_or(Ty::Err, |i| Ty::Named(i as u32)),
        }
    }

    fn c_ty(&self, t: Ty) -> String {
        match t {
            Ty::Named(id) => format!("d_{}", self.info.types[id as usize].name),
            t => c_ty_prim(t).into(),
        }
    }

    fn tname(&self, id: u32) -> &'a str {
        &self.info.types[id as usize].name
    }

    /// The checked type of an expression.
    fn ty(&self, e: &Expr) -> Ty {
        self.info
            .exprs
            .get(&(e.pos.offset, e.end))
            .copied()
            .unwrap_or(Ty::Err)
    }

    fn enum_named(&self, name: &str) -> Option<u32> {
        let i = self.info.types.iter().position(|d| d.name == name)?;
        matches!(self.info.types[i].body, TypeBody::Enum(_)).then_some(i as u32)
    }

    fn variant_index(&self, eid: u32, name: &str) -> usize {
        match &self.info.types[eid as usize].body {
            TypeBody::Enum(vs) => vs.iter().position(|v| v.0 == name).unwrap_or(0),
            TypeBody::Struct(_) => 0,
        }
    }

    // ---- type definitions and their helpers (equality, text) ----

    fn type_defs(&self) -> String {
        let mut done = vec![false; self.info.types.len()];
        let mut c = String::new();
        for i in 0..done.len() {
            self.type_def(i, &mut done, &mut c);
        }
        c
    }

    /// A type after the types it contains (a value has a fixed size, R5).
    fn type_def(&self, i: usize, done: &mut Vec<bool>, c: &mut String) {
        if done[i] {
            return;
        }
        done[i] = true;
        let d = &self.info.types[i];
        let members = |fs: &mut dyn Iterator<Item = (Ty, String)>| -> Vec<String> {
            fs.map(|(t, n)| format!("{} {n};", self.c_ty(t))).collect()
        };
        let contained: Vec<Ty> = match &d.body {
            TypeBody::Struct(fs) => fs.iter().map(|f| f.1).collect(),
            TypeBody::Enum(vs) => vs.iter().flat_map(|v| v.1.iter().map(|f| f.0)).collect(),
        };
        for t in contained {
            if let Ty::Named(j) = t {
                self.type_def(j as usize, done, c);
            }
        }
        let body = match &d.body {
            TypeBody::Struct(fs) => {
                let mut m = members(&mut fs.iter().map(|f| (f.1, format!("f_{}", f.0))));
                if m.is_empty() {
                    m.push("char unused;".into());
                }
                m.join(" ")
            }
            TypeBody::Enum(vs) => {
                let arms: Vec<String> = vs
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| !v.1.is_empty())
                    .map(|(k, v)| {
                        let m = members(
                            &mut v.1.iter().enumerate().map(|(j, f)| (f.0, format!("f{j}"))),
                        );
                        format!("struct {{ {} }} v{k};", m.join(" "))
                    })
                    .collect();
                if arms.is_empty() {
                    "int64_t tag;".into()
                } else {
                    format!("int64_t tag; union {{ {} }} u;", arms.join(" "))
                }
            }
        };
        let _ = writeln!(c, "typedef struct {{ {body} }} d_{};", d.name);
    }

    /// Prototypes of the helpers of every type.
    fn helper_protos(&self) -> String {
        let mut c = String::new();
        for d in &self.info.types {
            let n = &d.name;
            let _ = writeln!(c, "static inline bool dif_eq_{n}(d_{n} a, d_{n} b);");
            let _ = writeln!(c, "static inline void dif_put_{n}(dif_buf *b, d_{n} v);");
            let _ = writeln!(c, "static inline void dif_print_{n}(d_{n} v);");
        }
        c
    }

    /// `a == b` for values of type `t`.
    fn eq(&self, t: Ty, a: &str, b: &str) -> String {
        match t {
            Ty::Str => format!("dif_eq({a}, {b})"),
            Ty::Named(id) => format!("dif_eq_{}({a}, {b})", self.tname(id)),
            _ => format!("({a} == {b})"),
        }
    }

    /// A statement appending the text of `v` to buffer `b`; a `str` inside a
    /// struct or enum is quoted (R28).
    fn put(&self, t: Ty, b: &str, v: &str, quoted: bool) -> String {
        match t {
            Ty::Int => format!("dif_put_int({b}, {v});"),
            Ty::Bool => format!("dif_put_bool({b}, {v});"),
            Ty::Named(id) => format!("dif_put_{}({b}, {v});", self.tname(id)),
            _ if quoted => format!("dif_put_quoted({b}, {v});"),
            _ => format!("dif_put_str({b}, {v});"),
        }
    }

    fn lit(b: &str, s: &str) -> String {
        format!("dif_put({b}, {}, {});", c_string(s.as_bytes()), s.len())
    }

    fn helpers(&self) -> String {
        let mut c = String::new();
        for d in &self.info.types {
            let n = &d.name;
            let _ = writeln!(c, "static inline bool dif_eq_{n}(d_{n} a, d_{n} b) {{");
            let _ = writeln!(c, "    (void)a;\n    (void)b;");
            match &d.body {
                TypeBody::Struct(fs) => {
                    let all: Vec<String> = fs
                        .iter()
                        .map(|(f, t)| self.eq(*t, &format!("a.f_{f}"), &format!("b.f_{f}")))
                        .collect();
                    let all = if all.is_empty() {
                        "true".into()
                    } else {
                        all.join(" && ")
                    };
                    let _ = writeln!(c, "    return {all};\n}}");
                    let _ = writeln!(c, "static inline void dif_put_{n}(dif_buf *b, d_{n} v) {{");
                    let _ = writeln!(c, "    (void)v;");
                    let _ = writeln!(c, "    {}", Self::lit("b", &format!("{n}{{")));
                    for (k, (f, t)) in fs.iter().enumerate() {
                        let sep = if k == 0 { "" } else { ", " };
                        let _ = writeln!(c, "    {}", Self::lit("b", &format!("{sep}{f}: ")));
                        let _ = writeln!(c, "    {}", self.put(*t, "b", &format!("v.f_{f}"), true));
                    }
                    let _ = writeln!(c, "    {}\n}}", Self::lit("b", "}"));
                }
                TypeBody::Enum(vs) => {
                    let _ = writeln!(c, "    if (a.tag != b.tag) return false;");
                    for (k, v) in vs.iter().enumerate().filter(|(_, v)| !v.1.is_empty()) {
                        let all: Vec<String> =
                            v.1.iter()
                                .enumerate()
                                .map(|(j, f)| {
                                    let x = format!("a.u.v{k}.f{j}");
                                    self.eq(f.0, &x, &format!("b.u.v{k}.f{j}"))
                                })
                                .collect();
                        let _ = writeln!(c, "    if (a.tag == {k}) return {};", all.join(" && "));
                    }
                    let _ = writeln!(c, "    return true;\n}}");
                    let _ = writeln!(c, "static inline void dif_put_{n}(dif_buf *b, d_{n} v) {{");
                    let _ = writeln!(c, "    (void)v;");
                    for (k, v) in vs.iter().enumerate() {
                        let _ = writeln!(c, "    if (v.tag == {k}) {{");
                        let _ = writeln!(c, "        {}", Self::lit("b", &format!("{n}.{}", v.0)));
                        if !v.1.is_empty() {
                            let _ = writeln!(c, "        {}", Self::lit("b", "("));
                        }
                        for (j, f) in v.1.iter().enumerate() {
                            let sep = if j == 0 { "" } else { ", " };
                            let _ = writeln!(c, "        {}", Self::lit("b", sep));
                            let x = format!("v.u.v{k}.f{j}");
                            let _ = writeln!(c, "        {}", self.put(f.0, "b", &x, true));
                        }
                        if !v.1.is_empty() {
                            let _ = writeln!(c, "        {}", Self::lit("b", ")"));
                        }
                        let _ = writeln!(c, "    }}");
                    }
                    let _ = writeln!(c, "}}");
                }
            }
            let _ = writeln!(c, "static inline void dif_print_{n}(d_{n} v) {{");
            let _ = writeln!(c, "    dif_buf b = dif_buf_new();");
            let _ = writeln!(c, "    dif_put_{n}(&b, v);");
            let _ = writeln!(c, "    dif_print_str(dif_take(&b));\n}}");
        }
        c
    }

    // ---- module ----

    fn module(&mut self, m: &'a Module) -> String {
        let mut c = format!("static const char *const dif_file = {};\n", self.file);
        c.push_str(RUNTIME);
        for d in &m.decls {
            match d {
                Decl::Func(f) => match &f.recv {
                    Some(r) => {
                        self.methods.insert((&r.name, &f.name), f);
                    }
                    None => {
                        self.funcs.insert(&f.name, f);
                    }
                },
                Decl::Const(v) => {
                    let t = self.ty_of(&v.ty);
                    self.consts.insert(&v.name, t);
                }
                Decl::Struct(s) => {
                    self.structs.insert(&s.name, s);
                }
                Decl::Enum(_) | Decl::Test(_) => {}
            }
        }
        c.push_str(&self.type_defs());
        c.push_str(&self.helper_protos());
        // prototypes, so the order of declarations does not matter
        for d in &m.decls {
            match d {
                Decl::Func(f) => {
                    let _ = writeln!(c, "{};", self.header(f));
                }
                Decl::Const(v) => {
                    let _ = writeln!(
                        c,
                        "static inline {} d_{}(void);",
                        self.c_ty(self.ty_of(&v.ty)),
                        v.name
                    );
                }
                _ => {}
            }
        }
        c.push_str(&self.helpers());
        for d in &m.decls {
            match d {
                Decl::Const(v) => {
                    // a function, so consts may use each other in any order
                    self.temps.clear();
                    let value = self.expr(&v.value);
                    let t = self.c_ty(self.ty_of(&v.ty));
                    let _ = writeln!(
                        c,
                        "static inline {t} d_{}(void) {{\n{}    return {value};\n}}",
                        v.name,
                        self.temp_decls()
                    );
                }
                Decl::Func(f) => c.push_str(&self.func(f)),
                _ => {}
            }
        }
        c.push_str("int main(void) {\n    d_main();\n    return 0;\n}\n");
        c
    }

    fn cname(f: &Func) -> String {
        match &f.recv {
            Some(r) => format!("dm_{}_{}", r.name, f.name),
            None => format!("d_{}", f.name),
        }
    }

    fn changes_self(&self, f: &Func) -> bool {
        f.recv.as_ref().is_some_and(|r| {
            self.info
                .self_changes
                .contains(&(r.name.clone(), f.name.clone()))
        })
    }

    fn header(&self, f: &Func) -> String {
        let mut params: Vec<String> = Vec::new();
        if let Some(r) = &f.recv {
            let star = if self.changes_self(f) { " *" } else { " " };
            params.push(format!("d_{}{star}d_self", r.name));
        }
        for p in &f.params {
            let star = if p.mut_pos.is_some() { " *" } else { " " };
            params.push(format!(
                "{}{star}d_{}",
                self.c_ty(self.ty_of(&p.ty)),
                p.name
            ));
        }
        let params = if params.is_empty() {
            "void".into()
        } else {
            params.join(", ")
        };
        format!(
            "{} {}({params})",
            self.c_ty(self.ty_of(&f.ret)),
            Self::cname(f)
        )
    }

    fn temp_decls(&mut self) -> String {
        self.temps
            .iter()
            .map(|(t, n)| format!("    {t} {n};\n"))
            .collect()
    }

    fn temp(&mut self, t: String, prefix: &str) -> String {
        let name = format!("{prefix}{}", self.temps.len());
        self.temps.push((t, name.clone()));
        name
    }

    fn func(&mut self, f: &'a Func) -> String {
        self.temps.clear();
        self.out.clear();
        self.derefs.clear();
        self.lows.clear();
        self.steady = (f.params.iter())
            .filter(|p| p.mut_pos.is_none() && self.ty_of(&p.ty) == Ty::Int)
            .filter(|p| !changes(&f.body.stmts, &p.name))
            .map(|p| p.name.as_str())
            .collect();
        if f.recv.is_some() {
            if self.changes_self(f) {
                self.derefs.insert("self");
            }
            self.line(1, "(void)d_self;");
        }
        for p in &f.params {
            if p.mut_pos.is_some() {
                self.derefs.insert(&p.name);
            }
            self.line(1, &format!("(void)d_{};", p.name));
        }
        self.block_body(&f.body, 1);
        let mut c = format!("{} {{\n", self.header(f));
        c.push_str(&self.temp_decls());
        c.push_str(&self.out);
        if f.ret.name != "void" {
            c.push_str("    __builtin_unreachable();\n");
        }
        c.push_str("}\n");
        c
    }

    // ---- statements ----

    fn line(&mut self, depth: usize, text: &str) {
        let _ = writeln!(self.out, "{}{text}", "    ".repeat(depth));
    }

    fn block(&mut self, b: &'a Block, depth: usize) {
        self.line(depth - 1, "{");
        self.block_body(b, depth);
        self.line(depth - 1, "}");
    }

    fn block_body(&mut self, b: &'a Block, depth: usize) {
        let saved = self.lows.clone();
        for s in &b.stmts {
            self.stmt(s, depth);
            self.learn(s);
        }
        self.lows = saved;
    }

    /// After `if p < K { ...; return }` (no `else`) the rest of the block knows `p >= K`.
    fn learn(&mut self, s: &'a Stmt) {
        let Stmt::If {
            branches,
            otherwise: None,
            ..
        } = s
        else {
            return;
        };
        let [(c, b)] = branches.as_slice() else {
            return;
        };
        let (ExprKind::Binary(op, l, r, _), Some(Stmt::Return(..))) = (&c.kind, b.stmts.last())
        else {
            return;
        };
        let (ExprKind::Ident(n), Some(k)) = (&l.kind, self.int_value(r)) else {
            return;
        };
        let k = match *op {
            "<" => k,
            "<=" => k.saturating_add(1),
            _ => return,
        };
        if let Some(n) = self.steady.get(n.as_str()) {
            let low = self.lows.entry(n).or_insert(k);
            *low = (*low).max(k);
        }
    }

    fn int_value(&self, e: &Expr) -> Option<i64> {
        match (&e.kind, self.info.folded.get(&(e.pos.offset, e.end))) {
            (_, Some(v)) => Some(*v),
            (ExprKind::Int(v), _) => i64::try_from(*v).ok(),
            _ => None,
        }
    }

    /// `n - c` with `n >= K` known and `K - c` representable: no overflow check needed.
    fn safe_sub(&self, l: &Expr, r: &Expr) -> bool {
        let (ExprKind::Ident(n), Some(c)) = (&l.kind, self.int_value(r)) else {
            return false;
        };
        c >= 0
            && self
                .lows
                .get(n.as_str())
                .is_some_and(|k| k.checked_sub(c).is_some())
    }

    fn stmt(&mut self, s: &'a Stmt, d: usize) {
        let pos = match s {
            Stmt::Var(v) => v.pos,
            Stmt::Assign { pos, .. } | Stmt::If { pos, .. } | Stmt::While { pos, .. } => *pos,
            Stmt::For { pos, .. } | Stmt::Break(pos) | Stmt::Continue(pos) => *pos,
            Stmt::Switch { pos, .. } => *pos,
            Stmt::Return(_, pos) | Stmt::Discard(_, pos) => *pos,
            Stmt::Call(e) => e.pos,
        };
        if self.nested == 0 {
            let _ = writeln!(self.out, "#line {} {}", pos.line, self.file);
        }
        match s {
            Stmt::Var(v) => {
                let t = self.c_ty(self.ty_of(&v.ty));
                let value = self.expr(&v.value);
                self.line(d, &format!("{t} d_{} = {value};", v.name));
                self.line(d, &format!("(void)d_{};", v.name));
            }
            Stmt::Assign {
                place,
                op,
                value,
                pos,
            } => {
                let target = self.expr(place);
                let v = if *op == "=" {
                    self.expr(value)
                } else {
                    let t = self.ty(place);
                    let rhs = self.expr(value);
                    self.op(&op[..op.len() - 1], t, target.clone(), rhs, *pos)
                };
                self.line(d, &format!("{target} = {v};"));
            }
            Stmt::If {
                branches,
                otherwise,
                ..
            } => {
                for (k, (c, b)) in branches.iter().enumerate() {
                    let c = self.expr(c);
                    let kw = if k == 0 { "if" } else { "else if" };
                    self.line(d, &format!("{kw} {}", paren(&c)));
                    self.block(b, d + 1);
                }
                if let Some(b) = otherwise {
                    self.line(d, "else");
                    self.block(b, d + 1);
                }
            }
            Stmt::While { cond, body, .. } => {
                let c = self.expr(cond);
                self.line(d, &format!("while {}", paren(&c)));
                self.block(body, d + 1);
            }
            Stmt::For {
                vars, iter, body, ..
            } => self.for_range(vars, iter, body, d),
            Stmt::Switch {
                subject,
                cases,
                default,
                ..
            } => self.switch(subject, cases, default.as_ref().map(|d| &d.0), d),
            Stmt::Break(_) => self.line(d, "break;"),
            Stmt::Continue(_) => self.line(d, "continue;"),
            Stmt::Return(None, _) => self.line(d, "return;"),
            Stmt::Return(Some(e), _) => {
                let v = self.expr(e);
                self.line(d, &format!("return {v};"));
            }
            Stmt::Call(e) => {
                let v = self.expr(e);
                self.line(d, &format!("{v};"));
            }
            Stmt::Discard(e, _) => {
                let v = self.expr(e);
                self.line(d, &format!("(void)({v});"));
            }
        }
    }

    /// `switch`: an if-chain over the subject, evaluated once. No C `switch`: case values
    /// may be `const` names, nothing falls through, and `continue` reaches the loop (R10).
    fn switch(
        &mut self,
        subject: &'a Expr,
        cases: &'a [Case],
        default: Option<&'a Block>,
        d: usize,
    ) {
        let st = self.ty(subject);
        let s = format!("s{}", self.uid);
        self.uid += 1;
        let v = self.expr(subject);
        self.line(d, "{");
        self.line(d + 1, &format!("{} {s} = {v};", self.c_ty(st)));
        self.line(d + 1, &format!("(void){s};"));
        for (k, case) in cases.iter().enumerate() {
            let tests: Vec<String> = case.pats.iter().map(|p| self.pattern_test(p, &s)).collect();
            let kw = if k == 0 { "if" } else { "else if" };
            self.line(d + 1, &format!("{kw} ({}) {{", tests.join(" || ")));
            if let [Pattern::Variant {
                ty,
                name,
                binds: Some(binds),
                ..
            }] = case.pats.as_slice()
            {
                let eid = self.enum_named(ty).unwrap_or(0);
                let k = self.variant_index(eid, name);
                for (j, b) in binds.iter().enumerate() {
                    if let Some(b) = b {
                        let t = self.c_ty(self.ty_of(&b.ty));
                        self.line(d + 2, &format!("{t} d_{} = {s}.u.v{k}.f{j};", b.name));
                        self.line(d + 2, &format!("(void)d_{};", b.name));
                    }
                }
            }
            self.block_body(&case.body, d + 2);
            self.line(d + 1, "}");
        }
        if let Some(b) = default {
            if !cases.is_empty() {
                self.line(d + 1, "else");
            }
            self.block(b, d + 2);
        }
        self.line(d, "}");
    }

    fn pattern_test(&mut self, p: &Pattern, s: &str) -> String {
        match p {
            Pattern::Int(v, _) => format!("{s} == {}", c_int(*v as i64)),
            Pattern::Str(t, _) => format!("dif_eq({s}, {})", self.str_lit(t)),
            Pattern::Name(n, _) => match self.consts.get(n.as_str()) {
                Some(Ty::Str) => format!("dif_eq({s}, d_{n}())"),
                _ => format!("{s} == d_{n}()"),
            },
            Pattern::Variant { ty, name, .. } => {
                let eid = self.enum_named(ty).unwrap_or(0);
                format!("{s}.tag == {}", self.variant_index(eid, name))
            }
            Pattern::Result { .. } => "0".into(),
        }
    }

    /// `for int i in range(a, b)`: bounds once before the loop, the variable a fresh
    /// copy in every step (R11).
    fn for_range(&mut self, vars: &'a [Param], iter: &'a Expr, body: &'a Block, d: usize) {
        let ExprKind::Call(_, args) = &iter.kind else {
            return;
        };
        let (lo, hi) = match args.as_slice() {
            [n] => ("INT64_C(0)".to_string(), self.expr(n)),
            [a, b] => (self.expr(a), self.expr(b)),
            _ => return,
        };
        let (l, h, i) = (
            self.temp("int64_t".into(), "t"),
            self.temp("int64_t".into(), "t"),
            self.temp("int64_t".into(), "t"),
        );
        self.line(d, &format!("{l} = {lo};"));
        self.line(d, &format!("{h} = {hi};"));
        self.line(d, &format!("for ({i} = {l}; {i} < {h}; {i}++) {{"));
        let v = &vars[0];
        self.line(d + 1, &format!("int64_t d_{} = {i};", v.name));
        self.line(d + 1, &format!("(void)d_{};", v.name));
        self.block(body, d + 2);
        self.line(d, "}");
    }

    // ---- expressions ----

    fn str_lit(&self, s: &str) -> String {
        format!("((dif_str){{{}, {}}})", c_string(s.as_bytes()), s.len())
    }

    fn expr(&mut self, e: &'a Expr) -> String {
        if let Some(v) = self.info.folded.get(&(e.pos.offset, e.end)) {
            return c_int(*v);
        }
        match &e.kind {
            ExprKind::Int(v) => c_int(*v as i64),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Str(segs) => self.text(segs),
            ExprKind::Ident(n) if self.consts.contains_key(n.as_str()) => format!("d_{n}()"),
            ExprKind::Ident(n) if self.derefs.contains(n.as_str()) => format!("(*d_{n})"),
            ExprKind::Ident(n) => format!("d_{n}"),
            ExprKind::SelfRef if self.derefs.contains("self") => "(*d_self)".into(),
            ExprKind::SelfRef => "d_self".into(),
            ExprKind::Paren(x) => self.expr(x),
            ExprKind::Unary(op, x) => {
                let v = self.expr(x);
                match *op {
                    "-" => format!("dif_neg({v}, {}, {})", e.pos.line, e.pos.col),
                    o => format!("({o}{v})"),
                }
            }
            ExprKind::Binary(op, l, r, opos) => {
                let t = self.ty(l);
                if *op == "&&" || *op == "||" {
                    return format!("({} {op} {})", self.expr(l), self.expr(r));
                }
                if *op == "-" && t == Ty::Int && self.safe_sub(l, r) {
                    return format!("({} - {})", self.expr(l), self.expr(r));
                }
                let (pre, vals) = self.ordered(&[Arg::Val(l), Arg::Val(r)]);
                let [a, b] = <[String; 2]>::try_from(vals).unwrap_or_default();
                let v = self.op(op, t, a, b, *opos);
                if pre.is_empty() {
                    v
                } else {
                    format!("({pre}{v})")
                }
            }
            ExprKind::Call(f, args) => self.call(e, f, args),
            ExprKind::Field(base, name) => match &base.kind {
                ExprKind::Ident(tn) if self.enum_named(tn).is_some() => {
                    let eid = self.enum_named(tn).unwrap_or(0);
                    self.variant(eid, name, &[])
                }
                _ => format!("{}.f_{name}", self.expr(base)),
            },
            ExprKind::StructLit(name, inits) => self.struct_lit(name, inits),
            ExprKind::IfExpr(branches, last) => self.if_expr(self.ty(e), branches, last),
            ExprKind::Mut(x) => format!("(&{})", self.expr(x)),
            ExprKind::Float(_) | ExprKind::Index(..) | ExprKind::Missing => "0".into(),
            // part 2b: `emit` stops with #error before any of these is reached
            ExprKind::NoneVal
            | ExprKind::SomeVal(_)
            | ExprKind::ErrVal(_)
            | ExprKind::OkVal(_)
            | ExprKind::Try(..) => "0".into(),
        }
    }

    /// `if` as a value: only the chosen branch runs, its statements first.
    fn if_expr(
        &mut self,
        t: Ty,
        branches: &'a [(Expr, ValueBlock)],
        last: &'a ValueBlock,
    ) -> String {
        let r = format!("r{}", self.uid);
        self.uid += 1;
        let saved = std::mem::take(&mut self.out);
        self.nested += 1;
        let _ = writeln!(self.out, "({{ {} {r};", self.c_ty(t));
        for (k, (c, vb)) in branches.iter().enumerate() {
            let c = self.expr(c);
            let kw = if k == 0 { "if" } else { "else if" };
            let _ = writeln!(self.out, "{kw} {} {{", paren(&c));
            self.value_block(vb, &r);
        }
        let _ = writeln!(self.out, "else {{");
        self.value_block(last, &r);
        let _ = write!(self.out, "{r}; }})");
        self.nested -= 1;
        std::mem::replace(&mut self.out, saved)
    }

    fn value_block(&mut self, vb: &'a ValueBlock, r: &str) {
        for s in &vb.stmts {
            self.stmt(s, 1);
        }
        let v = self.expr(&vb.value);
        let _ = writeln!(self.out, "{r} = {v};\n}}");
    }

    /// `T{f: v, …}`: written fields left to right, then the defaults of the missing
    /// ones in declaration order (R5).
    fn struct_lit(&mut self, name: &'a str, inits: &'a [FieldInit]) -> String {
        let s = format!("l{}", self.uid);
        self.uid += 1;
        let mut c = format!("({{ d_{name} {s} = {{0}}; ");
        for i in inits {
            let v = self.expr(&i.value);
            let _ = write!(c, "{s}.f_{} = {v}; ", i.name);
        }
        if let Some(decl) = self.structs.get(name) {
            for f in &decl.fields {
                if let (Some(d), false) = (&f.default, inits.iter().any(|i| i.name == f.name)) {
                    let v = self.expr(d);
                    let _ = write!(c, "{s}.f_{} = {v}; ", f.name);
                }
            }
        }
        let _ = write!(c, "{s}; }})");
        c
    }

    /// `E.V` or `E.V(a, b)`.
    fn variant(&mut self, eid: u32, name: &str, args: &'a [Expr]) -> String {
        let k = self.variant_index(eid, name);
        let t = self.c_ty(Ty::Named(eid));
        if args.is_empty() {
            return format!("(({t}){{.tag = {k}}})");
        }
        let s = format!("l{}", self.uid);
        self.uid += 1;
        let mut c = format!("({{ {t} {s} = {{0}}; {s}.tag = {k}; ");
        for (j, a) in args.iter().enumerate() {
            let v = self.expr(a);
            let _ = write!(c, "{s}.u.v{k}.f{j} = {v}; ");
        }
        let _ = write!(c, "{s}; }})");
        c
    }

    /// Operands left to right (R29): when one of them calls something, all of them go
    /// into temporaries first (a call can change a place another operand reads).
    fn ordered(&mut self, items: &[Arg<'a>]) -> (String, Vec<String>) {
        let calls = items
            .iter()
            .any(|a| matches!(a, Arg::Val(e) if has_call(e)));
        let mut pre = String::new();
        let mut vals = Vec::new();
        for a in items {
            match *a {
                Arg::Addr(e) => vals.push(format!("(&{})", self.expr(e))),
                Arg::Val(e) => {
                    let v = self.expr(e);
                    let plain = matches!(e.kind, ExprKind::Int(_) | ExprKind::Bool(_))
                        || self.info.folded.contains_key(&(e.pos.offset, e.end));
                    if calls && items.len() >= 2 && !plain {
                        let t = self.temp(self.c_ty(self.ty(e)), "t");
                        let _ = write!(pre, "{t} = {v}, ");
                        vals.push(t);
                    } else {
                        vals.push(v);
                    }
                }
            }
        }
        (pre, vals)
    }

    fn op(&mut self, op: &str, t: Ty, a: String, b: String, p: Pos) -> String {
        let at = format!("{}, {}", p.line, p.col);
        match (op, t) {
            ("+", Ty::Str) => format!("dif_cat({a}, {b})"),
            ("==", Ty::Str | Ty::Named(_)) => self.eq(t, &a, &b),
            ("!=", Ty::Str | Ty::Named(_)) => format!("(!{})", self.eq(t, &a, &b)),
            ("<" | "<=" | ">" | ">=", Ty::Str) => format!("(dif_cmp({a}, {b}) {op} 0)"),
            ("+", _) => format!("dif_add({a}, {b}, {at})"),
            ("-", _) => format!("dif_sub({a}, {b}, {at})"),
            ("*", _) => format!("dif_mul({a}, {b}, {at})"),
            ("/", _) => format!("dif_div({a}, {b}, {at})"),
            ("%", _) => format!("dif_mod({a}, {b}, {at})"),
            ("<<", _) => format!("dif_shl({a}, {b}, {at})"),
            (">>", _) => format!("dif_shr({a}, {b}, {at})"),
            ("+%", _) => format!("dif_wadd({a}, {b})"),
            ("-%", _) => format!("dif_wsub({a}, {b})"),
            ("*%", _) => format!("dif_wmul({a}, {b})"),
            _ => format!("({a} {op} {b})"),
        }
    }

    fn call(&mut self, e: &'a Expr, f: &'a Expr, args: &'a [Expr]) -> String {
        let _ = e;
        match &f.kind {
            ExprKind::Field(base, m) => {
                if let ExprKind::Ident(tn) = &base.kind {
                    if let Some(eid) = self.enum_named(tn) {
                        return self.variant(eid, m, args);
                    }
                }
                self.method_call(base, m, args)
            }
            ExprKind::Ident(name) if name == "print" => {
                let t = self.ty(&args[0]);
                let v = self.expr(&args[0]);
                match t {
                    Ty::Int => format!("dif_print_int({v})"),
                    Ty::Bool => format!("dif_print_bool({v})"),
                    Ty::Named(id) => format!("dif_print_{}({v})", self.tname(id)),
                    _ => format!("dif_print_str({v})"),
                }
            }
            ExprKind::Ident(name) => {
                let Some(&func) = self.funcs.get(name.as_str()) else {
                    return "0".into();
                };
                let items = Self::args_of(func, args, None);
                self.apply(&Self::cname(func), &items)
            }
            _ => "0".into(),
        }
    }

    fn method_call(&mut self, base: &'a Expr, m: &str, args: &'a [Expr]) -> String {
        let Ty::Named(id) = self.ty(base) else {
            return "0".into();
        };
        let Some(&func) = self.methods.get(&(self.tname(id), m)) else {
            return "0".into();
        };
        let recv = if self.changes_self(func) {
            Arg::Addr(base)
        } else {
            Arg::Val(base)
        };
        let items = Self::args_of(func, args, Some(recv));
        self.apply(&Self::cname(func), &items)
    }

    /// The operands of a call: `mut` parameters take the place after `mut`.
    fn args_of(func: &Func, args: &'a [Expr], recv: Option<Arg<'a>>) -> Vec<Arg<'a>> {
        let mut items: Vec<Arg<'a>> = recv.into_iter().collect();
        for (p, a) in func.params.iter().zip(args) {
            items.push(match (&a.kind, p.mut_pos) {
                (ExprKind::Mut(x), Some(_)) => Arg::Addr(x),
                _ => Arg::Val(a),
            });
        }
        items
    }

    fn apply(&mut self, name: &str, items: &[Arg<'a>]) -> String {
        let (pre, vals) = self.ordered(items);
        let call = format!("{name}({})", vals.join(", "));
        if pre.is_empty() {
            call
        } else {
            format!("({pre}{call})")
        }
    }

    /// `"a {x} b"`: a fresh buffer, filled left to right.
    fn text(&mut self, segs: &'a [StrSeg]) -> String {
        if let [StrSeg::Lit(s)] = segs {
            return self.str_lit(s);
        }
        if segs.is_empty() {
            return "((dif_str){\"\", 0})".into();
        }
        let b = self.temp("dif_buf".into(), "b");
        let mut parts = vec![format!("{b} = dif_buf_new()")];
        for s in segs {
            match s {
                StrSeg::Lit(s) => {
                    let l = Self::lit(&format!("&{b}"), s);
                    parts.push(l.trim_end_matches(';').to_string());
                }
                StrSeg::Expr(x) => {
                    let t = self.ty(x);
                    let v = self.expr(x);
                    let p = self.put(t, &format!("&{b}"), &v, false);
                    parts.push(p.trim_end_matches(';').to_string());
                }
            }
        }
        parts.push(format!("dif_take(&{b})"));
        format!("({})", parts.join(", "))
    }
}

/// `c` as the parenthesized condition of `if`/`while`; clang warns about `((a == b))`.
fn paren(c: &str) -> String {
    let (mut depth, mut quoted, mut escaped) = (0, false, false);
    for (i, ch) in c.char_indices() {
        if quoted {
            match (escaped, ch) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => quoted = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => quoted = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return if i + 1 == c.len() {
                        c.to_string()
                    } else {
                        format!("({c})")
                    };
                }
            }
            _ if depth == 0 => break,
            _ => {}
        }
    }
    format!("({c})")
}
