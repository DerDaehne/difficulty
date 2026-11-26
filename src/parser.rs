//! Recursive-descent parser (spec §2, R1–R4, R11, R13 grammar).
//!
//! One cause, one diagnostic (spec §0.1): every statement and declaration is a
//! unit that reports at most its first error; the statements of a block inside it
//! are units of their own. Foreign spellings are recognized as a whole. After an
//! error the rest of the line and a directly following block are skipped. A
//! lexical error that breaks the token stream (L1, L5) silences the parser error
//! it causes in its unit.

use crate::ast::*;
use crate::diag::{Diag, Pos};
use crate::lexer::{StrPart, Tok, Token};

/// Deepest nesting of blocks and expressions; deeper input is S1, not a stack overflow.
const MAX_DEPTH: u32 = 256;

/// Most binary operators in one expression (a chain is a deep tree as well).
const MAX_OPS: u32 = 1024;

pub fn parse(src: &str, toks: &[Token], lex: &[Diag]) -> (Module, Vec<Diag>) {
    let breaks = lex
        .iter()
        .filter(|d| matches!(d.code, "L1" | "L5"))
        .map(|d| d.pos.offset)
        .collect();
    let mut p = Parser::new(src, toks, breaks);
    let m = p.module();
    (m, p.out)
}

const ASSIGN_OPS: &[&str] = &[
    "=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=",
];

const COMPARISONS: &[&str] = &["==", "!=", "<", "<=", ">", ">="];

const FOREIGN_FUNC: &[&str] = &["def", "fn", "function", "func"];

const FOREIGN_VAR: &[&str] = &["var", "let", "auto"];

/// Standard modules: `io.IoErr` is a type name with a dot (R16).
const MODULES: &[&str] = &["io", "os"];

fn level(op: &str) -> Option<u8> {
    Some(match op {
        "??" => 1,
        "||" => 2,
        "&&" => 3,
        "==" | "!=" | "<" | "<=" | ">" | ">=" => 4,
        "|" => 5,
        "^" => 6,
        "&" => 7,
        "<<" | ">>" => 8,
        "+" | "-" | "+%" | "-%" => 9,
        "*" | "/" | "%" | "*%" => 10,
        _ => return None,
    })
}

fn is_binary(e: &Expr, ops: &[&str]) -> bool {
    matches!(&e.kind, ExprKind::Binary(op, _, _, _) if ops.contains(op))
}

