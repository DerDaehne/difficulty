//! Lexer (spec §1). Produces tokens with positions, the `Nl` token for statement
//! ends (§1.3) and lexical diagnostics L1–L8.

use crate::diag::{Diag, Pos};

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Kw(&'static str),
    Int(u128),
    Float(String),
    Str(Vec<StrPart>),
    Op(&'static str),
    Semi,
    Nl,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Lit(String),
    /// Tokens of an interpolation `{expr}`, ending with `Eof`.
    Expr(Vec<Token>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
    /// Byte offset just after the token.
    pub end: u32,
}

/// Keywords, reserved words and reserved literals (spec §1.5): never names.
pub const KEYWORDS: &[&str] = &[
    "const", "struct", "enum", "if", "else", "while", "for", "in", "break", "continue", "return",
    "switch", "case", "default", "try", "or", "mut", "use", "pub", "test", "extern", "unsafe",
    "void", "self", "defer", "spawn", "parallel", "yield", "true", "false", "none", "some", "ok",
    "err",
];

/// Operators and punctuation, longest first.
const OPS: &[&str] = &[
    "<<=", ">>=", "+%", "-%", "*%", "<<", ">>", "==", "!=", "<=", ">=", "&&", "||", "??", "+=",
    "-=", "*=", "/=", "%=", "&=", "|=", "^=", "++", "--", "+", "-", "*", "/", "%", "&", "|", "^",
    "~", "!", "<", ">", "=", "(", ")", "[", "]", "{", "}", ",", ".", ":",
];

/// A line that ends with one of these continues on the next line (§1.3, rule 2).
const CONTINUES: &[&str] = &[
    "+", "-", "*", "/", "%", "+%", "-%", "*%", "&", "|", "^", "<<", ">>", "==", "!=", "<", "<=",
    ">", ">=", "&&", "||", "??", "=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=",
    ",",
];

/// Turns raw bytes into text; invalid UTF-8 is `L1`.
pub fn decode(bytes: &[u8]) -> Result<&str, Diag> {
    std::str::from_utf8(bytes).map_err(|e| {
        let at = e.valid_up_to();
        let before = &bytes[..at];
        let line = before.iter().filter(|&&b| b == b'\n').count() as u32 + 1;
        let line_start = before
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1);
        let col = String::from_utf8_lossy(&before[line_start..])
            .chars()
            .count() as u32
            + 1;
        Diag::new(
            "L1",
            Pos {
                line,
                col,
                offset: at as u32,
            },
            "invalid UTF-8 in source file",
        )
    })
}

pub fn lex(src: &str) -> (Vec<Token>, Vec<Diag>) {
    let mut lx = Lexer::new(
        src,
        Pos {
            line: 1,
            col: 1,
            offset: 0,
        },
        false,
    );
    lx.run();
    (lx.toks, lx.diags)
}

struct Lexer<'a> {
    src: &'a str,
    chars: Vec<(usize, char)>,
    i: usize,
    line: u32,
    col: u32,
    base: u32,
    /// Inside an interpolation: no `Nl` tokens.
    fragment: bool,
    brackets: Vec<char>,
    toks: Vec<Token>,
    diags: Vec<Diag>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, start: Pos, fragment: bool) -> Self {
        Lexer {
            src,
            chars: src.char_indices().collect(),
            i: 0,
            line: start.line,
            col: start.col,
            base: start.offset,
            fragment,
            brackets: Vec::new(),
            toks: Vec::new(),
            diags: Vec::new(),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).map(|&(_, c)| c)
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.chars.get(self.i + n).map(|&(_, c)| c)
    }

    fn byte(&self) -> u32 {
        self.chars.get(self.i).map_or(self.src.len(), |&(b, _)| b) as u32 + self.base
    }

    fn pos(&self) -> Pos {
        Pos {
            line: self.line,
            col: self.col,
            offset: self.byte(),
        }
    }

    fn adv(&mut self) {
        if let Some(c) = self.peek() {
            self.i += 1;
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
    }

    fn push(&mut self, tok: Tok, pos: Pos) {
        let end = self.byte();
        self.toks.push(Token { tok, pos, end });
    }

    fn err(&mut self, code: &'static str, pos: Pos, msg: impl Into<String>, hint: Option<&str>) {
        let mut d = Diag::new(code, pos, msg);
        if let Some(h) = hint {
            d = d.hint(h);
        }
        self.diags.push(d);
    }

    fn run(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                '\n' => self.newline(),
                ' ' | '\t' | '\r' => self.adv(),
                '/' if self.peek_at(1) == Some('/') => {
                    while !matches!(self.peek(), None | Some('\n')) {
                        self.adv();
                    }
                }
                '/' if self.peek_at(1) == Some('*') => self.block_comment(),
                '"' => self.string(),
                '0'..='9' => self.number(),
                c if c.is_ascii_alphabetic() || c == '_' => self.ident(),
                ';' => {
                    let p = self.pos();
                    self.adv();
                    self.push(Tok::Semi, p);
                }
                _ => self.op(),
            }
        }
        let p = self.pos();
        self.push(Tok::Eof, p);
    }

    fn newline(&mut self) {
        let p = self.pos();
        self.adv();
        if self.fragment || matches!(self.brackets.last(), Some('(') | Some('[')) {
            return;
        }
        match self.toks.last() {
            None => return,
            Some(t) if t.tok == Tok::Nl => return,
            Some(Token {
                tok: Tok::Op(o), ..
            }) if CONTINUES.contains(o) => return,
            _ => {}
        }
        if self.next_line_starts_with_dot() {
            return;
        }
        self.toks.push(Token {
            tok: Tok::Nl,
            pos: p,
            end: p.offset + 1,
        });
    }

    fn next_line_starts_with_dot(&self) -> bool {
        let mut j = self.i;
        while let Some(&(_, c)) = self.chars.get(j) {
            match c {
                ' ' | '\t' | '\r' | '\n' => j += 1,
                '/' if self.chars.get(j + 1).map(|&(_, c)| c) == Some('/') => {
                    while !matches!(self.chars.get(j).map(|&(_, c)| c), None | Some('\n')) {
                        j += 1;
                    }
                }
                '.' => return true,
                _ => return false,
            }
        }
        false
    }

    fn block_comment(&mut self) {
        let p = self.pos();
        self.err(
            "L2",
            p,
            "block comments `/* … */` do not exist",
            Some("use // for comments"),
        );
        self.adv();
        self.adv();
        while let Some(c) = self.peek() {
            if c == '*' && self.peek_at(1) == Some('/') {
                self.adv();
                self.adv();
                return;
            }
            self.adv();
        }
    }

    fn ident(&mut self) {
        let p = self.pos();
        let start = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
            self.adv();
        }
        let text: String = self.chars[start..self.i].iter().map(|&(_, c)| c).collect();
        let tok = match KEYWORDS.iter().find(|&&k| k == text) {
            Some(k) => Tok::Kw(k),
            None => Tok::Ident(text),
        };
        self.push(tok, p);
    }

    fn number(&mut self) {
        let p = self.pos();
        let first = self.peek().unwrap_or('0');
        if first == '0' && matches!(self.peek_at(1), Some('x' | 'X' | 'b' | 'B')) {
            let radix = if matches!(self.peek_at(1), Some('x' | 'X')) {
                16
            } else {
                2
            };
            self.adv();
            self.adv();
            let digits = self.take_digits(|c| c.is_digit(radix));
            self.push_int(&digits, radix, p);
            return;
        }
        let int_part = self.take_digits(|c| c.is_ascii_digit());
        let is_float =
            self.peek() == Some('.') && matches!(self.peek_at(1), Some(c) if c.is_ascii_digit());
        let has_exp = matches!(self.peek(), Some('e' | 'E'))
            && (matches!(self.peek_at(1), Some(c) if c.is_ascii_digit())
                || (matches!(self.peek_at(1), Some('+' | '-'))
                    && matches!(self.peek_at(2), Some(c) if c.is_ascii_digit())));
        if is_float || has_exp {
            let mut text = int_part;
            if is_float {
                self.adv();
                text.push('.');
                text.push_str(&self.take_digits(|c| c.is_ascii_digit()));
            }
            if matches!(self.peek(), Some('e' | 'E')) {
                text.push('e');
                self.adv();
                if let Some(s @ ('+' | '-')) = self.peek() {
                    text.push(s);
                    self.adv();
                }
                text.push_str(&self.take_digits(|c| c.is_ascii_digit()));
            }
            self.push(Tok::Float(text), p);
            return;
        }
        if int_part.len() > 1 && int_part.starts_with('0') {
            let trimmed = int_part.trim_start_matches('0');
            let hint = format!(
                "write {}; octal literals do not exist",
                if trimmed.is_empty() { "0" } else { trimmed }
            );
            self.err("L8", p, "integer literal with a leading zero", Some(&hint));
        }
        self.push_int(&int_part, 10, p);
    }

    /// Digits accepted by `ok`, with `_` separators removed.
    fn take_digits(&mut self, ok: impl Fn(char) -> bool) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if ok(c) {
                s.push(c);
            } else if c != '_' {
                break;
            }
            self.adv();
        }
        s
    }

    fn push_int(&mut self, digits: &str, radix: u32, p: Pos) {
        let mut v: u128 = 0;
        let mut too_big = false;
        for c in digits.chars() {
            v = v * radix as u128 + c.to_digit(radix).unwrap_or(0) as u128;
            if v > u64::MAX as u128 {
                too_big = true;
                v = 0;
                break;
            }
        }
        if too_big {
            self.err("L4", p, "integer literal is larger than 2^64 - 1", None);
        } else if digits.is_empty() {
            self.err("S1", p, "number prefix without digits", None);
        }
        self.push(Tok::Int(v), p);
    }

    fn string(&mut self) {
        let p = self.pos();
        self.adv();
        let mut parts = Vec::new();
        let mut lit = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    self.err(
                        "L5",
                        p,
                        "string is not closed on this line",
                        Some("add `\"` at the end of the string"),
                    );
                    break;
                }
                Some('"') => {
                    self.adv();
                    break;
                }
                Some('\\') => self.escape(&mut lit),
                Some('{') => {
                    if !lit.is_empty() {
                        parts.push(StrPart::Lit(std::mem::take(&mut lit)));
                    }
                    if let Some(toks) = self.interpolation() {
                        parts.push(StrPart::Expr(toks));
                    }
                }
                Some('}') => {
                    let q = self.pos();
                    self.err(
                        "L6",
                        q,
                        "single `}` in a string",
                        Some("write \\} for a literal brace"),
                    );
                    self.adv();
                }
                Some(c) => {
                    lit.push(c);
                    self.adv();
                }
            }
        }
        if !lit.is_empty() {
            parts.push(StrPart::Lit(lit));
        }
        self.push(Tok::Str(parts), p);
    }

    fn escape(&mut self, lit: &mut String) {
        let p = self.pos();
        self.adv();
        let c = match self.peek() {
            Some('n') => '\n',
            Some('t') => '\t',
            Some('r') => '\r',
            Some('0') => '\0',
            Some(c @ ('\\' | '"' | '{' | '}')) => c,
            Some('u') if self.peek_at(1) == Some('{') => {
                self.adv();
                self.adv();
                let hex = self.take_digits(|c| c.is_ascii_hexdigit());
                if self.peek() == Some('}') {
                    self.adv();
                    match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        Some(c) => lit.push(c),
                        None => self.err("L6", p, "invalid unicode escape", None),
                    }
                } else {
                    self.err("L6", p, "unclosed unicode escape", Some("write \\u{1F600}"));
                }
                return;
            }
            _ => {
                self.err(
                    "L6",
                    p,
                    "unknown escape sequence",
                    Some("known escapes: \\n \\t \\r \\\\ \\\" \\{ \\} \\0 \\u{…}"),
                );
                return;
            }
        };
        lit.push(c);
        self.adv();
    }

    /// Lexes `{expr}` into its own token list (§1.4).
    fn interpolation(&mut self) -> Option<Vec<Token>> {
        let open = self.pos();
        self.adv();
        let start = self.pos();
        let from = self.i;
        loop {
            match self.peek() {
                Some('}') => break,
                None | Some('\n') => return None, // reported as L5 by the caller
                Some(c @ ('"' | '{')) => {
                    let q = self.pos();
                    self.err(
                        "S1",
                        q,
                        format!("`{c}` inside an interpolation"),
                        Some("compute the value in a variable first"),
                    );
                    while !matches!(self.peek(), None | Some('\n') | Some('}')) {
                        self.adv();
                    }
                    if self.peek() == Some('}') {
                        self.adv();
                    }
                    return None;
                }
                Some(_) => self.adv(),
            }
        }
        let a = self.chars[from].0;
        let b = self.chars[self.i].0;
        let text = &self.src[a..b];
        self.adv();
        if text.trim().is_empty() {
            self.err(
                "S1",
                open,
                "empty interpolation `{}`",
                Some("write \\{\\} for literal braces"),
            );
            return None;
        }
        let mut sub = Lexer::new(text, start, true);
        sub.run();
        self.diags.extend(sub.diags);
        Some(sub.toks)
    }

    fn op(&mut self) {
        let p = self.pos();
        let rest = &self.src[self.chars[self.i].0..];
        if let Some(op) = OPS.iter().find(|o| rest.starts_with(**o)) {
            for _ in 0..op.chars().count() {
                self.adv();
            }
            match *op {
                "(" => self.brackets.push('('),
                "[" => self.brackets.push('['),
                "{" => self.brackets.push('{'),
                ")" | "]" | "}" => {
                    self.brackets.pop();
                }
                _ => {}
            }
            self.push(Tok::Op(op), p);
            return;
        }
        let c = self.peek().unwrap_or(' ');
        self.adv();
        if c.is_ascii() {
            self.err("L1", p, format!("unexpected character `{c}`"), None);
        } else {
            self.err(
                "L1",
                p,
                format!("`{c}` is only allowed in strings and comments"),
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Tok> {
        lex(src).0.into_iter().map(|t| t.tok).collect()
    }

    fn codes(src: &str) -> Vec<&'static str> {
        lex(src).1.into_iter().map(|d| d.code).collect()
    }

    #[test]
    fn newline_rules() {
        // inside parentheses, after an operator, before a leading dot: no Nl
        assert!(!kinds("f(1,\n2)").contains(&Tok::Nl));
        assert!(!kinds("a +\nb").contains(&Tok::Nl));
        assert!(!kinds("a\n  .b()").contains(&Tok::Nl));
        assert_eq!(kinds("a\nb").iter().filter(|t| **t == Tok::Nl).count(), 1);
        assert_eq!(
            kinds("a\n\n// c\n\nb")
                .iter()
                .filter(|t| **t == Tok::Nl)
                .count(),
            1
        );
    }

    #[test]
    fn lexical_errors() {
        assert_eq!(codes("/* x */"), ["L2"]);
        assert_eq!(codes("x = 1;").len(), 0); // `;` is reported by the parser
        assert_eq!(codes("18446744073709551616"), ["L4"]);
        assert_eq!(codes("18446744073709551615"), [] as [&str; 0]);
        assert_eq!(codes("\"abc"), ["L5"]);
        assert_eq!(codes("\"a\\q\""), ["L6"]);
        assert_eq!(codes("\"a}\""), ["L6"]);
        assert_eq!(codes("010"), ["L8"]);
        assert_eq!(codes("@"), ["L1"]);
        assert_eq!(codes("ä"), ["L1"]);
    }

    #[test]
    fn literals_and_interpolation() {
        assert_eq!(
            kinds("1_000 0x_ff 0b101 2.5 1e3")[..5],
            [
                Tok::Int(1000),
                Tok::Int(255),
                Tok::Int(5),
                Tok::Float("2.5".into()),
                Tok::Float("1e3".into())
            ]
        );
        let toks = kinds("\"fib({i}) = \\{x\\}\"");
        let Tok::Str(parts) = &toks[0] else {
            panic!("no string")
        };
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], StrPart::Lit("fib(".into()));
        assert!(matches!(&parts[1], StrPart::Expr(t) if t[0].tok == Tok::Ident("i".into())));
        assert_eq!(parts[2], StrPart::Lit(") = {x}".into()));
    }

    #[test]
    fn positions_count_characters() {
        let (toks, _) = lex("\"ä\" x");
        assert_eq!((toks[1].pos.line, toks[1].pos.col), (1, 5));
    }
}
