//! Recursive-descent parser (spec §2, R1–R4, R11, R13 grammar). One cause, one
//! diagnostic: foreign spellings are recognized as a whole (§0.1), and after an
//! error the rest of the statement and a directly following block are skipped.

use crate::ast::*;
use crate::diag::{Diag, Pos};
use crate::lexer::{StrPart, Tok, Token};

pub fn parse(src: &str, toks: &[Token]) -> (Module, Vec<Diag>) {
    let mut p = Parser::new(src, toks);
    let m = p.module();
    (m, p.diags)
}

const ASSIGN_OPS: &[&str] = &[
    "=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=",
];

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
    matches!(&e.kind, ExprKind::Binary(op, _, _) if ops.contains(op))
}

const COMPARISONS: &[&str] = &["==", "!=", "<", "<=", ">", ">="];

struct Parser<'a> {
    src: &'a str,
    t: &'a [Token],
    i: usize,
    last_end: u32,
    unclosed: bool,
    diags: Vec<Diag>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, t: &'a [Token]) -> Self {
        Parser {
            src,
            t,
            i: 0,
            last_end: 0,
            unclosed: false,
            diags: Vec::new(),
        }
    }

    // ---- token helpers ----

    fn peek(&self) -> &Tok {
        &self.t[self.i.min(self.t.len() - 1)].tok
    }

    fn peek_n(&self, n: usize) -> &Tok {
        &self.t[(self.i + n).min(self.t.len() - 1)].tok
    }

    fn pos(&self) -> Pos {
        self.t[self.i.min(self.t.len() - 1)].pos
    }

    fn bump(&mut self) -> Token {
        let tok = self.t[self.i.min(self.t.len() - 1)].clone();
        if tok.tok != Tok::Eof {
            self.i += 1;
            self.last_end = tok.end;
        }
        tok
    }

    fn is_op(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Op(o) if *o == s)
    }

    fn is_kw(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Kw(k) if *k == s)
    }

    fn at_line_end(&self) -> bool {
        matches!(self.peek(), Tok::Nl | Tok::Eof | Tok::Semi) || self.is_op("}")
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

    fn err(&mut self, code: &'static str, pos: Pos, msg: impl Into<String>, hint: Option<String>) {
        let mut d = Diag::new(code, pos, msg);
        if let Some(h) = hint {
            d = d.hint(h);
        }
        self.diags.push(d);
    }

    fn expect_op(&mut self, s: &str) -> bool {
        if self.is_op(s) {
            self.bump();
            true
        } else {
            let p = self.pos();
            let found = describe(self.peek());
            self.err("S1", p, format!("expected `{s}`, found {found}"), None);
            false
        }
    }

    fn ident(&mut self) -> Option<(String, Pos)> {
        let p = self.pos();
        match self.peek().clone() {
            Tok::Ident(n) => {
                self.bump();
                Some((n, p))
            }
            other => {
                self.err(
                    "S1",
                    p,
                    format!("expected a name, found {}", describe(&other)),
                    None,
                );
                None
            }
        }
    }

    /// Skips to the end of the current line at bracket depth 0 (not past `}` of the
    /// enclosing block), then a block that directly follows (§0.1).
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

    /// After a declaration or statement: end of line, `}` or end of file.
    fn end_of_stmt(&mut self) {
        loop {
            match self.peek() {
                Tok::Nl => {
                    self.skip_nl();
                    return;
                }
                Tok::Eof => return,
                Tok::Op("}") => return,
                Tok::Semi => {
                    let p = self.pos();
                    self.err(
                        "L3",
                        p,
                        "`;` is not used in difficulty",
                        Some("statements end at the end of the line".into()),
                    );
                    self.bump();
                }
                _ => {
                    let p = self.pos();
                    self.err(
                        "R3",
                        p,
                        "two statements on one line",
                        Some("put each statement on its own line".into()),
                    );
                    self.recover();
                }
            }
        }
    }

    // ---- module and declarations ----

    fn module(&mut self) -> Module {
        let mut decls = Vec::new();
        self.skip_nl();
        while *self.peek() != Tok::Eof {
            let before = self.i;
            if let Some(d) = self.decl() {
                decls.push(d);
            }
            if self.is_op("}") && self.i == before {
                let p = self.pos();
                self.err("S1", p, "unexpected `}`", None);
                self.bump();
            }
            self.end_of_stmt();
        }
        Module { decls }
    }

    fn is_type_name(&self) -> bool {
        matches!(self.peek(), Tok::Ident(_) | Tok::Kw("void"))
    }

    fn decl(&mut self) -> Option<Decl> {
        let start = self.pos();
        if self.is_kw("const") {
            return self.var_decl().map(Decl::Const);
        }
        if let Tok::Ident(w) = self.peek() {
            if ["def", "fn", "function", "func"].contains(&w.as_str())
                && matches!(self.peek_n(1), Tok::Ident(_))
            {
                let w = w.clone();
                self.err(
                    "R2",
                    start,
                    format!("functions start with their return type, not `{w}`"),
                    Some("the return type comes first: int add(int a, int b)".into()),
                );
                self.skip_to_block_and_skip();
                return None;
            }
        }
        if self.is_type_name() && matches!(self.peek_n(1), Tok::Ident(_)) {
            match self.peek_n(2) {
                Tok::Op("(") => return self.func().map(Decl::Func),
                Tok::Op("=") => {
                    let v = self.var_decl()?;
                    self.err(
                        "R4",
                        v.pos,
                        format!("variable `{}` outside a function", v.name),
                        Some("make it const, or move it into a function".into()),
                    );
                    return Some(Decl::Const(v));
                }
                _ => {}
            }
        }
        // A statement at the top level: one diagnostic (R1), not the statement's own.
        let n = self.diags.len();
        self.stmt();
        self.diags.truncate(n);
        self.err(
            "R1",
            start,
            "statement outside a function",
            Some("move it into a function, e.g. void main() { … }".into()),
        );
        None
    }

    /// Skips a foreign function: its header and its `{ … }` body, or, without
    /// braces (Python style), the indented lines below it.
    fn skip_to_block_and_skip(&mut self) {
        while !matches!(self.peek(), Tok::Eof) && !self.is_op("{") {
            if *self.peek() == Tok::Nl && !matches!(self.peek_n(1), Tok::Op("{")) {
                while *self.peek() == Tok::Nl && self.t[self.i + 1].pos.col > 1 {
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

    fn type_ref(&mut self) -> TypeRef {
        let p = self.pos();
        match self.peek().clone() {
            Tok::Ident(n) => {
                self.bump();
                TypeRef { name: n, pos: p }
            }
            Tok::Kw("void") => {
                self.bump();
                TypeRef {
                    name: "void".into(),
                    pos: p,
                }
            }
            other => {
                self.err(
                    "S1",
                    p,
                    format!("expected a type, found {}", describe(&other)),
                    None,
                );
                TypeRef {
                    name: "?".into(),
                    pos: p,
                }
            }
        }
    }

    fn func(&mut self) -> Option<Func> {
        let header = self.pos().offset;
        let ret = self.type_ref();
        let (name, pos) = self.ident()?;
        self.expect_op("(");
        let mut params = Vec::new();
        while !self.is_op(")") && *self.peek() != Tok::Eof {
            let ty = self.type_ref();
            let (pname, ppos) = self.ident()?;
            params.push(Param {
                ty,
                name: pname,
                pos: ppos,
            });
            if !self.is_op(")") && !self.expect_op(",") {
                self.recover();
                return None;
            }
        }
        self.expect_op(")");
        let body = self.body("function", header)?;
        Some(Func {
            ret,
            name,
            pos,
            params,
            body,
        })
    }

    /// `[const] T name = value` and the foreign spellings `var/let/auto x = v`.
    fn var_decl(&mut self) -> Option<VarDecl> {
        let is_const = self.is_kw("const");
        if is_const {
            self.bump();
        }
        let ty = self.type_ref();
        let (name, pos) = self.ident()?;
        if !self.is_op("=") {
            let default = match ty.name.as_str() {
                "bool" => "false",
                "str" => "\"\"",
                _ => "0",
            };
            self.err(
                "R2",
                pos,
                format!("`{}` needs a value", name),
                Some(format!(
                    "give it a value: {} {} = {}",
                    ty.name, name, default
                )),
            );
            self.recover();
            let value = Expr {
                kind: ExprKind::Missing,
                pos,
                end: pos.offset,
            };
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

    fn foreign_var(&mut self) -> Option<Stmt> {
        let start = self.pos();
        self.bump(); // var / let / auto
        let (name, pos) = self.ident()?;
        if !self.expect_op("=") {
            self.recover();
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
        let hint = match ty {
            Some(t) => format!("{} {} = {}", t, name, self.expr_text(&value)),
            None => format!("write its type: T {} = {}", name, self.expr_text(&value)),
        };
        self.err("R2", start, "declarations name their type", Some(hint));
        let ty = TypeRef {
            name: ty.unwrap_or("?").into(),
            pos: start,
        };
        Some(Stmt::Var(VarDecl {
            is_const: false,
            ty,
            name,
            pos,
            value,
        }))
    }

    // ---- blocks and statements ----

    /// A block that must start on the line of its header (R3).
    fn body(&mut self, what: &str, header_start: u32) -> Option<Block> {
        if self.is_op("{") {
            return Some(self.block());
        }
        let p = self.pos();
        if *self.peek() == Tok::Nl && *self.peek_n(1) == Tok::Op("{") {
            let header = self.text(header_start, self.last_end).to_string();
            self.err(
                "R3",
                p,
                format!("the body of `{what}` must start on the same line"),
                Some(format!("{header} {{")),
            );
            self.bump();
            self.skip_block();
            return Some(Block {
                stmts: Vec::new(),
                pos: p,
            });
        }
        let found = describe(self.peek());
        self.err(
            "R3",
            p,
            format!("expected `{{` to start the body of `{what}`, found {found}"),
            Some(format!(
                "put the body in braces: {} {{ … }}",
                self.text(header_start, self.last_end)
            )),
        );
        self.recover();
        None
    }

    fn block(&mut self) -> Block {
        let pos = self.pos();
        self.bump(); // {
        self.skip_nl();
        let mut stmts = Vec::new();
        while !self.is_op("}") && *self.peek() != Tok::Eof {
            if let Some(s) = self.stmt() {
                stmts.push(s);
            }
            self.end_of_stmt();
        }
        if self.is_op("}") {
            self.bump();
        } else if !self.unclosed {
            // the innermost open block is reported; the outer ones share the cause
            self.unclosed = true;
            self.err(
                "S1",
                pos,
                "this block is never closed",
                Some("add `}`".into()),
            );
        }
        Block { stmts, pos }
    }

    fn stmt(&mut self) -> Option<Stmt> {
        let p = self.pos();
        match self.peek().clone() {
            Tok::Kw("if") => self.if_stmt(),
            Tok::Kw("while") => {
                self.bump();
                let cond = self.expr();
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
                let value = if self.at_line_end() {
                    None
                } else {
                    Some(self.expr())
                };
                Some(Stmt::Return(value, p))
            }
            Tok::Kw("const") => self.var_decl().map(Stmt::Var),
            Tok::Kw("else") => {
                self.err("S1", p, "`else` without `if`", None);
                self.recover();
                None
            }
            Tok::Ident(w) if w == "_" && *self.peek_n(1) == Tok::Op("=") => {
                self.bump();
                self.bump();
                let e = self.expr();
                Some(Stmt::Discard(e, p))
            }
            Tok::Ident(w)
                if ["var", "let", "auto"].contains(&w.as_str())
                    && matches!(self.peek_n(1), Tok::Ident(_)) =>
            {
                self.foreign_var()
            }
            Tok::Ident(w) if w == "elif" || w == "elsif" => {
                self.err(
                    "R3",
                    p,
                    format!("`{w}` does not exist"),
                    Some("use else if".into()),
                );
                self.recover();
                None
            }
            Tok::Ident(_) | Tok::Kw("void") if matches!(self.peek_n(1), Tok::Ident(_)) => {
                self.var_decl().map(Stmt::Var)
            }
            _ => self.expr_stmt(),
        }
    }

    fn if_stmt(&mut self) -> Option<Stmt> {
        let pos = self.pos();
        self.bump();
        let cond = self.expr();
        let body = self.body("if", pos.offset)?;
        let mut branches = vec![(cond, body)];
        let mut otherwise = None;
        loop {
            if *self.peek() == Tok::Nl && *self.peek_n(1) == Tok::Kw("else") {
                let p = self.t[self.i + 1].pos;
                self.err(
                    "R3",
                    p,
                    "`else` must be on the same line as `}`",
                    Some("} else {".into()),
                );
                self.skip_nl();
            }
            if !self.is_kw("else") {
                break;
            }
            let else_pos = self.pos();
            self.bump();
            if self.is_kw("if") {
                self.bump();
                let c = self.expr();
                let b = self.body("else if", else_pos.offset)?;
                branches.push((c, b));
            } else {
                otherwise = Some(self.body("else", else_pos.offset)?);
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
        loop {
            let ty = self.type_ref();
            let (name, p) = self.ident()?;
            vars.push(Param { ty, name, pos: p });
            if vars.len() == 2 || !self.is_op(",") {
                break;
            }
            self.bump();
        }
        if !self.is_kw("in") {
            let p = self.pos();
            let found = describe(self.peek());
            self.err(
                "S1",
                p,
                format!("expected `in`, found {found}"),
                Some("for int i in range(n) { … }".into()),
            );
            self.recover();
            return None;
        }
        self.bump();
        let iter = self.expr();
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
        loop {
            match self.peek() {
                Tok::Eof => break,
                Tok::Op("(") => depth += 1,
                Tok::Op(")") => {
                    depth -= 1;
                    if depth == 0 {
                        self.bump();
                        break;
                    }
                }
                _ => {}
            }
            self.bump();
        }
        let inner: Vec<&Tok> = self.t[open + 1..self.i.saturating_sub(1)]
            .iter()
            .map(|t| &t.tok)
            .collect();
        let hint = c_for_hint(&inner);
        self.err("R11", pos, "C-style for loop", Some(hint));
        self.skip_block();
        None
    }

    fn expr_stmt(&mut self) -> Option<Stmt> {
        let e = self.expr();
        if let Tok::Op(op) = *self.peek() {
            if ASSIGN_OPS.contains(&op) {
                let pos = self.pos();
                self.bump();
                let value = self.expr();
                if !is_place(&e) {
                    let text = self.expr_text(&e).to_string();
                    self.err("S1", e.pos, format!("cannot assign to `{text}`"), None);
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
        match e.kind {
            ExprKind::Call(..) => Some(Stmt::Call(e)),
            ExprKind::Missing => {
                self.recover();
                None
            }
            _ => {
                let text = self.expr_text(&e).to_string();
                self.err(
                    "S4",
                    e.pos,
                    format!("`{text}` does nothing as a statement"),
                    Some(format!("use its value, e.g. int x = {text}")),
                );
                None
            }
        }
    }

    // ---- expressions ----

    fn expr(&mut self) -> Expr {
        self.binary(1)
    }

    fn binary(&mut self, min: u8) -> Expr {
        let mut lhs = self.unary();
        while let Tok::Op(o) = self.peek() {
            let (op, lvl) = match level(o) {
                Some(l) if l >= min => (*o, l),
                _ => break,
            };
            let opos = self.pos();
            self.bump();
            let rhs = self.binary(if op == "??" { lvl } else { lvl + 1 });
            if lvl == 4 && is_binary(&lhs, COMPARISONS) {
                if let ExprKind::Binary(op1, a, b) = &lhs.kind {
                    let hint = format!(
                        "write {} {} {} && {} {} {}",
                        self.expr_text(a),
                        op1,
                        self.expr_text(b),
                        self.expr_text(b),
                        op,
                        self.expr_text(&rhs)
                    );
                    self.err("S2", opos, "comparisons cannot be chained", Some(hint));
                }
            }
            let mixed = (op == "||" && (is_binary(&lhs, &["&&"]) || is_binary(&rhs, &["&&"])))
                || (op == "&&" && (is_binary(&lhs, &["||"]) || is_binary(&rhs, &["||"])));
            if mixed {
                let (l, r) = (self.expr_text(&lhs), self.expr_text(&rhs));
                let hint = if is_binary(&lhs, &["&&", "||"]) {
                    format!("add parentheses: ({l}) {op} {r}")
                } else {
                    format!("add parentheses: {l} {op} ({r})")
                };
                self.err(
                    "S3",
                    opos,
                    "`&&` and `||` are mixed without parentheses",
                    Some(hint),
                );
            }
            let pos = lhs.pos;
            let end = rhs.end;
            lhs = Expr {
                kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
                pos,
                end,
            };
        }
        lhs
    }

    fn unary(&mut self) -> Expr {
        let p = self.pos();
        let op = match self.peek() {
            Tok::Op(o @ ("-" | "!" | "~")) => *o,
            Tok::Kw("try") => "try",
            Tok::Op(o @ ("++" | "--")) => {
                let o = *o;
                self.bump();
                let e = self.unary();
                let name = self.expr_text(&e).to_string();
                let fix = if o == "++" { "+=" } else { "-=" };
                self.err(
                    "L7",
                    p,
                    format!("`{o}` does not exist"),
                    Some(format!("use {name} {fix} 1")),
                );
                return Expr {
                    kind: ExprKind::Missing,
                    pos: p,
                    end: e.end,
                };
            }
            _ => return self.postfix(),
        };
        self.bump();
        let e = self.unary();
        let end = e.end;
        Expr {
            kind: ExprKind::Unary(op, Box::new(e)),
            pos: p,
            end,
        }
    }

    fn postfix(&mut self) -> Expr {
        let mut e = self.primary();
        loop {
            if self.is_op("(") {
                self.bump();
                let mut args = Vec::new();
                while !self.is_op(")") && *self.peek() != Tok::Eof {
                    args.push(self.expr());
                    if !self.is_op(")") && !self.expect_op(",") {
                        break;
                    }
                }
                self.expect_op(")");
                let pos = e.pos;
                e = Expr {
                    kind: ExprKind::Call(Box::new(e), args),
                    pos,
                    end: self.last_end,
                };
            } else if self.is_op("[") {
                self.bump();
                let idx = self.expr();
                self.expect_op("]");
                let pos = e.pos;
                e = Expr {
                    kind: ExprKind::Index(Box::new(e), Box::new(idx)),
                    pos,
                    end: self.last_end,
                };
            } else if self.is_op(".") {
                self.bump();
                let Some((name, _)) = self.ident() else { break };
                let pos = e.pos;
                e = Expr {
                    kind: ExprKind::Field(Box::new(e), name),
                    pos,
                    end: self.last_end,
                };
            } else if let Tok::Op(o @ ("++" | "--")) = self.peek() {
                let o = *o;
                let p = self.pos();
                self.bump();
                let name = self.expr_text(&e).to_string();
                let fix = if o == "++" { "+=" } else { "-=" };
                self.err(
                    "L7",
                    p,
                    format!("`{o}` does not exist"),
                    Some(format!("use {name} {fix} 1")),
                );
                return Expr {
                    kind: ExprKind::Missing,
                    pos: e.pos,
                    end: self.last_end,
                };
            } else {
                break;
            }
        }
        e
    }

    fn primary(&mut self) -> Expr {
        let p = self.pos();
        let kind = match self.peek().clone() {
            Tok::Int(v) => ExprKind::Int(v),
            Tok::Float(s) => ExprKind::Float(s),
            Tok::Kw("true") => ExprKind::Bool(true),
            Tok::Kw("false") => ExprKind::Bool(false),
            Tok::Ident(n) => ExprKind::Ident(n),
            Tok::Str(parts) => {
                self.bump();
                let segs = self.string_parts(&parts);
                return Expr {
                    kind: ExprKind::Str(segs),
                    pos: p,
                    end: self.last_end,
                };
            }
            Tok::Op("(") => {
                self.bump();
                let inner = self.expr();
                self.expect_op(")");
                return Expr {
                    kind: ExprKind::Paren(Box::new(inner)),
                    pos: p,
                    end: self.last_end,
                };
            }
            other => {
                self.err(
                    "S1",
                    p,
                    format!("expected an expression, found {}", describe(&other)),
                    None,
                );
                return Expr {
                    kind: ExprKind::Missing,
                    pos: p,
                    end: p.offset,
                };
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
                StrPart::Expr(toks) => {
                    let mut sub = Parser::new(self.src, toks);
                    let e = sub.expr();
                    if *sub.peek() != Tok::Eof {
                        let p = sub.pos();
                        let found = describe(sub.peek());
                        sub.err(
                            "S1",
                            p,
                            format!("unexpected {found} in interpolation"),
                            None,
                        );
                    }
                    self.diags.extend(sub.diags);
                    segs.push(StrSeg::Expr(e));
                }
            }
        }
        segs
    }
}

fn is_place(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Ident(_) => true,
        ExprKind::Field(inner, _) | ExprKind::Index(inner, _) => is_place(inner),
        _ => false,
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Ident(n) => format!("`{n}`"),
        Tok::Kw(k) => format!("`{k}`"),
        Tok::Op(o) => format!("`{o}`"),
        Tok::Int(_) | Tok::Float(_) => "a number".into(),
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

#[cfg(test)]
mod tests {
    use crate::check_source;

    fn codes(src: &str) -> Vec<&'static str> {
        check_source(src).into_iter().map(|d| d.code).collect()
    }

    fn first(src: &str) -> crate::diag::Diag {
        check_source(src)
            .into_iter()
            .next()
            .expect("expected a diagnostic")
    }

    #[test]
    fn examples_parse() {
        for f in ["examples/hello.dif", "examples/fib.dif"] {
            let src =
                std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/").to_string() + f)
                    .unwrap();
            assert_eq!(check_source(&src), vec![], "{f}");
        }
    }

    #[test]
    fn foreign_syntax() {
        let d = first("void main() {\n    int i = 0\n    i++\n}\n");
        assert_eq!((d.code, d.hint.as_deref()), ("L7", Some("use i += 1")));
        let d = first("void main() {\n    var x = 5\n    print(x)\n}\n");
        assert_eq!(
            (d.code, d.pos.line, d.pos.col, d.hint.as_deref()),
            ("R2", 2, 5, Some("int x = 5"))
        );
        let d = first("fn add(a: int) {\n}\n");
        assert_eq!(d.code, "R2");
        assert!(d.hint.unwrap().contains("return type comes first"));
        let d = first("void main() {\n    elif x {\n    }\n}\n");
        assert_eq!((d.code, d.hint.as_deref()), ("R3", Some("use else if")));
        let d = first(
            "void main() {\n    for (int i = 0; i < 3; i += 1) {\n        print(i)\n    }\n}\n",
        );
        assert_eq!(
            (d.code, d.pos.col, d.hint.as_deref()),
            ("R11", 5, Some("for int i in range(3)"))
        );
        assert_eq!(
            codes("void main() {\n    for (int i = 0; i < 3; i += 1) {\n    }\n}\n"),
            ["R11"]
        );
        assert_eq!(codes("/* x */\nvoid main() {\n}\n"), ["L2"]);
    }

    #[test]
    fn precedence_errors() {
        let d = first("void main() {\n    bool b = 1 < 2 < 3\n}\n");
        assert_eq!(
            (d.code, d.hint.as_deref()),
            ("S2", Some("write 1 < 2 && 2 < 3"))
        );
        let d = first("void main() {\n    bool b = true && false || true\n}\n");
        assert_eq!(
            (d.code, d.hint.as_deref()),
            ("S3", Some("add parentheses: (true && false) || true"))
        );
        assert_eq!(
            codes("void main() {\n    bool b = (true && false) || true\n}\n"),
            [] as [&str; 0]
        );
        assert_eq!(
            codes("void main() {\n    bool b = 1 & 2 == 2\n}\n"),
            [] as [&str; 0]
        );
    }

    #[test]
    fn statement_and_block_rules() {
        let d = first("print(\"Hallo\")\n");
        assert_eq!((d.code, d.pos.line, d.pos.col), ("R1", 1, 1));
        let d =
            first("void main() {\n    int n = 3\n    if n > 0\n    {\n        n -= 1\n    }\n}\n");
        assert_eq!(
            (d.code, d.pos.line, d.pos.col, d.hint.as_deref()),
            ("R3", 3, 13, Some("if n > 0 {"))
        );
        assert_eq!(
            codes("void main() {\n    int n = 3\n    if n > 0\n    {\n        n -= 1\n    }\n}\n"),
            ["R3"]
        );
        assert_eq!(codes("void main() {\n    int a = 1 int b = 2\n}\n"), ["R3"]);
        assert_eq!(codes("void main() {\n    int a = 1;\n}\n"), ["L3"]);
        assert_eq!(codes("void main() {\n    1 + 2\n}\n"), ["S4"]);
        assert_eq!(codes("int x = 1\n"), ["R4"]);
        assert_eq!(codes("void main() {\n    int n\n}\n"), ["R2"]);
        assert_eq!(
            codes("void main() {\n    if true {\n    }\n    else {\n    }\n}\n"),
            ["R3"]
        );
    }

    #[test]
    fn continuation_and_interpolation() {
        let src = "int add(int a,\n        int b) {\n    return a +\n        b\n}\n\nvoid main() {\n    print(\"sum {add(1, 2)}\")\n}\n";
        assert_eq!(codes(src), [] as [&str; 0]);
    }

    #[test]
    fn one_cause_one_message() {
        // Python body below a foreign header is part of the same mistake
        assert_eq!(
            codes("def main():\n    print(\"hi\")\n\nvoid f() {\n}\n"),
            ["R2"]
        );
        // a missing `}` is reported once, at the innermost open block
        let d = check_source("void main() {\n    if true {\n");
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].code, d[0].pos.line, d[0].pos.col), ("S1", 2, 13));
    }
}
