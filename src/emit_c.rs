//! C11 output for a checked module: one C file per program (spec §6, R14, R29).
//! Names get the prefix `d_`; `#line` points every statement at the `.dif` source.

use std::collections::HashMap;
use std::fmt::Write;

use crate::ast::*;
use crate::checker::{Folded, Ty};
use crate::diag::Pos;

const RUNTIME: &str = r#"#include <inttypes.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { const char *p; int64_t n; } dif_str;
typedef struct { char *p; int64_t n, cap; } dif_buf;

static inline _Noreturn void dif_panic(const char *msg, int line, int col) {
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
"#;

/// C source for a module that passed the checker.
pub fn emit(file: &str, m: &Module, folded: &Folded) -> String {
    let mut e = Emitter {
        file: c_string(file.as_bytes()),
        folded,
        funcs: HashMap::new(),
        consts: HashMap::new(),
        scopes: Vec::new(),
        temps: Vec::new(),
        out: String::new(),
    };
    e.module(m)
}

struct Emitter<'a> {
    file: String,
    folded: &'a Folded,
    funcs: HashMap<&'a str, Ty>,
    consts: HashMap<&'a str, Ty>,
    scopes: Vec<HashMap<&'a str, Ty>>,
    /// Temporaries of the current function: C type and name.
    temps: Vec<(&'static str, String)>,
    out: String,
}

fn ty_of(t: &TypeRef) -> Ty {
    match t.name.as_str() {
        "int" => Ty::Int,
        "bool" => Ty::Bool,
        "str" => Ty::Str,
        _ => Ty::Void,
    }
}

fn c_ty(t: Ty) -> &'static str {
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
        ExprKind::Call(..) => true,
        ExprKind::Unary(_, x) | ExprKind::Paren(x) => has_call(x),
        ExprKind::Binary(_, l, r, _) => has_call(l) || has_call(r),
        ExprKind::Str(segs) => segs
            .iter()
            .any(|s| matches!(s, StrSeg::Expr(x) if has_call(x))),
        _ => false,
    }
}

impl<'a> Emitter<'a> {
    fn module(&mut self, m: &'a Module) -> String {
        let mut c = format!("static const char *const dif_file = {};\n", self.file);
        c.push_str(RUNTIME);
        for d in &m.decls {
            match d {
                Decl::Func(f) => {
                    self.funcs.insert(&f.name, ty_of(&f.ret));
                }
                Decl::Const(v) => {
                    self.consts.insert(&v.name, ty_of(&v.ty));
                }
            }
        }
        // prototypes, so the order of declarations does not matter
        for d in &m.decls {
            if let Decl::Func(f) = d {
                let _ = writeln!(c, "{};", self.header(f));
            }
        }
        for d in &m.decls {
            match d {
                Decl::Const(v) => {
                    // a function, so consts may use each other in any order
                    self.temps.clear();
                    let value = self.expr(&v.value);
                    let t = c_ty(ty_of(&v.ty));
                    let _ = writeln!(
                        c,
                        "static inline {t} d_{}(void) {{\n{}    return {value};\n}}",
                        v.name,
                        self.temp_decls()
                    );
                }
                Decl::Func(f) => c.push_str(&self.func(f)),
            }
        }
        c.push_str("int main(void) {\n    d_main();\n    return 0;\n}\n");
        c
    }

    fn header(&self, f: &Func) -> String {
        let params: Vec<String> = f
            .params
            .iter()
            .map(|p| format!("{} d_{}", c_ty(ty_of(&p.ty)), p.name))
            .collect();
        let params = if params.is_empty() {
            "void".into()
        } else {
            params.join(", ")
        };
        format!("{} d_{}({params})", c_ty(ty_of(&f.ret)), f.name)
    }

    fn temp_decls(&mut self) -> String {
        self.temps
            .iter()
            .map(|(t, n)| format!("    {t} {n};\n"))
            .collect()
    }

    fn temp(&mut self, t: &'static str, prefix: &str) -> String {
        let name = format!("{prefix}{}", self.temps.len());
        self.temps.push((t, name.clone()));
        name
    }