struct Parser<'a> {
    src: &'a str,
    t: &'a [Token],
    i: usize,
    last_end: u32,
    /// Inside the header of `if`/`while`/`for`/`switch`: `T {` starts the body (R3).
    header: bool,
    /// Offsets of lexical errors that break the token stream.
    breaks: Vec<u32>,
    /// First error of each open unit, with the offset up to which it was seen.
    frames: Vec<Option<(Diag, u32)>>,
    /// Above 0 while a top-level statement is parsed: only its R1 counts.
    silent: u32,
    depth: u32,
    too_deep: bool,
    /// Operators in the current top-level expression and its nesting level.
    ops: u32,
    expr_level: u32,
    unclosed: bool,
    /// Reserved words already reported as names; their uses are not reported again.
    reserved: Vec<&'static str>,
    out: Vec<Diag>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, t: &'a [Token], breaks: Vec<u32>) -> Self {
        Parser {
            src,
            t,
            i: 0,
            last_end: 0,
            header: false,
            breaks,
            frames: Vec::new(),
            silent: 0,
            depth: 0,
            too_deep: false,
            ops: 0,
            expr_level: 0,
            unclosed: false,
            reserved: Vec::new(),
            out: Vec::new(),
        }
    }

    // ---- tokens ----

    fn tok(&self, n: usize) -> &'a Token {
        &self.t[(self.i + n).min(self.t.len() - 1)]
    }

    fn peek(&self) -> &'a Tok {
        &self.tok(0).tok
    }

    fn peek_n(&self, n: usize) -> &'a Tok {
        &self.tok(n).tok
    }

    fn pos(&self) -> Pos {
        self.tok(0).pos
    }

    fn bump(&mut self) {
        let tok = self.tok(0);
        if tok.tok != Tok::Eof {
            self.i += 1;
            self.last_end = tok.end;
        }
    }

    fn is_op(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Op(o) if *o == s)
    }

    fn is_kw(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Kw(k) if *k == s)
    }

    fn word_at(&self, n: usize, words: &[&str]) -> bool {
        matches!(self.peek_n(n), Tok::Ident(w) if words.contains(&w.as_str()))
    }

    fn skip_nl(&mut self) {
        while *self.peek() == Tok::Nl {
            self.bump();
        }
    }

    fn text(&self, from: u32, to: u32) -> &'a str {
        self.src.get(from as usize..to as usize).unwrap_or("")
    }

    fn expr_text(&self, e: &Expr) -> &'a str {
        self.text(e.pos.offset, e.end)
    }

    fn missing(&self, pos: Pos) -> Expr {
        Expr {
            kind: ExprKind::Missing,
            pos,
            end: pos.offset,
        }
    }

    // ---- diagnostics ----

    fn err(&mut self, code: &'static str, pos: Pos, msg: impl Into<String>, hint: Option<String>) {
        let mut d = Diag::new(code, pos, msg);
        d.hint = hint;
        self.report(d);
    }

    /// Keeps the first error of the current unit; outside a unit it is emitted.
    fn report(&mut self, d: Diag) {
        let seen = self.tok(0).end;
        match self.frames.last_mut() {
            Some(f) => {
                if f.is_none() {
                    *f = Some((d, seen));
                }
            }
            None => self.emit(d),
        }
    }

    fn emit(&mut self, d: Diag) {
        if self.silent == 0 {
            self.out.push(d);
        }
    }

    /// Parses one statement or declaration, reports at most its first error and
    /// ends its line (§0.1).
    fn unit<T>(&mut self, f: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        let start = self.pos().offset;
        self.frames.push(None);
        let r = f(self);
        if matches!(self.frames.last(), Some(Some(_))) {
            self.recover();
        }
        self.end_of_stmt();
        if let Some((d, seen)) = self.frames.pop().flatten() {
            if !self.breaks.iter().any(|&b| b >= start && b < seen) {
                self.emit(d);
            }
        }
        r
    }

    /// Skips to the end of the line at bracket depth 0 (not past the `}` of the
    /// enclosing block), then a block that directly follows on the next line.
    fn recover(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::Nl if depth == 0 => break,
                Tok::Op("}") if depth == 0 => return,
                Tok::Op("(" | "[" | "{") => depth += 1,
                Tok::Op(")" | "]" | "}") => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.bump();
        }
        if *self.peek_n(1) == Tok::Op("{") {
            self.bump();
            self.skip_block();
        }
    }

    fn skip_block(&mut self) {
        if !self.is_op("{") {
            return;
        }
        let mut depth = 0usize;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::Op("{") => depth += 1,
                Tok::Op("}") => {
                    depth -= 1;
                    if depth == 0 {
                        self.bump();
                        return;
                    }
                }
                _ => {}
            }
            self.bump();
        }
    }

    /// After a statement: end of line, `}` or end of file.
    fn end_of_stmt(&mut self) {
        if self.i > 0 && self.t[self.i - 1].tok == Tok::Nl {
            return; // the line already ended (a block closed early)
        }
        match self.peek() {
            Tok::Nl => {}
            Tok::Eof | Tok::Op("}") => return,
            Tok::Semi => {
                let p = self.pos();
                let hint = "statements end at the end of the line".to_string();
                self.err("L3", p, "`;` is not used in difficulty", Some(hint));
                self.recover();
            }
            t => {
                let p = self.pos();
                if can_start_stmt(t) {
                    let hint = "put each statement on its own line".to_string();
                    self.err("R3", p, "two statements on one line", Some(hint));
                } else {
                    let hint = match t {
                        Tok::Op("=") => Some(
                            "an assignment has no value; assign each variable on its own line"
                                .into(),
                        ),
                        Tok::Op(")" | "]") => {
                            Some("it has no matching opening bracket; remove it".into())
                        }
                        Tok::Kw("or") => Some("or belongs to try: try f() or E.V(…)".into()),
                        _ => None,
                    };
                    self.err("S1", p, format!("unexpected {}", describe(t)), hint);
                }
                self.recover();
            }
        }
        self.skip_nl();
    }

    fn enter(&mut self) -> bool {
        self.depth += 1;
        if self.depth <= MAX_DEPTH {
            return true;
        }
        if !self.too_deep {
            self.too_deep = true;
            let p = self.pos();
            let hint = "move inner parts into functions or variables".to_string();
            self.err(
                "S1",
                p,
                format!("nesting is deeper than {MAX_DEPTH} levels"),
                Some(hint),
            );
        }
        false
    }

    /// Too deep inside an expression: skips the rest of the operand, up to the end
    /// of the line, a `{`, or a closing bracket of an enclosing level.
    fn skip_operand(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::Nl | Tok::Op("{" | ")" | "]" | "}") if depth == 0 => return,
                Tok::Op("(" | "[") => depth += 1,
                Tok::Op(")" | "]") => depth -= 1,
                _ => {}
            }
            self.bump();
        }
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn expect_op(&mut self, s: &str) -> bool {
        if self.is_op(s) {
            self.bump();
            return true;
        }
        let p = self.pos();
        let found = describe(self.peek());
        self.err("S1", p, format!("expected `{s}`, found {found}"), None);
        false
    }

    /// The closing bracket of `open`; a missing one is reported at the opening one
    /// when the next token is on a later line.
    fn close(&mut self, open: Pos, c: &str) -> bool {
        if self.is_op(c) {
            self.bump();
            return true;
        }
        let prev_line = self.t[self.i.saturating_sub(1)].pos.line;
        if *self.peek() == Tok::Eof || self.pos().line > prev_line {
            let o = if c == ")" { "(" } else { "[" };
            self.err(
                "S1",
                open,
                format!("`{o}` is never closed"),
                Some(format!("add `{c}`")),
            );
        } else {
            self.expect_op(c);
        }
        false
    }

    /// A name, or a reserved word in the place of a name (`bool ok = true`).
    fn name(&mut self) -> Option<(String, Pos)> {
        let p = self.pos();
        match self.peek() {
            Tok::Ident(n) => {
                self.bump();
                Some((n.clone(), p))
            }
            Tok::Kw(k) if !matches!(*k, "in" | "or") => {
                let k = *k;
                self.bump();
                let msg = format!("`{k}` is a reserved word and cannot be a name");
                self.err("S1", p, msg, Some("choose another name".into()));
                if !self.reserved.contains(&k) {
                    self.reserved.push(k);
                }
                Some((k.to_string(), p))
            }
            other => {
                let msg = format!("expected a name, found {}", describe(other));
                self.err("S1", p, msg, None);
                None
            }
        }
    }

    /// Is there a name at `n` (or a reserved word used as one, see `name`)?
    fn name_at(&self, n: usize) -> bool {
        match self.peek_n(n) {
            Tok::Ident(_) => true,
            Tok::Kw(k) => {
                !matches!(*k, "in" | "or")
                    && matches!(self.peek_n(n + 1), Tok::Op("=" | "(" | "," | ")"))
            }
            _ => false,
        }
    }

    /// Number of tokens of a type written here (`T`, `?T`, `T!E`, `[]T`, `[N]T`,
    /// `map[K]V`, …), 0 if none. Only looks; `type_ref` reads (and reports malformed types).
    fn type_len(&self) -> usize {
        self.type_end(0).unwrap_or(0)
    }

    fn type_end(&self, mut k: usize) -> Option<usize> {
        while k < 2 && matches!(self.peek_n(k), Tok::Op("?" | "??")) {
            k += 1;
        }
        k = self.base_end(k)?;
        while *self.peek_n(k) == Tok::Op("!") {
            match self.base_end(k + 1) {
                Some(e) => k = e,
                None => break,
            }
        }
        Some(k)
    }

    fn base_end(&self, k: usize) -> Option<usize> {
        match self.peek_n(k) {
            Tok::Op("[") => {
                let mut j = k + 1;
                if *self.peek_n(j) == Tok::Op("-") && matches!(self.peek_n(j + 1), Tok::Int(_)) {
                    j += 1;
                }
                if matches!(self.peek_n(j), Tok::Int(_) | Tok::Ident(_)) {
                    j += 1;
                }
                if *self.peek_n(j) != Tok::Op("]") {
                    return None;
                }
                self.elem_end(j + 1)
            }
            Tok::Ident(m) if m == "map" && *self.peek_n(k + 1) == Tok::Op("[") => {
                let j = self.elem_end(k + 2)?;
                if *self.peek_n(j) != Tok::Op("]") {
                    return None;
                }
                self.elem_end(j + 1)
            }
            Tok::Ident(m) if MODULES.contains(&m.as_str()) && self.qualified_at(k) => Some(k + 3),
            Tok::Ident(_) | Tok::Kw("void") => Some(k + 1),
            _ => None,
        }
    }

    fn elem_end(&self, k: usize) -> Option<usize> {
        if *self.peek_n(k) == Tok::Op("(") {
            let j = self.type_end(k + 1)?;
            return (*self.peek_n(j) == Tok::Op(")")).then_some(j + 1);
        }
        let k = if matches!(self.peek_n(k), Tok::Op("?")) {
            k + 1
        } else {
            k
        };
        self.base_end(k)
    }

    /// `io.IoErr`: a module name, a dot and a name.
    fn qualified_at(&self, k: usize) -> bool {
        *self.peek_n(k + 1) == Tok::Op(".") && matches!(self.peek_n(k + 2), Tok::Ident(_))
    }

    /// `T name` at the start of a statement: a declaration.
    fn decl_start(&self) -> bool {
        let k = self.type_len();
        k > 0 && self.name_at(k)
    }

    /// `T name(` starts a function; inside a block that means a missing `}`.
    fn func_start(&self) -> bool {
        let k = self.type_len();
        k > 0
            && !self.word_at(0, FOREIGN_FUNC)
            && matches!(self.peek_n(k), Tok::Ident(_))
            && *self.peek_n(k + 1) == Tok::Op("(")
    }

    // ---- module and declarations ----

    fn module(&mut self) -> Module {
        let mut decls = Vec::new();
        self.skip_nl();
        while *self.peek() != Tok::Eof {
            let before = self.i;
            if self.is_op("}") {
                let p = self.pos();
                let hint = "no block is open here; remove it".to_string();
                self.err("S1", p, "unexpected `}`", Some(hint));
                self.bump();
                self.skip_nl();
                continue;
            }
            self.unclosed = false;
            if let Some(d) = self.unit(|p| p.decl()) {
                decls.push(d);
            }
            if self.i == before {
                self.bump(); // never loop without progress
            }
        }
        Module { decls }
    }

    fn decl(&mut self) -> Option<Decl> {
        let start = self.pos();
        if self.is_kw("use") {
            return self.use_decl().map(Decl::Use);
        }
        if self.is_kw("const") {
            return self.var_decl(true).map(Decl::Const);
        }
        if self.word_at(0, FOREIGN_FUNC) && self.name_at(1) {
            self.foreign_func();
            return None;
        }
        if self.word_at(0, FOREIGN_VAR) && self.name_at(1) {
            return self.foreign_var(true).map(Decl::Const);
        }
        if self.is_kw("struct") {
            return self.struct_decl().map(Decl::Struct);
        }
        if self.is_kw("enum") {
            return self.enum_decl().map(Decl::Enum);
        }
        // `R T.name(` is a method
        let k = self.type_len();
        if k > 0
            && matches!(self.peek_n(k), Tok::Ident(_))
            && *self.peek_n(k + 1) == Tok::Op(".")
            && self.name_at(k + 2)
        {
            return self.func().map(Decl::Func);
        }
        if self.is_kw("test") {
            return self.test_decl().map(Decl::Test);
        }
        if self.decl_start() {
            if matches!(self.peek_n(k + 1), Tok::Op("(" | "{")) {
                return self.func().map(Decl::Func);
            }
            let v = self.var_decl(true)?;
            let msg = format!("variable `{}` outside a function", v.name);
            let hint = "make it const, or move it into a function".to_string();
            self.err("R4", v.pos, msg, Some(hint));
            return Some(Decl::Const(v));
        }
        // A statement at the top level: its R1 replaces whatever the statement reports.
        self.silent += 1;
        let _ = self.stmt();
        self.silent -= 1;
        let d = Diag::new("R1", start, "statement outside a function")
            .hint("move it into a function, e.g. void main() { … }");
        if let Some(f) = self.frames.last_mut() {
            *f = Some((d, start.offset));
        }
        None
    }

    /// `def/fn/function/func name …`: one R2; the header and its body are skipped,
    /// a `{ … }` body as well as indented lines below it (Python style).
    fn foreign_func(&mut self) {
        let p = self.pos();
        let Tok::Ident(w) = self.peek() else { return };
        let hint = "the return type comes first: int add(int a, int b)".to_string();
        self.err(
            "R2",
            p,
            format!("functions start with their return type, not `{w}`"),
            Some(hint),
        );
        while *self.peek() != Tok::Eof && !self.is_op("{") {
            if *self.peek() == Tok::Nl && *self.peek_n(1) != Tok::Op("{") {
                while *self.peek() == Tok::Nl && self.tok(1).pos.col > 1 {
                    self.bump();
                    while !matches!(self.peek(), Tok::Nl | Tok::Eof) {
                        self.bump();
                    }
                }
                return;
            }
            self.bump();
        }
        self.skip_block();
    }

    /// A type: `T`, `?T`, `T!E`, `[]T`, `[N]T`, `map[K]V`, `[](T!E)`; the text of the
    /// type without spaces is its name.
    fn type_ref(&mut self) -> TypeRef {
        let p = self.pos();
        let mut name = String::new();
        let mut nested = false;
        if !self.type_top(&mut name, &mut nested) {
            return TypeRef {
                name: "?".into(),
                pos: p,
            };
        }
        if nested {
            let hint = "a type has at most one ? and one !E".to_string();
            self.err("S1", p, "this type is nested too far", Some(hint));
            name = "?".into();
        }
        TypeRef { name, pos: p }
    }

    fn type_top(&mut self, name: &mut String, nested: &mut bool) -> bool {
        if self.is_op("?") || self.is_op("??") {
            *nested |= self.is_op("??");
            self.bump();
            name.push('?');
            if self.is_op("?") || self.is_op("??") {
                *nested = true;
                self.bump();
            }
        }
        if !self.base_type(name, nested) {
            return false;
        }
        let mut errs = 0;
        while self.is_op("!") {
            let at = self.pos();
            self.bump();
            if !matches!(self.peek(), Tok::Ident(_) | Tok::Kw("void") | Tok::Op("[")) {
                let msg = format!(
                    "expected the error type after `!`, found {}",
                    describe(self.peek())
                );
                self.err("S1", at, msg, None);
                return false;
            }
            errs += 1;
            let mut e = String::new();
            if !self.base_type(&mut e, nested) {
                return false;
            }
            if errs == 1 {
                name.push('!');
                name.push_str(&e);
            } else {
                *nested = true;
            }
        }
        true
    }

    fn base_type(&mut self, name: &mut String, nested: &mut bool) -> bool {
        match self.peek() {
            Tok::Op("[") => {
                self.bump();
                name.push('[');
                if self.is_op("-") && matches!(self.peek_n(1), Tok::Int(_)) {
                    name.push('-');
                    self.bump();
                }
                match self.peek() {
                    Tok::Int(v) => {
                        name.push_str(&v.to_string());
                        self.bump();
                    }
                    Tok::Ident(n) => {
                        name.push_str(n);
                        self.bump();
                    }
                    _ => {}
                }
                if !self.expect_op("]") {
                    return false;
                }
                name.push(']');
                self.elem_type(name, nested)
            }
            Tok::Ident(m) if m == "map" && *self.peek_n(1) == Tok::Op("[") => {
                self.bump();
                self.bump();
                name.push_str("map[");
                if !self.elem_type(name, nested) || !self.expect_op("]") {
                    return false;
                }
                name.push(']');
                self.elem_type(name, nested)
            }
            Tok::Ident(n) => {
                name.push_str(n);
                let qualified = MODULES.contains(&n.as_str()) && self.qualified_at(0);
                self.bump();
                if qualified {
                    self.bump();
                    if let Tok::Ident(q) = self.peek() {
                        name.push('.');
                        name.push_str(q);
                    }
                    self.bump();
                }
                true
            }
            Tok::Kw("void") => {
                name.push_str("void");
                self.bump();
                true
            }
            other => {
                let msg = format!("expected a type, found {}", describe(other));
                self.err("S1", self.pos(), msg, None);
                false
            }
        }
    }

    /// The element type of a list, array or map: `?T`, `T` or `(type)`.
    fn elem_type(&mut self, name: &mut String, nested: &mut bool) -> bool {
        if self.is_op("(") {
            let open = self.pos();
            self.bump();
            name.push('(');
            if !self.type_top(name, nested) || !self.close(open, ")") {
                return false;
            }
            name.push(')');
            return true;
        }
        if self.is_op("?") || self.is_op("??") {
            *nested |= self.is_op("??");
            self.bump();
            name.push('?');
        }
        self.base_type(name, nested)
    }

    fn func(&mut self) -> Option<Func> {
        let header = self.pos().offset;
        let mut ret = self.type_ref();
        let recv = if *self.peek_n(1) == Tok::Op(".") {
            let t = self.type_ref();
            self.bump(); // .
            Some(t)
        } else {
            None
        };
        let (name, pos) = self.name()?;
        let mut params = Vec::new();
        if self.is_op("{") {
            let p = self.pos();
            let hint = format!("{}() {{", self.text(header, self.last_end));
            self.err(
                "S1",
                p,
                format!("function `{name}` needs a parameter list"),
                Some(hint),
            );
        } else {
            let open = self.pos();
            self.bump(); // (
            if self.is_kw("void") && *self.peek_n(1) == Tok::Op(")") {
                let p = self.pos();
                let hint = format!("{}() {{", self.text(header, open.offset));
                self.err(
                    "S1",
                    p,
                    "an empty parameter list is written `()`",
                    Some(hint),
                );
                self.bump();
            }
            // `name: T` and `-> T` from other languages: one R2 with the whole header
            let mut foreign: Option<(Pos, &str)> = None;
            while !self.is_op(")") {
                let mut_pos = self.is_kw("mut").then(|| self.pos());
                if mut_pos.is_some() {
                    self.bump();
                }
                if matches!(self.peek(), Tok::Ident(_)) && *self.peek_n(1) == Tok::Op(":") {
                    let (n, p) = self.name()?;
                    foreign.get_or_insert((p, "the type comes before the name"));
                    self.bump(); // :
                    let ty = self.type_ref();
                    params.push(Param {
                        ty,
                        name: n,
                        pos: p,
                        mut_pos,
                    });
                } else {
                    let ty = self.type_ref();
                    let (n, p) = self.name()?;
                    params.push(Param {
                        ty,
                        name: n,
                        pos: p,
                        mut_pos,
                    });
                }
                if !self.is_op(",") {
                    break;
                }
                self.bump(); // a trailing comma is allowed
            }
            if !self.close(open, ")") {
                return None;
            }
            if self.is_op("-") && *self.peek_n(1) == Tok::Op(">") {
                foreign.get_or_insert((self.pos(), "the return type comes first"));
                self.bump();
                self.bump();
                ret = self.type_ref();
            }
            if let Some((p, msg)) = foreign {
                let list: Vec<String> = params
                    .iter()
                    .map(|q| format!("{} {}", q.ty.name, q.name))
                    .collect();
                let hint = format!("{} {}({}) {{", ret.name, name, list.join(", "));
                self.err("R2", p, msg, Some(hint));
            }
        }
        let body = self.body("function", header)?;
        Some(Func {
            ret,
            recv,
            name,
            pos,
            params,
            body,
        })
    }

    /// `[const] T name = value`. At the top level a hint keeps the `const`.
    fn var_decl(&mut self, top: bool) -> Option<VarDecl> {
        let is_const = self.is_kw("const");
        if is_const {
            self.bump();
        }
        let ty = self.type_ref();
        let (name, pos) = self.name()?;
        if !self.is_op("=") {
            let default = match ty.name.as_str() {
                "bool" => "false",
                "str" => "\"\"",
                _ => "0",
            };
            let c = if is_const || top { "const " } else { "" };
            let hint = format!("give it a value: {c}{} {name} = {default}", ty.name);
            self.err("R2", pos, format!("`{name}` needs a value"), Some(hint));
            let value = self.missing(pos);
            return Some(VarDecl {
                is_const,
                ty,
                name,
                pos,
                value,
            });
        }
        self.bump();
        let value = self.expr();
        Some(VarDecl {
            is_const,
            ty,
            name,
            pos,
            value,
        })
    }

    /// `var/let/auto x = v`: R2 with the derived type; it still counts as a declaration.
    fn foreign_var(&mut self, top: bool) -> Option<VarDecl> {
        let start = self.pos();
        self.bump();
        let (name, pos) = self.name()?;
        if !self.expect_op("=") {
            return None;
        }
        let value = self.expr();
        let ty = match &value.kind {
            ExprKind::Int(_) => Some("int"),
            ExprKind::Float(_) => Some("f64"),
            ExprKind::Bool(_) => Some("bool"),
            ExprKind::Str(_) => Some("str"),
            _ => None,
        };
        let c = if top { "const " } else { "" };
        let v = self.expr_text(&value);
        let hint = match ty {
            Some(t) => format!("{c}{t} {name} = {v}"),
            None => format!("write its type: {c}T {name} = {v}"),
        };
        self.err("R2", start, "declarations name their type", Some(hint));
        let ty = TypeRef {
            name: ty.unwrap_or("?").into(),
            pos: start,
        };
        Some(VarDecl {
            is_const: false,
            ty,
            name,
            pos,
            value,
        })
    }

    // ---- blocks and statements ----

    /// The body of a header; it must start on the header's line (R3).
    fn body(&mut self, what: &str, header_start: u32) -> Option<Block> {
        if self.is_op("=") && matches!(what, "if" | "else if" | "while") {
            let head = self.text(header_start, self.last_end);
            let p = self.pos();
            self.bump();
            let rhs = self.expr();
            let hint = format!("{head} == {} {{", self.expr_text(&rhs));
            self.err("S1", p, "`=` assigns; compare with `==`", Some(hint));
        }
        if self.is_op("{") {
            return Some(self.block());
        }
        let p = self.pos();
        let head = self.text(header_start, self.last_end);
        if *self.peek() == Tok::Nl && *self.peek_n(1) == Tok::Op("{") {
            let msg = format!("the body of `{what}` must start on the same line");
            self.err("R3", p, msg, Some(format!("{head} {{")));
            self.bump();
            return Some(self.block());
        }
        let msg = format!(
            "expected `{{` to start the body of `{what}`, found {}",
            describe(self.peek())
        );
        self.err(
            "R3",
            p,
            msg,
            Some(format!("put the body in braces: {head} {{ … }}")),
        );
        None
    }

    fn block(&mut self) -> Block {
        let pos = self.pos();
        let mut stmts = Vec::new();
        if !self.enter() {
            self.skip_block();
            self.leave();
            return Block { stmts, pos };
        }
        self.bump(); // {
        self.skip_nl();
        let mut cut = false; // the last statement was reported
        loop {
            let before = self.i;
            if self.is_op("}") {
                self.bump();
                break;
            }
            if *self.peek() == Tok::Eof || self.func_start() {
                // a statement cut off by the end of the file was reported already
                if !(cut && *self.peek() == Tok::Eof) {
                    self.never_closed(pos);
                }
                break;
            }
            let n = self.out.len();
            if let Some(s) = self.unit(|p| p.stmt()) {
                stmts.push(s);
            }
            cut = self.out.len() > n;
            if self.i == before {
                self.bump(); // never loop without progress
            }
        }
        self.leave();
        Block { stmts, pos }
    }

    /// Reported once, at the innermost open block; the outer ones share the cause.
    fn never_closed(&mut self, pos: Pos) {
        if self.unclosed {
            return;
        }
        self.unclosed = true;
        // a statement cut off by the end of the file was already reported there
        if *self.peek() == Tok::Eof && self.out.last().is_some_and(|d| d.pos == self.pos()) {
            return;
        }
        let hint = match self.peek() {
            Tok::Eof => "add `}`".to_string(),
            _ => format!("add `}}` before line {}", self.pos().line),
        };
        self.emit(Diag::new("S1", pos, "this block is never closed").hint(hint));
    }

    fn stmt(&mut self) -> Option<Stmt> {
        let p = self.pos();
        match self.peek() {
            Tok::Kw("if") => self.if_stmt(),
            Tok::Kw("switch") => self.switch_stmt(),
            Tok::Kw("while") => {
                self.bump();
                let cond = self.head_expr();
                let body = self.body("while", p.offset)?;
                Some(Stmt::While { cond, body, pos: p })
            }
            Tok::Kw("for") => self.for_stmt(),
            Tok::Kw("break") => {
                self.bump();
                Some(Stmt::Break(p))
            }
            Tok::Kw("continue") => {
                self.bump();
                Some(Stmt::Continue(p))
            }
            Tok::Kw("return") => {
                self.bump();
                let value = match self.peek() {
                    Tok::Nl | Tok::Eof | Tok::Semi | Tok::Op("}") => None,
                    _ => Some(self.expr()),
                };
                Some(Stmt::Return(value, p))
            }
            Tok::Kw("const") => self.var_decl(false).map(Stmt::Var),
            Tok::Kw("else") => {
                self.err("S1", p, "`else` without `if`", None);
                None
            }
            Tok::Ident(w) if w == "_" && *self.peek_n(1) == Tok::Op("=") => {
                self.bump();
                self.bump();
                let e = self.expr();
                Some(Stmt::Discard(e, p))
            }
            Tok::Ident(w) if w == "elif" || w == "elsif" => {
                let msg = format!("`{w}` does not exist");
                self.err("R3", p, msg, Some("use else if".into()));
                None
            }
            _ if self.word_at(0, FOREIGN_VAR) && self.name_at(1) => {
                self.foreign_var(false).map(Stmt::Var)
            }
            _ if self.word_at(0, FOREIGN_FUNC) && self.name_at(1) => {
                self.foreign_func();
                None
            }
            Tok::Kw("test") => {
                let hint = "move it out of the function".to_string();
                self.err("R18", p, "tests are declared at the top level", Some(hint));
                let _ = self.test_decl();
                None
            }
            _ if self.decl_start() => self.var_decl(false).map(Stmt::Var),
            _ => self.expr_stmt(),
        }
    }

    fn if_stmt(&mut self) -> Option<Stmt> {
        let pos = self.pos();
        self.bump();
        let cond = self.head_expr();
        let body = self.body("if", pos.offset)?;
        let mut branches = vec![(cond, body)];
        let mut otherwise = None;
        loop {
            let elif = |p: &Self, n| p.word_at(n, &["elif", "elsif"]);
            if *self.peek() == Tok::Nl && (*self.peek_n(1) == Tok::Kw("else") || elif(self, 1)) {
                if *self.peek_n(1) == Tok::Kw("else") {
                    let p = self.tok(1).pos;
                    let hint = "} else {".to_string();
                    self.err(
                        "R3",
                        p,
                        "`else` must be on the same line as `}`",
                        Some(hint),
                    );
                }
                self.skip_nl();
            }
            let p = self.pos();
            if elif(self, 0) {
                let Tok::Ident(w) = self.peek() else { break };
                self.err(
                    "R3",
                    p,
                    format!("`{w}` does not exist"),
                    Some("use else if".into()),
                );
                self.bump();
                let c = self.head_expr();
                let b = self.body("else if", p.offset)?;
                branches.push((c, b));
                continue;
            }
            if !self.is_kw("else") {
                break;
            }
            self.bump();
            if self.is_kw("if") {
                self.bump();
                let c = self.head_expr();
                let b = self.body("else if", p.offset)?;
                branches.push((c, b));
            } else {
                otherwise = Some(self.body("else", p.offset)?);
                break;
            }
        }
        Some(Stmt::If {
            branches,
            otherwise,
            pos,
        })
    }

    fn for_stmt(&mut self) -> Option<Stmt> {
        let pos = self.pos();
        self.bump();
        if self.is_op("(") {
            return self.c_for(pos);
        }
        let mut vars = Vec::new();
        let mut untyped = None;
        loop {
            let ty = if matches!(self.peek(), Tok::Ident(_))
                && matches!(self.peek_n(1), Tok::Kw("in") | Tok::Op(","))
            {
                untyped.get_or_insert(self.pos());
                TypeRef {
                    name: "?".into(),
                    pos: self.pos(),
                }
            } else {
                self.type_ref()
            };
            let (name, p) = self.name()?;
            vars.push(Param {
                ty,
                name,
                pos: p,
                mut_pos: None,
            });
            if vars.len() == 2 || !self.is_op(",") {
                break;
            }
            self.bump();
        }
        if !self.is_kw("in") {
            let p = self.pos();
            let msg = format!("expected `in`, found {}", describe(self.peek()));
            self.err("S1", p, msg, Some("for int i in range(n) { … }".into()));
            return None;
        }
        self.bump();
        let iter = self.head_expr();
        if let Some(p) = untyped {
            let range = matches!(&iter.kind, ExprKind::Call(f, _)
                if matches!(&f.kind, ExprKind::Ident(n) if n == "range"));
            let list: Vec<String> = vars
                .iter()
                .map(|v| match v.ty.name.as_str() {
                    "?" if range => format!("int {}", v.name),
                    "?" => format!("T {}", v.name),
                    t => format!("{t} {}", v.name),
                })
                .collect();
            let hint = format!("for {} in {}", list.join(", "), self.expr_text(&iter));
            self.err("R2", p, "loop variables name their type", Some(hint));
        }
        let body = self.body("for", pos.offset)?;
        Some(Stmt::For {
            vars,
            iter,
            body,
            pos,
        })
    }

    /// `for (init; cond; step) { … }`: one R11 with the matching `range` form.
    fn c_for(&mut self, pos: Pos) -> Option<Stmt> {
        let open = self.i;
        let mut depth = 0usize;
        let mut closed = false;
        loop {
            match self.peek() {
                // an unclosed header ends before the next block or line
                Tok::Eof | Tok::Nl | Tok::Op("{" | "}") => break,
                Tok::Op("(") => depth += 1,
                Tok::Op(")") => {
                    depth -= 1;
                    if depth == 0 {
                        self.bump();
                        closed = true;
                        break;
                    }
                }
                _ => {}
            }
            self.bump();
        }
        let end = if closed { self.i - 1 } else { self.i };
        let inner: Vec<&Tok> = self.t[open + 1..end].iter().map(|t| &t.tok).collect();
        self.err("R11", pos, "C-style for loop", Some(c_for_hint(&inner)));
        self.skip_block();
        None
    }

    fn expr_stmt(&mut self) -> Option<Stmt> {
        let e = self.expr();
        if let Tok::Op(op) = self.peek() {
            if ASSIGN_OPS.contains(op) {
                let op = *op;
                let pos = self.pos();
                self.bump();
                let value = self.expr();
                if matches!(e.kind, ExprKind::Missing) {
                    return None; // already reported
                }
                if !is_place(&e) {
                    let msg = format!("cannot assign to `{}`", self.expr_text(&e));
                    self.err("S1", e.pos, msg, None);
                    return None;
                }
                return Some(Stmt::Assign {
                    place: e,
                    op,
                    value,
                    pos,
                });
            }
        }
        match &e.kind {
            ExprKind::Call(..) => Some(Stmt::Call(e)),
            ExprKind::Try(x, _) if matches!(x.kind, ExprKind::Call(..)) => Some(Stmt::Call(e)),
            ExprKind::Missing => None,
            kind => {
                let text = self.expr_text(&e);
                let hint = match kind {
                    ExprKind::Ident(n) => format!("to call it, write {n}(…)"),
                    ExprKind::Binary("==", a, b, _) => {
                        format!(
                            "to assign, write {} = {}",
                            self.expr_text(a),
                            self.expr_text(b)
                        )
                    }
                    _ => "use the value: assign it, pass it to a function, or return it".into(),
                };
                let msg = format!("`{text}` does nothing as a statement");
                self.err("S4", e.pos, msg, Some(hint));
                None
            }
        }
    }

    // ---- expressions ----

    fn expr(&mut self) -> Expr {
        self.expr_level += 1;
        if self.expr_level == 1 {
            self.ops = 0;
        }
        let e = self.binary(1);
        self.expr_level -= 1;
        e
    }

    fn binary(&mut self, min: u8) -> Expr {
        let p = self.pos();
        if !self.enter() {
            self.leave();
            self.skip_operand();
            return self.missing(p);
        }
        let mut lhs = self.unary();
        while let Tok::Op(o) = self.peek() {
            let (op, lvl) = match level(o) {
                Some(l) if l >= min => (*o, l),
                _ => break,
            };
            let opos = self.pos();
            // a long chain is a deep tree for every later pass: bounded like nesting
            self.ops += 1;
            if self.ops > MAX_OPS {
                if self.ops == MAX_OPS + 1 {
                    let msg = format!("an expression has more than {MAX_OPS} operators");
                    let hint = "split it into several variables".to_string();
                    self.err("S1", opos, msg, Some(hint));
                }
                self.skip_operand();
                break;
            }
            self.bump();
            let rhs = self.binary(if op == "??" { lvl } else { lvl + 1 });
            if lvl == 4 && is_binary(&lhs, COMPARISONS) {
                lhs = self.chain(lhs, op, rhs, opos);
                continue;
            }
            if op == "||" && (is_binary(&lhs, &["&&"]) || is_binary(&rhs, &["&&"])) {
                let wrap = |e: &Expr| {
                    if is_binary(e, &["&&"]) {
                        format!("({})", self.expr_text(e))
                    } else {
                        self.expr_text(e).to_string()
                    }
                };
                let hint = format!("add parentheses: {} || {}", wrap(&lhs), wrap(&rhs));
                let msg = "`&&` and `||` are mixed without parentheses";
                self.err("S3", opos, msg, Some(hint));
            }
            let pos = lhs.pos;
            let end = rhs.end;
            lhs = Expr {
                kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs), opos),
                pos,
                end,
            };
        }
        self.leave();
        lhs
    }

    /// `a < b < c …`: one S2 for the whole chain, with every pair joined by `&&`.
    fn chain(&mut self, lhs: Expr, op: &'static str, rhs: Expr, opos: Pos) -> Expr {
        let ExprKind::Binary(op1, a, b, _) = &lhs.kind else {
            return lhs;
        };
        let mut terms = vec![self.expr_text(a), self.expr_text(b), self.expr_text(&rhs)];
        let mut ops = vec![*op1, op];
        let mut end = rhs.end;
        while let Tok::Op(o) = self.peek() {
            if !COMPARISONS.contains(o) {
                break;
            }
            ops.push(*o);
            self.bump();
            let e = self.binary(5);
            terms.push(self.expr_text(&e));
            end = e.end;
        }
        let pairs: Vec<String> = ops
            .iter()
            .enumerate()
            .map(|(k, o)| format!("{} {o} {}", terms[k], terms[k + 1]))
            .collect();
        let hint = format!("write {}", pairs.join(" && "));
        self.err("S2", opos, "comparisons cannot be chained", Some(hint));
        Expr {
            kind: ExprKind::Missing,
            pos: lhs.pos,
            end,
        }
    }

    fn unary(&mut self) -> Expr {
        if self.is_kw("try") {
            return self.try_expr();
        }
        let p = self.pos();
        let op = match self.peek() {
            Tok::Op(o @ ("-" | "!" | "~" | "++" | "--")) => *o,
            _ => return self.postfix(),
        };
        self.bump();
        if !self.enter() {
            self.leave();
            self.skip_operand();
            return self.missing(p);
        }
        let e = self.unary();
        self.leave();
        let end = e.end;
        if op == "++" || op == "--" {
            self.inc_dec(op, p, &e);
            return Expr {
                kind: ExprKind::Missing,
                pos: p,
                end,
            };
        }
        Expr {
            kind: ExprKind::Unary(op, Box::new(e)),
            pos: p,
            end,
        }
    }

    /// `x++`, `++x` (L7).
    fn inc_dec(&mut self, op: &'static str, p: Pos, e: &Expr) {
        let text = self.expr_text(e);
        let hint = if is_place(e) {
            Some(format!("use {text} {}= 1", &op[..1]))
        } else if op == "--" {
            Some(format!("write -(-{text})"))
        } else {
            None
        };
        self.err("L7", p, format!("`{op}` does not exist"), hint);
    }

    fn postfix(&mut self) -> Expr {
        let mut e = self.primary();
        let start = e.pos;
        loop {
            let open = self.pos();
            let kind = match self.peek() {
                Tok::Op("(") => {
                    self.bump();
                    let mut args = Vec::new();
                    while !self.is_op(")") {
                        args.push(self.arg());
                        if !self.is_op(",") {
                            break;
                        }
                        self.bump(); // a trailing comma is allowed
                    }
                    self.close(open, ")");
                    ExprKind::Call(Box::new(e), args)
                }
                Tok::Op("[") => {
                    self.bump();
                    let header = std::mem::replace(&mut self.header, false);
                    let idx = self.expr();
                    self.header = header;
                    self.close(open, "]");
                    ExprKind::Index(Box::new(e), Box::new(idx), open)
                }
                Tok::Op(".") => {
                    self.bump();
                    let Some((name, _)) = self.name() else {
                        return e;
                    };
                    ExprKind::Field(Box::new(e), name)
                }
                Tok::Op(o @ ("++" | "--")) => {
                    self.bump();
                    self.inc_dec(o, open, &e);
                    return Expr {
                        kind: ExprKind::Missing,
                        pos: start,
                        end: self.last_end,
                    };
                }
                _ => return e,
            };
            e = Expr {
                kind,
                pos: start,
                end: self.last_end,
            };
        }
    }

    fn primary(&mut self) -> Expr {
        let p = self.pos();
        let kind = match self.peek() {
            Tok::Int(v) => ExprKind::Int(*v),
            Tok::Float(s) => ExprKind::Float(s.clone()),
            Tok::Rune(c) => ExprKind::Rune(*c),
            Tok::Op("[") => return self.list_lit(),
            Tok::Op("{") if !self.header => return self.map_lit(),
            Tok::Kw("true") => ExprKind::Bool(true),
            Tok::Kw("false") => ExprKind::Bool(false),
            Tok::Ident(_) if *self.peek_n(1) == Tok::Op("{") && self.literal_here() => {
                return self.struct_lit();
            }
            Tok::Kw("self") => ExprKind::SelfRef,
            Tok::Kw("if") => return self.if_expr(),
            Tok::Ident(n) => ExprKind::Ident(n.clone()),
            Tok::Kw(k) if self.reserved.contains(k) => ExprKind::Ident(k.to_string()),
            Tok::Kw("none") => ExprKind::NoneVal,
            Tok::Kw("some" | "err" | "ok") => return self.form_value(),
            Tok::Str(parts) => {
                self.bump();
                let segs = self.string_parts(parts);
                return Expr {
                    kind: ExprKind::Str(segs),
                    pos: p,
                    end: self.last_end,
                };
            }
            Tok::Op("(") => {
                self.bump();
                let header = std::mem::replace(&mut self.header, false);
                let inner = self.expr();
                self.header = header;
                self.close(p, ")");
                return Expr {
                    kind: ExprKind::Paren(Box::new(inner)),
                    pos: p,
                    end: self.last_end,
                };
            }
            other => {
                let msg = format!("expected an expression, found {}", describe(other));
                let line_start = self.i > 0 && self.t[self.i - 1].tok == Tok::Nl;
                let hint = match other {
                    Tok::Op(o) if line_start && (level(o).is_some() || ASSIGN_OPS.contains(o)) => {
                        Some("a line that starts with an operator does not continue the line above; put the operator at the end of that line".into())
                    }
                    _ => None,
                };
                self.err("S1", p, msg, hint);
                return self.missing(p);
            }
        };
        self.bump();
        Expr {
            kind,
            pos: p,
            end: self.last_end,
        }
    }

    fn string_parts(&mut self, parts: &[StrPart]) -> Vec<StrSeg> {
        let mut segs = Vec::new();
        for part in parts {
            match part {
                StrPart::Lit(s) => segs.push(StrSeg::Lit(s.clone())),
                StrPart::Expr(toks) | StrPart::Fmt(toks, _) => {
                    let mut sub = Parser::new(self.src, toks, Vec::new());
                    sub.reserved = self.reserved.clone();
                    let e = sub.expr();
                    if *sub.peek() != Tok::Eof {
                        let p = sub.pos();
                        let msg = format!(
                            "an interpolation holds one expression; found {} after it",
                            describe(sub.peek())
                        );
                        sub.err("S1", p, msg, None);
                    }
                    for d in sub.out {
                        self.report(d);
                    }
                    segs.push(match part {
                        StrPart::Fmt(_, f) => StrSeg::Fmt(e, *f),
                        _ => StrSeg::Expr(e),
                    });
                }
            }
        }
        segs
    }
}

