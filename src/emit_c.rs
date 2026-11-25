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

/// A literal needs no line in the failure report of `expect` (R18).
fn is_literal(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) | ExprKind::NoneVal => true,
        ExprKind::Str(segs) => segs.iter().all(|s| matches!(s, StrSeg::Lit(_))),
        ExprKind::Unary("-", x) | ExprKind::Paren(x) => is_literal(x),
        _ => false,
    }
}

/// Test binary only. A failure is one line on stderr, `dif-test-fail`, tab-separated
/// (`\`, tab, newline escaped), and exit code 3; `dif test` reads it (R18).
const TEST_RUNTIME: &str = r#"
static inline void dif_esc(dif_buf *b, const char *p, int64_t n) {
    for (int64_t i = 0; i < n; i++) {
        switch (p[i]) {
        case '\\': dif_put(b, "\\\\", 2); break;
        case '\n': dif_put(b, "\\n", 2); break;
        case '\t': dif_put(b, "\\t", 2); break;
        case '\r': dif_put(b, "\\r", 2); break;
        default: dif_put(b, &p[i], 1);
        }
    }
}
static _Noreturn void dif_fail(dif_buf *b) {
    dif_put(b, "\n", 1);
    fflush(stdout);
    fwrite(b->p, 1, (size_t)b->n, stderr);
    exit(3);
}
static void dif_fail_head(dif_buf *b, const char *kind, int line, int col, int64_t off) {
    char t[96];
    int n = snprintf(t, sizeof t, "dif-test-fail\t%s\t%d\t%d\t%" PRId64 "\t", kind, line, col, off);
    dif_put(b, t, n);
}
static _Noreturn void dif_try_fail(int line, int col, int64_t off, dif_str msg) {
    dif_buf b = dif_buf_new();
    dif_fail_head(&b, "try", line, col, off);
    dif_esc(&b, msg.p, msg.n);
    dif_fail(&b);
}
static _Noreturn void dif_expect_fail(int line, int col, int64_t off, const char *cond, int n,
                                      const char *const *texts, const dif_str *vals, const bool *done) {
    dif_buf b = dif_buf_new();
    dif_fail_head(&b, "expect", line, col, off);
    dif_esc(&b, cond, (int64_t)strlen(cond));
    for (int i = 0; i < n; i++) {
        bool seen = false;
        for (int j = 0; j < i; j++) {
            if (done[j] && strcmp(texts[j], texts[i]) == 0) seen = true;
        }
        if (!done[i] || seen) continue;
        dif_put(&b, "\t", 1);
        dif_esc(&b, texts[i], (int64_t)strlen(texts[i]));
        dif_put(&b, "\t", 1);
        dif_esc(&b, vals[i].p, vals[i].n);
    }
    dif_fail(&b);
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

/// C source for a module that passed the checker; `test` blocks are left out.
pub fn emit(file: &str, src: &str, m: &Module, info: &Info) -> String {
    emitter(file, src, info).module(m, false)
}

/// C source of the test binary: `./prog <k>` runs test number `k` (R18). `main` is not
/// called, and a module without `main` is fine.
pub fn emit_tests(file: &str, src: &str, m: &Module, info: &Info) -> String {
    emitter(file, src, info).module(m, true)
}

fn emitter<'a>(file: &str, src: &'a str, info: &'a Info) -> Emitter<'a> {
    Emitter {
        file: c_string(file.as_bytes()),
        src,
        ret: Ty::Void,
        in_test: false,
        slots: Vec::new(),
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
    }
}

struct Emitter<'a> {
    file: String,
    src: &'a str,
    /// Return type of the current function (`void` in a test).
    ret: Ty,
    in_test: bool,
    /// Texts of the operands of the `expect` being lowered, in source order.
    slots: Vec<String>,
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

/// Can evaluating `e` have an effect that order matters for (R29)? A call can change a
/// place; `try` can leave the function, so it must happen before whatever follows it.
fn has_call(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Call(..) | ExprKind::IfExpr(..) | ExprKind::Try(..) => true,
        ExprKind::Unary(_, x)
        | ExprKind::Paren(x)
        | ExprKind::Mut(x)
        | ExprKind::Field(x, _)
        | ExprKind::SomeVal(x)
        | ExprKind::ErrVal(x)
        | ExprKind::OkVal(Some(x)) => has_call(x),
        ExprKind::Binary(_, l, r, _) | ExprKind::Index(l, r) => has_call(l) || has_call(r),
        ExprKind::StructLit(_, inits) => inits.iter().any(|i| has_call(&i.value)),
        ExprKind::Str(segs) => segs
            .iter()
            .any(|s| matches!(s, StrSeg::Expr(x) if has_call(x))),
        // no `_` arm: a new expression kind must be classified here (R29)
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

/// An operand of a call: its value (with the type of its place, so `T` becomes
/// `?T`/`T!E` there), or the address of a place (`mut`, a receiver that is changed).
#[derive(Clone, Copy)]
enum Arg<'a> {
    Val(&'a Expr, Ty),
    Addr(&'a Expr),
}

impl<'a> Emitter<'a> {
    // ---- types ----

    fn ty_of(&self, t: &TypeRef) -> Ty {
        self.ty_named(&t.name)
    }

    /// A written type: `int`, `Point`, `?int`, `int!E`, `?int!E` (`!E` binds weakest).
    fn ty_named(&self, n: &str) -> Ty {
        if let Some((t, e)) = n.split_once('!') {
            let want = (self.ty_named(t), self.ty_named(e));
            return (self.info.ress.iter().position(|&x| x == want))
                .map_or(Ty::Err, |i| Ty::Res(i as u32));
        }
        if let Some(t) = n.strip_prefix('?') {
            let want = self.ty_named(t);
            return (self.info.opts.iter().position(|&x| x == want))
                .map_or(Ty::Err, |i| Ty::Opt(i as u32));
        }
        match n {
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
            Ty::Opt(i) => format!("dif_o{i}"),
            Ty::Res(i) => format!("dif_r{i}"),
            t => c_ty_prim(t).into(),
        }
    }

    /// Name part of the helpers (`dif_eq_…`, `dif_put_…`, `dif_print_…`) of a type.
    fn sfx(&self, t: Ty) -> String {
        match t {
            Ty::Named(id) => self.info.types[id as usize].name.clone(),
            Ty::Opt(i) => format!("__o{i}"),
            Ty::Res(i) => format!("__r{i}"),
            _ => String::new(),
        }
    }

    /// Does the type have helpers (a struct, an enum, `?T`, `T!E`)?
    fn composite(t: Ty) -> bool {
        matches!(t, Ty::Named(_) | Ty::Opt(_) | Ty::Res(_))
    }

    /// `T` of `T!E` is `void`: the C struct has no value member.
    fn void_res(&self, t: Ty) -> bool {
        matches!(t, Ty::Res(i) if self.info.ress[i as usize].0 == Ty::Void)
    }

    /// The checked type of an expression.
    fn ty(&self, e: &Expr) -> Ty {
        self.info
            .exprs
            .get(&(e.pos.offset, e.end))
            .copied()
            .unwrap_or(Ty::Err)
    }

    /// `v` of type `got` where `want` is expected: a `T` becomes `some(T)` or `ok(T)` (R31).
    fn wrap(&self, want: Ty, got: Ty, v: String) -> String {
        if want == got || got == Ty::Err {
            return v;
        }
        match want {
            Ty::Opt(_) => format!("(({}){{.some = true, .v = {v}}})", self.c_ty(want)),
            Ty::Res(_) => format!("(({}){{.ok = true, .v = {v}}})", self.c_ty(want)),
            _ => v,
        }
    }

    /// `e` at a place of type `want`.
    fn at(&mut self, want: Ty, e: &'a Expr) -> String {
        let v = self.expr(e);
        self.wrap(want, self.ty(e), v)
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

    /// The types of the fields of variant `k` of enum `eid`.
    fn variant_fields(&self, eid: u32, k: usize) -> Vec<Ty> {
        match &self.info.types[eid as usize].body {
            TypeBody::Enum(vs) => vs[k].1.iter().map(|f| f.0).collect(),
            TypeBody::Struct(_) => Vec::new(),
        }
    }

    fn field_ty(&self, sname: &str, field: &str) -> Ty {
        let body = self.info.types.iter().find(|d| d.name == sname);
        match body.map(|d| &d.body) {
            Some(TypeBody::Struct(fs)) => fs.iter().find(|f| f.0 == field).map_or(Ty::Err, |f| f.1),
            _ => Ty::Err,
        }
    }

    // ---- type definitions and their helpers (equality, text) ----

    /// Every type of the module, after the types it contains (a value has a fixed size, R5).
    fn type_defs(&self) -> String {
        let mut done = HashSet::new();
        let mut c = String::new();
        let all = (0..self.info.types.len() as u32)
            .map(Ty::Named)
            .chain((0..self.info.opts.len() as u32).map(Ty::Opt))
            .chain((0..self.info.ress.len() as u32).map(Ty::Res));
        for t in all {
            self.type_def(t, &mut done, &mut c);
        }
        c
    }

    fn type_def(&self, t: Ty, done: &mut HashSet<Ty>, c: &mut String) {
        if !Self::composite(t) || !done.insert(t) {
            return;
        }
        let members = |fs: &mut dyn Iterator<Item = (Ty, String)>| -> Vec<String> {
            fs.map(|(t, n)| format!("{} {n};", self.c_ty(t))).collect()
        };
        let (contained, body): (Vec<Ty>, String) = match t {
            Ty::Opt(i) => {
                let v = self.info.opts[i as usize];
                (vec![v], format!("bool some; {} v;", self.c_ty(v)))
            }
            Ty::Res(i) => {
                let (v, e) = self.info.ress[i as usize];
                let value = if v == Ty::Void {
                    String::new()
                } else {
                    format!(" {} v;", self.c_ty(v))
                };
                (vec![v, e], format!("bool ok;{value} {} e;", self.c_ty(e)))
            }
            Ty::Named(i) => {
                let d = &self.info.types[i as usize];
                let contained: Vec<Ty> = match &d.body {
                    TypeBody::Struct(fs) => fs.iter().map(|f| f.1).collect(),
                    TypeBody::Enum(vs) => vs.iter().flat_map(|v| v.1.iter().map(|f| f.0)).collect(),
                };
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
                                    &mut v
                                        .1
                                        .iter()
                                        .enumerate()
                                        .map(|(j, f)| (f.0, format!("f{j}"))),
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
                (contained, body)
            }
            _ => return,
        };
        for u in contained {
            self.type_def(u, done, c);
        }
        let _ = writeln!(c, "typedef struct {{ {body} }} {};", self.c_ty(t));
    }

    /// Every type that has helpers.
    fn composites(&self) -> Vec<Ty> {
        let n = &self.info;
        (0..n.types.len() as u32)
            .map(Ty::Named)
            .chain((0..n.opts.len() as u32).map(Ty::Opt))
            .chain((0..n.ress.len() as u32).map(Ty::Res))
            .collect()
    }

    /// Prototypes of the helpers of every type.
    fn helper_protos(&self) -> String {
        let mut c = String::new();
        for t in self.composites() {
            let (n, ct) = (self.sfx(t), self.c_ty(t));
            let _ = writeln!(c, "static inline bool dif_eq_{n}({ct} a, {ct} b);");
            let _ = writeln!(c, "static inline void dif_put_{n}(dif_buf *b, {ct} v);");
            let _ = writeln!(c, "static inline void dif_print_{n}({ct} v);");
        }
        c
    }

    /// `a == b` for values of type `t`.
    fn eq(&self, t: Ty, a: &str, b: &str) -> String {
        match t {
            Ty::Str => format!("dif_eq({a}, {b})"),
            t if Self::composite(t) => format!("dif_eq_{}({a}, {b})", self.sfx(t)),
            _ => format!("({a} == {b})"),
        }
    }

    /// A statement appending the text of `v` to buffer `b`; a `str` inside a
    /// struct or enum is quoted (R28).
    fn put(&self, t: Ty, b: &str, v: &str, quoted: bool) -> String {
        match t {
            Ty::Int => format!("dif_put_int({b}, {v});"),
            Ty::Bool => format!("dif_put_bool({b}, {v});"),
            t if Self::composite(t) => format!("dif_put_{}({b}, {v});", self.sfx(t)),
            _ if quoted => format!("dif_put_quoted({b}, {v});"),
            _ => format!("dif_put_str({b}, {v});"),
        }
    }

    fn lit(b: &str, s: &str) -> String {
        format!("dif_put({b}, {}, {});", c_string(s.as_bytes()), s.len())
    }

    fn helpers(&self) -> String {
        let mut c = String::new();
        for t in self.composites() {
            let (n, ct) = (self.sfx(t), self.c_ty(t));
            let _ = writeln!(c, "static inline bool dif_eq_{n}({ct} a, {ct} b) {{");
            let _ = writeln!(c, "    (void)a;\n    (void)b;");
            let mut text = String::new();
            match t {
                Ty::Opt(i) => {
                    let v = self.info.opts[i as usize];
                    let same = self.eq(v, "a.v", "b.v");
                    let _ = writeln!(c, "    return a.some == b.some && (!a.some || {same});\n}}");
                    let _ = writeln!(
                        text,
                        "    if (!v.some) {{\n        {}",
                        Self::lit("b", "none")
                    );
                    let _ = writeln!(text, "        return;\n    }}");
                    let _ = writeln!(text, "    {}", Self::lit("b", "some("));
                    let _ = writeln!(text, "    {}", self.put(v, "b", "v.v", true));
                    let _ = writeln!(text, "    {}", Self::lit("b", ")"));
                }
                Ty::Res(i) => {
                    let (v, e) = self.info.ress[i as usize];
                    let err = self.eq(e, "a.e", "b.e");
                    let same = if v == Ty::Void {
                        format!("(a.ok || {err})")
                    } else {
                        format!("(a.ok ? {} : {err})", self.eq(v, "a.v", "b.v"))
                    };
                    let _ = writeln!(c, "    return a.ok == b.ok && {same};\n}}");
                    let _ = writeln!(text, "    if (v.ok) {{");
                    let _ = writeln!(text, "        {}", Self::lit("b", "ok"));
                    if v != Ty::Void {
                        let _ = writeln!(text, "        {}", Self::lit("b", "("));
                        let _ = writeln!(text, "        {}", self.put(v, "b", "v.v", true));
                        let _ = writeln!(text, "        {}", Self::lit("b", ")"));
                    }
                    let _ = writeln!(text, "        return;\n    }}");
                    let _ = writeln!(text, "    {}", Self::lit("b", "err("));
                    let _ = writeln!(text, "    {}", self.put(e, "b", "v.e", true));
                    let _ = writeln!(text, "    {}", Self::lit("b", ")"));
                }
                Ty::Named(id) => {
                    let d = &self.info.types[id as usize];
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
                            let _ = writeln!(text, "    {}", Self::lit("b", &format!("{n}{{")));
                            for (k, (f, t)) in fs.iter().enumerate() {
                                let sep = if k == 0 { "" } else { ", " };
                                let _ = writeln!(
                                    text,
                                    "    {}",
                                    Self::lit("b", &format!("{sep}{f}: "))
                                );
                                let _ = writeln!(
                                    text,
                                    "    {}",
                                    self.put(*t, "b", &format!("v.f_{f}"), true)
                                );
                            }
                            let _ = writeln!(text, "    {}", Self::lit("b", "}"));
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
                                let _ = writeln!(
                                    c,
                                    "    if (a.tag == {k}) return {};",
                                    all.join(" && ")
                                );
                            }
                            let _ = writeln!(c, "    return true;\n}}");
                            for (k, v) in vs.iter().enumerate() {
                                let _ = writeln!(text, "    if (v.tag == {k}) {{");
                                let _ = writeln!(
                                    text,
                                    "        {}",
                                    Self::lit("b", &format!("{n}.{}", v.0))
                                );
                                if !v.1.is_empty() {
                                    let _ = writeln!(text, "        {}", Self::lit("b", "("));
                                }
                                for (j, f) in v.1.iter().enumerate() {
                                    let sep = if j == 0 { "" } else { ", " };
                                    let _ = writeln!(text, "        {}", Self::lit("b", sep));
                                    let x = format!("v.u.v{k}.f{j}");
                                    let _ =
                                        writeln!(text, "        {}", self.put(f.0, "b", &x, true));
                                }
                                if !v.1.is_empty() {
                                    let _ = writeln!(text, "        {}", Self::lit("b", ")"));
                                }
                                let _ = writeln!(text, "    }}");
                            }
                        }
                    }
                }
                _ => {}
            }
            let _ = writeln!(c, "static inline void dif_put_{n}(dif_buf *b, {ct} v) {{");
            let _ = writeln!(c, "    (void)v;");
            c.push_str(&text);
            let _ = writeln!(c, "}}");
            let _ = writeln!(c, "static inline void dif_print_{n}({ct} v) {{");
            let _ = writeln!(c, "    dif_buf b = dif_buf_new();");
            let _ = writeln!(c, "    dif_put_{n}(&b, v);");
            let _ = writeln!(c, "    dif_print_str(dif_take(&b));\n}}");
        }
        c
    }

    // ---- module ----

    fn module(&mut self, m: &'a Module, tests: bool) -> String {
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
        if tests {
            c.push_str(TEST_RUNTIME);
        }
        for d in &m.decls {
            match d {
                Decl::Const(v) => {
                    // a function, so consts may use each other in any order
                    self.temps.clear();
                    let t = self.ty_of(&v.ty);
                    let value = self.at(t, &v.value);
                    let _ = writeln!(
                        c,
                        "static inline {} d_{}(void) {{\n{}    return {value};\n}}",
                        self.c_ty(t),
                        v.name,
                        self.temp_decls()
                    );
                }
                Decl::Func(f) => c.push_str(&self.func(f)),
                _ => {}
            }
        }
        if tests {
            let mut runs = String::new();
            for (k, d) in m
                .decls
                .iter()
                .filter_map(|d| match d {
                    Decl::Test(t) => Some(t),
                    _ => None,
                })
                .enumerate()
            {
                c.push_str(&self.test_fn(k, d));
                let _ = writeln!(runs, "    case {k}: dt_{k}(); break;");
            }
            let _ = write!(
                c,
                "int main(int argc, char **argv) {{\n    switch (argc > 1 ? atoi(argv[1]) : -1) {{\n{runs}    default: return 4;\n    }}\n    return 0;\n}}\n"
            );
            return c;
        }
        // `void!E main()`: an error ends the program with `error: <text>` and exit code 1 (R12)
        let main_ty = self
            .funcs
            .get("main")
            .map_or(Ty::Void, |f| self.ty_of(&f.ret));
        if let Ty::Res(i) = main_ty {
            let e = self.info.ress[i as usize].1;
            let text = self.put(e, "&b", "r.e", false);
            let _ = write!(
                c,
                "int main(void) {{\n    {} r = d_main();\n    if (!r.ok) {{\n        dif_buf b = dif_buf_new();\n        {text}\n        fflush(stdout);\n        fputs(\"error: \", stderr);\n        fwrite(b.p, 1, (size_t)b.n, stderr);\n        fputc('\\n', stderr);\n        return 1;\n    }}\n    return 0;\n}}\n",
                self.c_ty(main_ty)
            );
        } else {
            c.push_str("int main(void) {\n    d_main();\n    return 0;\n}\n");
        }
        c
    }

    /// One `test` block as `static void dt_<k>(void)`.
    fn test_fn(&mut self, k: usize, t: &'a TestDecl) -> String {
        self.start_fn(Ty::Void, true);
        self.block_body(&t.body, 1);
        let mut c = format!("static void dt_{k}(void) {{\n");
        c.push_str(&self.temp_decls());
        c.push_str(&self.out);
        c.push_str("}\n");
        c
    }

    fn start_fn(&mut self, ret: Ty, in_test: bool) {
        self.temps.clear();
        self.out.clear();
        self.derefs.clear();
        self.lows.clear();
        self.steady.clear();
        self.ret = ret;
        self.in_test = in_test;
    }

    fn tname(&self, id: u32) -> &'a str {
        &self.info.types[id as usize].name
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
        let ret = self.ty_of(&f.ret);
        self.start_fn(ret, false);
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
        if self.void_res(ret) {
            // a `void!E` function that reaches its end succeeded
            let _ = writeln!(c, "    return (({}){{.ok = true}});", self.c_ty(ret));
        } else if ret != Ty::Void {
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
                let ty = self.ty_of(&v.ty);
                let value = self.at(ty, &v.value);
                let t = self.c_ty(ty);
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
                    let t = self.ty(place);
                    self.at(t, value)
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
            Stmt::Return(None, _) if self.void_res(self.ret) => {
                let r = self.c_ty(self.ret);
                self.line(d, &format!("return (({r}){{.ok = true}});"));
            }
            Stmt::Return(None, _) => self.line(d, "return;"),
            Stmt::Return(Some(e), _) => {
                let v = self.at(self.ret, e);
                self.line(d, &format!("return {v};"));
            }
            Stmt::Call(e) => {
                let v = self.expr(e);
                // `try f()` may produce a value that is not used
                let cast = if matches!(e.kind, ExprKind::Try(..)) {
                    "(void)"
                } else {
                    ""
                };
                self.line(d, &format!("{cast}{v};"));
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
            if let [Pattern::Result {
                kind,
                bind: Some(Some(b)),
                ..
            }] = case.pats.as_slice()
            {
                let member = if *kind == "err" { "e" } else { "v" };
                let t = self.c_ty(self.ty_of(&b.ty));
                self.line(d + 2, &format!("{t} d_{} = {s}.{member};", b.name));
                self.line(d + 2, &format!("(void)d_{};", b.name));
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
            Pattern::Result { kind, .. } => match *kind {
                "ok" => format!("{s}.ok"),
                "err" => format!("!{s}.ok"),
                "some" => format!("{s}.some"),
                _ => format!("!{s}.some"),
            },
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
                if *op == "??" {
                    return self.coalesce(e, l, r);
                }
                if *op == "-" && t == Ty::Int && self.safe_sub(l, r) {
                    return format!("({} - {})", self.expr(l), self.expr(r));
                }
                let (pre, vals) = self.ordered(&[Arg::Val(l, t), Arg::Val(r, self.ty(r))]);
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
            ExprKind::NoneVal => format!("(({}){{0}})", self.c_ty(self.ty(e))),
            ExprKind::SomeVal(x) => match self.ty(e) {
                t @ Ty::Opt(i) => {
                    let v = self.at(self.info.opts[i as usize], x);
                    format!("(({}){{.some = true, .v = {v}}})", self.c_ty(t))
                }
                _ => "0".into(),
            },
            ExprKind::ErrVal(x) => match self.ty(e) {
                t @ Ty::Res(i) => {
                    let v = self.at(self.info.ress[i as usize].1, x);
                    format!("(({}){{.e = {v}}})", self.c_ty(t))
                }
                _ => "0".into(),
            },
            ExprKind::Try(x, or) => self.try_expr(e, x, or.as_ref().map(|o| &*o.1)),
            // only a pattern (R12): the checker rejects it as a value
            ExprKind::OkVal(_) => "0".into(),
        }
    }

    /// `a ?? b`: `b` only when `a` has no value (R12).
    fn coalesce(&mut self, e: &'a Expr, l: &'a Expr, r: &'a Expr) -> String {
        let (lt, t) = (self.ty(l), self.ty(e));
        let y = format!("y{}", self.uid);
        self.uid += 1;
        let has = if matches!(lt, Ty::Res(_)) {
            "ok"
        } else {
            "some"
        };
        let (v, alt) = (self.expr(l), self.at(t, r));
        format!(
            "({{ {} {y} = {v}; {y}.{has} ? {y}.v : {alt}; }})",
            self.c_ty(lt)
        )
    }

    /// `try x [or alt]`: the value, or an early return of the failure (R12). In a
    /// test a failure ends the test (R18).
    fn try_expr(&mut self, e: &'a Expr, x: &'a Expr, or: Option<&'a Expr>) -> String {
        let xt = self.ty(x);
        let y = format!("y{}", self.uid);
        self.uid += 1;
        let (v, res) = (self.expr(x), matches!(xt, Ty::Res(_)));
        let (ok, p) = (if res { "ok" } else { "some" }, e.pos);
        let leave = if self.in_test {
            let text = match xt {
                Ty::Res(i) => {
                    let put =
                        self.put(self.info.ress[i as usize].1, "&b", &format!("{y}.e"), false);
                    format!("dif_buf b = dif_buf_new(); {put}")
                }
                _ => "dif_buf b = dif_buf_new(); dif_put(&b, \"none\", 4);".to_string(),
            };
            format!(
                "{{ {text} dif_try_fail({}, {}, {}, dif_take(&b)); }}",
                p.line, p.col, p.offset
            )
        } else {
            let r = self.c_ty(self.ret);
            match (self.ret, or) {
                (Ty::Res(i), Some(alt)) => {
                    let alt = self.at(self.info.ress[i as usize].1, alt);
                    format!("return (({r}){{.e = {alt}}});")
                }
                (Ty::Res(_), None) => format!("return (({r}){{.e = {y}.e}});"),
                _ => format!("return (({r}){{0}});"),
            }
        };
        let value = if self.ty(e) == Ty::Void {
            "(void)0;".to_string()
        } else {
            format!("{y}.v;")
        };
        format!(
            "({{ {} {y} = {v}; if (!{y}.{ok}) {leave} {value} }})",
            self.c_ty(xt)
        )
    }

    /// `expect(cond)` (R18): the condition is read as a tree; every operand that is not a
    /// literal is evaluated once and its text recorded, so a failure can show it.
    fn expect(&mut self, e: &'a Expr, cond: &'a Expr) -> String {
        let id = self.uid;
        self.uid += 1;
        let saved = std::mem::take(&mut self.slots);
        let c = self.cond_tree(cond, id);
        let texts = std::mem::replace(&mut self.slots, saved);
        let n = texts.len() + 1;
        let list: Vec<String> = texts.iter().map(|t| c_string(t.as_bytes())).collect();
        let list = if list.is_empty() {
            "\"\"".into()
        } else {
            list.join(", ")
        };
        let text = c_string(self.text_of(cond).as_bytes());
        let p = e.pos;
        format!(
            "({{ dif_str xv{id}[{n}]; bool xd{id}[{n}] = {{false}}; const char *const xt{id}[{n}] = {{{list}}}; if (!{c}) dif_expect_fail({}, {}, {}, {text}, {}, xt{id}, xv{id}, xd{id}); }})",
            p.line,
            p.col,
            p.offset,
            texts.len()
        )
    }

    /// Source text of an expression, line breaks as spaces.
    fn text_of(&self, e: &Expr) -> String {
        let s = self
            .src
            .get(e.pos.offset as usize..e.end as usize)
            .unwrap_or("");
        s.replace('\n', " ")
    }

    /// The boolean skeleton of an `expect` condition: `&&`, `||`, `!`, parentheses and
    /// comparisons; anything else is a leaf.
    fn cond_tree(&mut self, e: &'a Expr, id: usize) -> String {
        match &e.kind {
            ExprKind::Paren(x) => self.cond_tree(x, id),
            ExprKind::Unary("!", x) => format!("(!{})", self.cond_tree(x, id)),
            ExprKind::Binary(op @ ("&&" | "||"), l, r, _) => {
                format!("({} {op} {})", self.cond_tree(l, id), self.cond_tree(r, id))
            }
            ExprKind::Binary(op @ ("==" | "!=" | "<" | "<=" | ">" | ">="), l, r, opos) => {
                let t = self.ty(l);
                let (a, b) = (format!("a{}", self.uid), format!("b{}", self.uid));
                self.uid += 1;
                let (la, va) = self.operand(l, &a, id);
                let (lb, vb) = self.operand(r, &b, id);
                let v = self.op(op, t, va, vb, *opos);
                format!("({{ {la} {lb} {v}; }})")
            }
            _ => {
                let y = format!("a{}", self.uid);
                self.uid += 1;
                let (decl, v) = self.operand(e, &y, id);
                format!("({{ {decl} {v}; }})")
            }
        }
    }

    /// An operand of a comparison: statements that evaluate it once and record its text,
    /// and the name of the value. Literals are used as they are.
    fn operand(&mut self, e: &'a Expr, name: &str, id: usize) -> (String, String) {
        if is_literal(e) {
            return (String::new(), self.expr(e));
        }
        let t = self.ty(e);
        let k = self.slots.len();
        self.slots.push(self.text_of(e));
        let v = self.expr(e);
        let put = self.put(t, "&bb", name, true);
        let decl = format!(
            "{} {name} = {v}; {{ dif_buf bb = dif_buf_new(); {put} xv{id}[{k}] = dif_take(&bb); }} xd{id}[{k}] = true;",
            self.c_ty(t)
        );
        (decl, name.to_string())
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
            self.value_block(vb, &r, t);
        }
        let _ = writeln!(self.out, "else {{");
        self.value_block(last, &r, t);
        let _ = write!(self.out, "{r}; }})");
        self.nested -= 1;
        std::mem::replace(&mut self.out, saved)
    }

    fn value_block(&mut self, vb: &'a ValueBlock, r: &str, t: Ty) {
        for s in &vb.stmts {
            self.stmt(s, 1);
        }
        let v = self.at(t, &vb.value);
        let _ = writeln!(self.out, "{r} = {v};\n}}");
    }

    /// `T{f: v, …}`: written fields left to right, then the defaults of the missing
    /// ones in declaration order (R5).
    fn struct_lit(&mut self, name: &'a str, inits: &'a [FieldInit]) -> String {
        let s = format!("l{}", self.uid);
        self.uid += 1;
        let mut c = format!("({{ d_{name} {s} = {{0}}; ");
        for i in inits {
            let v = self.at(self.field_ty(name, &i.name), &i.value);
            let _ = write!(c, "{s}.f_{} = {v}; ", i.name);
        }
        if let Some(decl) = self.structs.get(name) {
            for f in &decl.fields {
                if let (Some(d), false) = (&f.default, inits.iter().any(|i| i.name == f.name)) {
                    let v = self.at(self.field_ty(name, &f.name), d);
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
        let fields = self.variant_fields(eid, k);
        for (j, a) in args.iter().enumerate() {
            let v = self.at(fields[j], a);
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
            .any(|a| matches!(a, Arg::Val(e, _) if has_call(e)));
        let mut pre = String::new();
        let mut vals = Vec::new();
        for a in items {
            match *a {
                Arg::Addr(e) => vals.push(format!("(&{})", self.expr(e))),
                Arg::Val(e, want) => {
                    let v = self.at(want, e);
                    let plain = matches!(
                        e.kind,
                        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::NoneVal
                    ) || self.info.folded.contains_key(&(e.pos.offset, e.end));
                    if calls && items.len() >= 2 && !plain {
                        let t = self.temp(self.c_ty(want), "t");
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
            ("==", Ty::Str) => self.eq(t, &a, &b),
            ("!=", Ty::Str) => format!("(!{})", self.eq(t, &a, &b)),
            ("==", t) if Self::composite(t) => self.eq(t, &a, &b),
            ("!=", t) if Self::composite(t) => format!("(!{})", self.eq(t, &a, &b)),
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
                    t if Self::composite(t) => format!("dif_print_{}({v})", self.sfx(t)),
                    _ => format!("dif_print_str({v})"),
                }
            }
            ExprKind::Ident(name) if name == "expect" && self.in_test => self.expect(e, &args[0]),
            ExprKind::Ident(name) => {
                let Some(&func) = self.funcs.get(name.as_str()) else {
                    return "0".into();
                };
                let items = self.args_of(func, args, None);
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
            Arg::Val(base, self.ty(base))
        };
        let items = self.args_of(func, args, Some(recv));
        self.apply(&Self::cname(func), &items)
    }

    /// The operands of a call: `mut` parameters take the place after `mut`.
    fn args_of(&self, func: &Func, args: &'a [Expr], recv: Option<Arg<'a>>) -> Vec<Arg<'a>> {
        let mut items: Vec<Arg<'a>> = recv.into_iter().collect();
        for (p, a) in func.params.iter().zip(args) {
            items.push(match (&a.kind, p.mut_pos) {
                (ExprKind::Mut(x), Some(_)) => Arg::Addr(x),
                _ => Arg::Val(a, self.ty_of(&p.ty)),
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