    fn func(&mut self, f: &'a Func) -> String {
        self.temps.clear();
        self.out.clear();
        self.scopes.push(
            f.params
                .iter()
                .map(|p| (p.name.as_str(), ty_of(&p.ty)))
                .collect(),
        );
        for p in &f.params {
            let _ = writeln!(self.out, "    (void)d_{};", p.name);
        }
        self.block_body(&f.body, 1);
        self.scopes.pop();
        let mut c = format!("{} {{\n", self.header(f));
        c.push_str(&self.temp_decls());
        c.push_str(&self.out);
        if f.ret.name != "void" {
            c.push_str("    __builtin_unreachable();\n");
        }
        c.push_str("}\n");
        c
    }

    fn line(&mut self, depth: usize, text: &str) {
        let _ = writeln!(self.out, "{}{text}", "    ".repeat(depth));
    }

    fn block(&mut self, b: &'a Block, depth: usize) {
        self.line(depth - 1, "{");
        self.block_body(b, depth);
        self.line(depth - 1, "}");
    }

    fn block_body(&mut self, b: &'a Block, depth: usize) {
        self.scopes.push(HashMap::new());
        for s in &b.stmts {
            self.stmt(s, depth);
        }
        self.scopes.pop();
    }

    fn stmt(&mut self, s: &'a Stmt, d: usize) {
        let pos = match s {
            Stmt::Var(v) => v.pos,
            Stmt::Assign { pos, .. } | Stmt::If { pos, .. } | Stmt::While { pos, .. } => *pos,
            Stmt::For { pos, .. } | Stmt::Break(pos) | Stmt::Continue(pos) => *pos,
            Stmt::Return(_, pos) | Stmt::Discard(_, pos) => *pos,
            Stmt::Call(e) => e.pos,
        };
        let _ = writeln!(self.out, "#line {} {}", pos.line, self.file);
        match s {
            Stmt::Var(v) => {
                let t = ty_of(&v.ty);
                let value = self.expr(&v.value);
                if let Some(s) = self.scopes.last_mut() {
                    s.insert(&v.name, t);
                }
                self.line(d, &format!("{} d_{} = {value};", c_ty(t), v.name));
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
                    self.line(d, &format!("{kw} ({c})"));
                    self.block(b, d + 1);
                }
                if let Some(b) = otherwise {
                    self.line(d, "else");
                    self.block(b, d + 1);
                }
            }
            Stmt::While { cond, body, .. } => {
                let c = self.expr(cond);
                self.line(d, &format!("while ({c})"));
                self.block(body, d + 1);
            }
            Stmt::For {
                vars, iter, body, ..
            } => self.for_range(vars, iter, body, d),
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
            self.temp("int64_t", "t"),
            self.temp("int64_t", "t"),
            self.temp("int64_t", "t"),
        );
        self.line(d, &format!("{l} = {lo};"));
        self.line(d, &format!("{h} = {hi};"));
        self.line(d, &format!("for ({i} = {l}; {i} < {h}; {i}++) {{"));
        let v = &vars[0];
        self.scopes
            .push(HashMap::from([(v.name.as_str(), Ty::Int)]));
        self.line(d + 1, &format!("int64_t d_{} = {i};", v.name));
        self.line(d + 1, &format!("(void)d_{};", v.name));
        self.block(body, d + 2);
        self.scopes.pop();
        self.line(d, "}");
    }

    // ---- expressions ----

    /// The type of a checked expression.
    fn ty(&self, e: &Expr) -> Ty {
        match &e.kind {
            ExprKind::Int(_) => Ty::Int,
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Str(_) => Ty::Str,
            ExprKind::Ident(n) => self
                .scopes
                .iter()
                .rev()
                .find_map(|s| s.get(n.as_str()).copied())
                .or_else(|| self.consts.get(n.as_str()).copied())
                .unwrap_or(Ty::Err),
            ExprKind::Paren(x) => self.ty(x),
            ExprKind::Unary("!", _) => Ty::Bool,
            ExprKind::Unary(..) => Ty::Int,
            ExprKind::Binary(op, l, _, _) => match *op {
                "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" => Ty::Bool,
                _ => self.ty(l),
            },
            ExprKind::Call(f, _) => match &f.kind {
                ExprKind::Ident(n) => self.funcs.get(n.as_str()).copied().unwrap_or(Ty::Void),
                _ => Ty::Err,
            },
            _ => Ty::Err,
        }
    }

    fn expr(&mut self, e: &'a Expr) -> String {
        if let Some(v) = self.folded.get(&(e.pos.offset, e.end)) {
            return c_int(*v);
        }
        match &e.kind {
            ExprKind::Int(v) => c_int(*v as i64),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Str(segs) => self.text(segs),
            ExprKind::Ident(n)
                if self.consts.contains_key(n.as_str())
                    && !self.scopes.iter().any(|s| s.contains_key(n.as_str())) =>
            {
                format!("d_{n}()")
            }
            ExprKind::Ident(n) => format!("d_{n}"),
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
                let (pre, vals) = self.ordered(&[l, r]);
                let [a, b] = <[String; 2]>::try_from(vals).unwrap_or_default();
                let v = self.op(op, t, a, b, *opos);
                if pre.is_empty() {
                    v
                } else {
                    format!("({pre}{v})")
                }
            }
            ExprKind::Call(f, args) => self.call(f, args),
            _ => "0".into(), // not produced by the checker in this part
        }
    }

    /// Operands left to right (R29): when two of them call functions, the earlier
    /// ones go into temporaries first.
    fn ordered(&mut self, items: &[&'a Expr]) -> (String, Vec<String>) {
        let calls = items.iter().filter(|e| has_call(e)).count();
        let mut pre = String::new();
        let mut vals = Vec::new();
        for e in items {
            let v = self.expr(e);
            if calls >= 2 {
                let t = self.temp(c_ty(self.ty(e)), "t");
                let _ = write!(pre, "{t} = {v}, ");
                vals.push(t);
            } else {
                vals.push(v);
            }
        }
        (pre, vals)
    }

    fn op(&mut self, op: &str, t: Ty, a: String, b: String, p: Pos) -> String {
        let at = format!("{}, {}", p.line, p.col);
        match (op, t) {
            ("+", Ty::Str) => format!("dif_cat({a}, {b})"),
            ("==", Ty::Str) => format!("dif_eq({a}, {b})"),
            ("!=", Ty::Str) => format!("(!dif_eq({a}, {b}))"),
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

    fn call(&mut self, f: &'a Expr, args: &'a [Expr]) -> String {
        let ExprKind::Ident(name) = &f.kind else {
            return "0".into();
        };
        if name == "print" {
            let t = self.ty(&args[0]);
            let v = self.expr(&args[0]);
            let kind = match t {
                Ty::Int => "int",
                Ty::Bool => "bool",
                _ => "str",
            };
            return format!("dif_print_{kind}({v})");
        }
        let refs: Vec<&Expr> = args.iter().collect();
        let (pre, vals) = self.ordered(&refs);
        let call = format!("d_{name}({})", vals.join(", "));
        if pre.is_empty() {
            call
        } else {
            format!("({pre}{call})")
        }
    }

    /// `"a {x} b"`: a fresh buffer, filled left to right.
    fn text(&mut self, segs: &'a [StrSeg]) -> String {
        if let [StrSeg::Lit(s)] = segs {
            return format!("((dif_str){{{}, {}}})", c_string(s.as_bytes()), s.len());
        }
        if segs.is_empty() {
            return "((dif_str){\"\", 0})".into();
        }
        let b = self.temp("dif_buf", "b");
        let mut parts = vec![format!("{b} = dif_buf_new()")];
        for s in segs {
            match s {
                StrSeg::Lit(s) => parts.push(format!(
                    "dif_put(&{b}, {}, {})",
                    c_string(s.as_bytes()),
                    s.len()
                )),
                StrSeg::Expr(x) => {
                    let kind = match self.ty(x) {
                        Ty::Int => "int",
                        Ty::Bool => "bool",
                        _ => "str",
                    };
                    let v = self.expr(x);
                    parts.push(format!("dif_put_{kind}(&{b}, {v})"));
                }
            }
        }
        parts.push(format!("dif_take(&{b})"));
        format!("({})", parts.join(", "))
    }
}