/// Part 2a: struct, enum, struct literals, `mut` arguments, `if` as a value, switch.
impl<'a> Parser<'a> {
    /// The expression in the header of `if`/`while`/`for`/`switch`.
    fn head_expr(&mut self) -> Expr {
        let saved = std::mem::replace(&mut self.header, true);
        let e = self.expr();
        self.header = saved;
        e
    }

    /// An argument: `mut place` or an expression; never inside a header.
    fn arg(&mut self) -> Expr {
        let saved = std::mem::replace(&mut self.header, false);
        let e = if self.is_kw("mut") {
            let p = self.pos();
            self.bump();
            let inner = self.expr();
            let end = inner.end;
            Expr {
                kind: ExprKind::Mut(Box::new(inner)),
                pos: p,
                end,
            }
        } else {
            self.expr()
        };
        self.header = saved;
        e
    }

    /// Skips to the `}` that closes the block we are in, and past it.
    fn skip_rest_of_block(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::Op("{") => depth += 1,
                Tok::Op("}") if depth == 0 => {
                    self.bump();
                    return;
                }
                Tok::Op("}") => depth -= 1,
                _ => {}
            }
            self.bump();
        }
    }

    fn bail<T>(&mut self) -> Option<T> {
        self.skip_rest_of_block();
        None
    }

    fn struct_decl(&mut self) -> Option<StructDecl> {
        self.bump(); // struct
        let (name, pos) = self.name()?;
        if !self.expect_op("{") {
            return None;
        }
        let mut fields = Vec::new();
        let one_per_line = |p: &mut Self| {
            let at = p.pos();
            let hint = "put each field on its own line".to_string();
            p.err(
                "R3",
                at,
                "fields of a struct stand one per line",
                Some(hint),
            );
            p.bail::<StructDecl>()
        };
        if !matches!(self.peek(), Tok::Nl) && !self.is_op("}") {
            return one_per_line(self);
        }
        self.skip_nl();
        while !self.is_op("}") && *self.peek() != Tok::Eof {
            let ty = self.type_ref();
            let Some((fname, fpos)) = self.name() else {
                return self.bail();
            };
            let default = if self.is_op("=") {
                self.bump();
                Some(self.expr())
            } else {
                None
            };
            fields.push(Field {
                ty,
                name: fname,
                pos: fpos,
                default,
            });
            if !matches!(self.peek(), Tok::Nl) && !self.is_op("}") {
                return one_per_line(self);
            }
            self.skip_nl();
        }
        self.expect_op("}");
        Some(StructDecl { name, pos, fields })
    }

    fn enum_decl(&mut self) -> Option<EnumDecl> {
        self.bump(); // enum
        let (name, pos) = self.name()?;
        if !self.expect_op("{") {
            return None;
        }
        self.skip_nl();
        let mut variants = Vec::new();
        while !self.is_op("}") && *self.peek() != Tok::Eof {
            let Some((vname, vpos)) = self.name() else {
                return self.bail();
            };
            let mut fields = Vec::new();
            if self.is_op("(") {
                let open = self.pos();
                self.bump();
                while !self.is_op(")") {
                    let ty = self.type_ref();
                    let Some((n, p)) = self.name() else {
                        return self.bail();
                    };
                    fields.push(Param {
                        ty,
                        name: n,
                        pos: p,
                        mut_pos: None,
                    });
                    if !self.is_op(",") {
                        break;
                    }
                    self.bump();
                }
                if !self.close(open, ")") {
                    return self.bail();
                }
            }
            variants.push(Variant {
                name: vname,
                pos: vpos,
                fields,
            });
            if self.is_op(",") {
                self.bump();
            } else if !matches!(self.peek(), Tok::Nl) && !self.is_op("}") {
                self.expect_op(",");
                return self.bail();
            }
            self.skip_nl();
        }
        self.expect_op("}");
        Some(EnumDecl {
            name,
            pos,
            variants,
        })
    }

    /// `T {` is a struct literal outside headers; inside a header only when it
    /// clearly is one (`T{name: …` or `T{}`), and then it is reported (R3).
    fn literal_here(&self) -> bool {
        !self.header
            || *self.peek_n(2) == Tok::Op("}")
            || (matches!(self.peek_n(2), Tok::Ident(_)) && *self.peek_n(3) == Tok::Op(":"))
    }

    fn struct_lit(&mut self) -> Expr {
        let p = self.pos();
        let name = match self.peek() {
            Tok::Ident(n) => n.clone(),
            _ => String::new(),
        };
        self.bump(); // name
        let open = self.pos();
        self.bump(); // {
        let in_header = std::mem::replace(&mut self.header, false);
        self.skip_nl();
        let mut inits = Vec::new();
        while !self.is_op("}") && *self.peek() != Tok::Eof {
            let Some((f, fp)) = self.name() else { break };
            if !self.expect_op(":") {
                break;
            }
            let value = self.expr();
            inits.push(FieldInit {
                name: f,
                pos: fp,
                value,
            });
            self.skip_nl();
            if !self.is_op(",") {
                break;
            }
            self.bump();
            self.skip_nl();
        }
        self.skip_nl();
        if !self.is_op("}") {
            let found = describe(self.peek());
            self.err(
                "S1",
                open,
                format!("expected `}}` to close `{name}{{`, found {found}"),
                None,
            );
        } else {
            self.bump();
        }
        self.header = in_header;
        let e = Expr {
            kind: ExprKind::StructLit(name, inits),
            pos: p,
            end: self.last_end,
        };
        if in_header {
            let (a, b) = (e.pos.offset as usize, e.end as usize);
            let start = self.src[..a].rfind('\n').map_or(0, |i| i + 1);
            let end = self.src[b..].find('\n').map_or(self.src.len(), |i| b + i);
            let hint = format!(
                "{}({}){}",
                self.src[start..a].trim_start(),
                &self.src[a..b],
                &self.src[b..end]
            );
            let msg = "a struct literal in a header needs parentheses";
            self.err("S1", p, msg, Some(hint));
        }
        e
    }

    /// `if c { … } else if … else { … }` in an expression (R15).
    fn if_expr(&mut self) -> Expr {
        let p = self.pos();
        self.bump(); // if
        let mut branches = Vec::new();
        loop {
            let c = self.head_expr();
            let Some(b) = self.value_block() else {
                return self.missing(p);
            };
            branches.push((c, b));
            if *self.peek() == Tok::Nl && *self.peek_n(1) == Tok::Kw("else") {
                let at = self.tok(1).pos;
                let hint = "} else {".to_string();
                self.err(
                    "R3",
                    at,
                    "`else` must be on the same line as `}`",
                    Some(hint),
                );
                self.skip_nl();
            }
            if !self.is_kw("else") {
                let text = self.text(p.offset, self.last_end);
                let hint = format!("{text} else {{ … }}");
                self.err("R15", p, "if as a value needs else", Some(hint));
                return self.missing(p);
            }
            self.bump();
            if self.is_kw("if") {
                self.bump();
                continue;
            }
            let Some(last) = self.value_block() else {
                return self.missing(p);
            };
            return Expr {
                kind: ExprKind::IfExpr(branches, Box::new(last)),
                pos: p,
                end: self.last_end,
            };
        }
    }

    /// A branch of `if` as a value: statements, and an expression as the last line.
    fn value_block(&mut self) -> Option<ValueBlock> {
        let pos = self.pos();
        if !self.is_op("{") {
            let msg = format!(
                "expected `{{` to start a branch, found {}",
                describe(self.peek())
            );
            self.err("R3", pos, msg, None);
            return None;
        }
        if !self.enter() {
            self.skip_block();
            self.leave();
            return None;
        }
        self.bump(); // {
        let header = std::mem::replace(&mut self.header, false);
        self.skip_nl();
        let mut stmts = Vec::new();
        let value = loop {
            if self.is_op("}") || *self.peek() == Tok::Eof {
                let msg = "this branch of if gives no value";
                self.err("R15", pos, msg, None);
                break None;
            }
            if self.last_line() && !self.statement_line() {
                let v = self.expr();
                self.skip_nl();
                break Some(v);
            }
            let before = self.i;
            if let Some(s) = self.unit(|p| p.stmt()) {
                stmts.push(s);
            }
            if self.i == before {
                self.bump();
            }
        };
        self.header = header;
        if self.is_op("}") {
            self.bump();
        } else {
            self.expect_op("}");
            self.skip_rest_of_block();
        }
        self.leave();
        Some(ValueBlock {
            stmts,
            value: value?,
            pos,
        })
    }

    /// Is the line at the cursor the last one before the `}` of its block?
    fn last_line(&self) -> bool {
        let mut depth = 0usize;
        let mut k = 0;
        loop {
            match self.peek_n(k) {
                Tok::Eof => return true,
                Tok::Op("(" | "[" | "{") => depth += 1,
                Tok::Op(")" | "]") => depth = depth.saturating_sub(1),
                Tok::Op("}") if depth == 0 => return true,
                Tok::Op("}") => depth -= 1,
                Tok::Nl if depth == 0 => {
                    while *self.peek_n(k) == Tok::Nl {
                        k += 1;
                    }
                    return *self.peek_n(k) == Tok::Op("}");
                }
                _ => {}
            }
            k += 1;
        }
    }

    /// Does the line at the cursor hold a statement rather than a value?
    fn statement_line(&self) -> bool {
        match self.peek() {
            Tok::Kw(
                "return" | "break" | "continue" | "while" | "for" | "switch" | "const" | "else",
            ) => return true,
            Tok::Kw("if") => return false,
            Tok::Ident(w) if w == "_" && *self.peek_n(1) == Tok::Op("=") => return true,
            Tok::Ident(_) | Tok::Kw("void") if self.name_at(1) => return true,
            _ => {}
        }
        let mut depth = 0usize;
        let mut k = 0;
        loop {
            match self.peek_n(k) {
                Tok::Eof | Tok::Nl if depth == 0 => return false,
                Tok::Eof => return false,
                Tok::Op("(" | "[" | "{") => depth += 1,
                Tok::Op(")" | "]" | "}") if depth == 0 => return false,
                Tok::Op(")" | "]" | "}") => depth -= 1,
                Tok::Op(o) if depth == 0 && ASSIGN_OPS.contains(o) => return true,
                _ => {}
            }
            k += 1;
        }
    }

    fn switch_stmt(&mut self) -> Option<Stmt> {
        let pos = self.pos();
        self.bump(); // switch
        let subject = self.head_expr();
        if !self.is_op("{") {
            let msg = format!(
                "expected `{{` to start the body of `switch`, found {}",
                describe(self.peek())
            );
            self.err("R3", self.pos(), msg, None);
            return None;
        }
        let open = self.pos();
        if !self.enter() {
            self.skip_block();
            self.leave();
            return None;
        }
        self.bump(); // {
        let result = self.switch_body(subject, open, pos);
        self.leave();
        result
    }

    fn switch_body(&mut self, subject: Expr, open: Pos, pos: Pos) -> Option<Stmt> {
        if !matches!(self.peek(), Tok::Nl) && !self.is_op("}") {
            let at = self.pos();
            self.err(
                "R3",
                at,
                "the statements of a case start on the next line",
                None,
            );
            return self.bail();
        }
        self.skip_nl();
        let mut cases = Vec::new();
        let mut default: Option<(Block, Pos)> = None;
        loop {
            match self.peek() {
                Tok::Op("}") => {
                    self.bump();
                    break;
                }
                Tok::Eof => {
                    self.never_closed(open);
                    break;
                }
                Tok::Kw("case") => {
                    let cpos = self.pos();
                    self.bump();
                    if let Some((_, dpos)) = &default {
                        let msg = format!(
                            "default at {}:{} must be the last branch",
                            dpos.line, dpos.col
                        );
                        self.err("R10", cpos, msg, None);
                        return self.bail();
                    }
                    let mut pats = Vec::new();
                    loop {
                        let Some(pat) = self.pattern() else {
                            return self.bail();
                        };
                        pats.push(pat);
                        if !self.is_op(",") {
                            break;
                        }
                        self.bump();
                    }
                    if !self.expect_op(":") {
                        return self.bail();
                    }
                    let Some(body) = self.case_body() else {
                        return self.bail();
                    };
                    cases.push(Case {
                        pats,
                        body,
                        pos: cpos,
                    });
                }
                Tok::Kw("default") => {
                    let dpos = self.pos();
                    self.bump();
                    if default.is_some() {
                        self.err("R10", dpos, "switch has two defaults", None);
                        return self.bail();
                    }
                    if !self.expect_op(":") {
                        return self.bail();
                    }
                    let Some(body) = self.case_body() else {
                        return self.bail();
                    };
                    default = Some((body, dpos));
                }
                other => {
                    let msg = format!("expected `case` or `default`, found {}", describe(other));
                    let at = self.pos();
                    self.err("S1", at, msg, None);
                    return self.bail();
                }
            }
        }
        Some(Stmt::Switch {
            subject,
            cases,
            default,
            pos,
        })
    }

    /// The statements after `case …:` up to the next `case`, `default` or `}`.
    fn case_body(&mut self) -> Option<Block> {
        let pos = self.pos();
        if !matches!(self.peek(), Tok::Nl) {
            self.err(
                "R3",
                pos,
                "the statements of a case start on the next line",
                None,
            );
            return None;
        }
        self.skip_nl();
        let mut stmts = Vec::new();
        while !matches!(
            self.peek(),
            Tok::Kw("case" | "default") | Tok::Op("}") | Tok::Eof
        ) {
            let before = self.i;
            if let Some(s) = self.unit(|p| p.stmt()) {
                stmts.push(s);
            }
            if self.i == before {
                self.bump();
            }
        }
        Some(Block { stmts, pos })
    }

    fn pattern(&mut self) -> Option<Pattern> {
        let p = self.pos();
        match self.peek() {
            Tok::Op("-") if matches!(self.peek_n(1), Tok::Int(_)) => {
                self.bump();
                let v = match self.peek() {
                    Tok::Int(v) => *v as i128,
                    _ => 0,
                };
                self.bump();
                Some(Pattern::Int(-v, p))
            }
            Tok::Int(v) => {
                let v = *v as i128;
                self.bump();
                Some(Pattern::Int(v, p))
            }
            Tok::Rune(c) => {
                let c = *c;
                self.bump();
                Some(Pattern::Rune(c, p))
            }
            Tok::Str(parts) => {
                let mut text = String::new();
                for part in parts {
                    match part {
                        StrPart::Lit(s) => text.push_str(s),
                        StrPart::Expr(_) | StrPart::Fmt(..) => {
                            let hint = "write \\{ and \\} for literal braces".to_string();
                            self.err("S1", p, "a case value is a plain string", Some(hint));
                            return None;
                        }
                    }
                }
                self.bump();
                Some(Pattern::Str(text, p))
            }
            Tok::Kw("ok" | "err" | "some" | "none") => self.result_pattern(),
            Tok::Ident(ty) if *self.peek_n(1) == Tok::Op(".") => {
                let mut ty = ty.clone();
                self.bump();
                self.bump();
                let (mut name, _) = self.name()?;
                // `io.IoErr.NotFound`: the type of a standard module
                if MODULES.contains(&ty.as_str()) && self.is_op(".") {
                    self.bump();
                    ty = format!("{ty}.{name}");
                    name = self.name()?.0;
                }
                let mut binds = None;
                if self.is_op("(") {
                    let open = self.pos();
                    self.bump();
                    let mut list = Vec::new();
                    while !self.is_op(")") {
                        if matches!(self.peek(), Tok::Ident(w) if w == "_") {
                            self.bump();
                            list.push(None);
                        } else {
                            let t = self.type_ref();
                            let (n, np) = self.name()?;
                            list.push(Some(Param {
                                ty: t,
                                name: n,
                                pos: np,
                                mut_pos: None,
                            }));
                        }
                        if !self.is_op(",") {
                            break;
                        }
                        self.bump();
                    }
                    if !self.close(open, ")") {
                        return None;
                    }
                    binds = Some(list);
                }
                Some(Pattern::Variant {
                    ty,
                    name,
                    binds,
                    pos: p,
                })
            }
            Tok::Ident(n) => {
                let n = n.clone();
                self.bump();
                Some(Pattern::Name(n, p))
            }
            other => {
                let msg = format!("expected a case value, found {}", describe(other));
                self.err("S1", p, msg, None);
                None
            }
        }
    }
}

/// Part 2b: `none`, `some`, `err`, `try`, `test` blocks and the patterns of results.
impl<'a> Parser<'a> {
    /// `try x` or `try x or e`; `or` belongs to the `try` just before it (R12).
    fn try_expr(&mut self) -> Expr {
        let p = self.pos();
        self.bump(); // try
        if !self.enter() {
            self.leave();
            self.skip_operand();
            return self.missing(p);
        }
        let x = self.postfix();
        let mut or = None;
        if self.is_kw("or") {
            let at = self.pos();
            self.bump();
            or = Some((at, Box::new(self.postfix())));
        }
        self.leave();
        Expr {
            kind: ExprKind::Try(Box::new(x), or),
            pos: p,
            end: self.last_end,
        }
    }

    /// `some(x)`, `err(e)`, `ok(x)` and a bare `ok`.
    fn form_value(&mut self) -> Expr {
        let p = self.pos();
        let word = match self.peek() {
            Tok::Kw(k) => *k,
            _ => "",
        };
        self.bump();
        let mut arg = None;
        if self.is_op("(") {
            let open = self.pos();
            self.bump();
            arg = Some(Box::new(self.arg()));
            self.close(open, ")");
        } else if word != "ok" {
            let msg = format!("`{word}` needs a value: {word}(x)");
            self.err("S1", self.pos(), msg, None);
            return self.missing(p);
        }
        let kind = match (word, arg) {
            ("some", Some(x)) => ExprKind::SomeVal(x),
            ("err", Some(x)) => ExprKind::ErrVal(x),
            (_, x) => ExprKind::OkVal(x),
        };
        Expr {
            kind,
            pos: p,
            end: self.last_end,
        }
    }

    /// The plain text of a string token that must not contain `{ }` (a test name).
    fn plain_string(&mut self, parts: &[StrPart]) -> (String, bool) {
        let mut text = String::new();
        let mut plain = true;
        for part in parts {
            match part {
                StrPart::Lit(s) => text.push_str(s),
                StrPart::Expr(_) | StrPart::Fmt(..) => plain = false,
            }
        }
        (text, plain)
    }

    /// `test "name" [panics "text"] { … }` (R18).
    fn test_decl(&mut self) -> Option<TestDecl> {
        let pos = self.pos();
        self.bump(); // test
        let name_pos = self.pos();
        let Tok::Str(parts) = self.peek() else {
            let hint = "test \"name\" { … }".to_string();
            self.err("R18", name_pos, "a test needs a name", Some(hint));
            return None;
        };
        let (name, plain) = self.plain_string(parts);
        self.bump();
        if !plain {
            let hint = "remove the { }".to_string();
            self.err("R18", name_pos, "a test name is plain text", Some(hint));
        } else if name.is_empty() {
            self.err("R18", name_pos, "a test needs a name", None);
        }
        let mut panics = None;
        if self.word_at(0, &["panics"]) {
            self.bump();
            let at = self.pos();
            let Tok::Str(parts) = self.peek() else {
                let msg = format!("expected the panic text, found {}", describe(self.peek()));
                self.err("S1", at, msg, Some("panics \"text\"".into()));
                return None;
            };
            let (text, plain) = self.plain_string(parts);
            self.bump();
            if !plain || text.is_empty() {
                let hint = "give the part of the panic message to look for".to_string();
                self.err("R18", at, "panics needs a message text", Some(hint));
            }
            panics = Some((text, at));
        }
        let body = self.body("test", pos.offset)?;
        Some(TestDecl {
            name,
            name_pos,
            panics,
            body,
            pos,
        })
    }

    /// `ok`, `err`, `some`, `none`, each with an optional `(T name)` or `(_)`.
    fn result_pattern(&mut self) -> Option<Pattern> {
        let pos = self.pos();
        let Tok::Kw(kind) = self.peek() else {
            return None;
        };
        self.bump();
        let mut bind = None;
        if self.is_op("(") {
            let open = self.pos();
            self.bump();
            if matches!(self.peek(), Tok::Ident(w) if w == "_") {
                self.bump();
                bind = Some(None);
            } else {
                let ty = self.type_ref();
                let (name, np) = self.name()?;
                bind = Some(Some(Param {
                    ty,
                    name,
                    pos: np,
                    mut_pos: None,
                }));
            }
            if !self.close(open, ")") {
                return None;
            }
        }
        Some(Pattern::Result { kind, bind, pos })
    }
}

/// Part 2c: list and map literals, `use std.…`.
impl<'a> Parser<'a> {
    /// `[a, b, c]`, also over several lines (R5).
    fn list_lit(&mut self) -> Expr {
        let p = self.pos();
        self.bump(); // [
        let header = std::mem::replace(&mut self.header, false);
        let mut items = Vec::new();
        while !self.is_op("]") && *self.peek() != Tok::Eof {
            items.push(self.expr());
            if !self.is_op(",") {
                break;
            }
            self.bump(); // a trailing comma is allowed
        }
        self.header = header;
        self.close(p, "]");
        Expr {
            kind: ExprKind::ListLit(items),
            pos: p,
            end: self.last_end,
        }
    }

    /// `{k: v, …}` (R30); like a struct literal it may span lines.
    fn map_lit(&mut self) -> Expr {
        let p = self.pos();
        self.bump(); // {
        let header = std::mem::replace(&mut self.header, false);
        self.skip_nl();
        let mut entries = Vec::new();
        while !self.is_op("}") && *self.peek() != Tok::Eof {
            let k = self.expr();
            if !self.expect_op(":") {
                break;
            }
            let v = self.expr();
            entries.push((k, v));
            self.skip_nl();
            if !self.is_op(",") {
                break;
            }
            self.bump();
            self.skip_nl();
        }
        self.skip_nl();
        self.header = header;
        if !self.is_op("}") {
            let found = describe(self.peek());
            let msg = format!("expected `}}` to close `{{`, found {found}");
            self.err("S1", p, msg, None);
        } else {
            self.bump();
        }
        Expr {
            kind: ExprKind::MapLit(entries),
            pos: p,
            end: self.last_end,
        }
    }

    /// `use std.io` (R16); the other forms of `use` come with modules.
    fn use_decl(&mut self) -> Option<UseDecl> {
        let pos = self.pos();
        self.bump(); // use
        let std = matches!(self.peek(), Tok::Ident(n) if n == "std");
        if !std || *self.peek_n(1) != Tok::Op(".") || !matches!(self.peek_n(2), Tok::Ident(_)) {
            let msg = format!(
                "expected `std.name` after `use`, found {}",
                describe(self.peek())
            );
            self.err("S1", self.pos(), msg, Some("use std.io".into()));
            return None;
        }
        self.bump();
        self.bump();
        let name_pos = self.pos();
        let Tok::Ident(module) = self.peek() else {
            return None;
        };
        let module = module.clone();
        self.bump();
        Some(UseDecl {
            module,
            name_pos,
            pos,
        })
    }
}

fn can_start_stmt(t: &Tok) -> bool {
    matches!(
        t,
        Tok::Ident(_)
            | Tok::Kw(
                "if" | "while"
                    | "for"
                    | "switch"
                    | "break"
                    | "continue"
                    | "return"
                    | "const"
                    | "void"
                    | "self"
                    | "try"
                    | "test"
            )
    )
}

fn is_place(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Ident(_) | ExprKind::SelfRef => true,
        ExprKind::Field(inner, _) | ExprKind::Index(inner, _, _) => is_place(inner),
        _ => false,
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(n) => format!("`{n}`"),
        Tok::Kw(k) => format!("`{k}`"),
        Tok::Op(o) => format!("`{o}`"),
        Tok::Int(v) => format!("number `{v}`"),
        Tok::Float(f) => format!("number `{f}`"),
        Tok::Rune(_) => "a rune".into(),
        Tok::Str(_) => "a string".into(),
        Tok::Semi => "`;`".into(),
        Tok::Nl => "end of line".into(),
        Tok::Eof => "end of file".into(),
    }
}

/// `int i = A; i < B; …` → `for int i in range(B)` or `range(A, B)`.
fn c_for_hint(inner: &[&Tok]) -> String {
    let word = |t: &Tok| match t {
        Tok::Ident(n) => Some(n.clone()),
        Tok::Int(v) => Some(v.to_string()),
        _ => None,
    };
    let mut k = 0;
    if matches!(inner.first(), Some(Tok::Ident(_))) && matches!(inner.get(1), Some(Tok::Ident(_))) {
        k = 1; // skip the type
    }
    let var = inner.get(k).and_then(|t| word(t));
    let from = match (inner.get(k + 1), inner.get(k + 2)) {
        (Some(Tok::Op("=")), Some(t)) => word(t),
        _ => None,
    };
    let to = match (inner.get(k + 4), inner.get(k + 5), inner.get(k + 6)) {
        (Some(_), Some(Tok::Op("<")), Some(t)) => word(t),
        _ => None,
    };
    match (var, from, to) {
        (Some(v), Some(f), Some(t)) if f == "0" => format!("for int {v} in range({t})"),
        (Some(v), Some(f), Some(t)) => format!("for int {v} in range({f}, {t})"),
        _ => "for int i in range(n)".into(),
    }
}
