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
    "~", "!", "?", "<", ">", "=", "(", ")", "[", "]", "{", "}", ",", ".", ":",
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
    /// Names with non-ASCII letters already reported (L1).
    foreign_words: Vec<String>,
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
            foreign_words: Vec::new(),
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
                c if c.is_alphabetic() || c == '_' => self.ident(),
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
        self.line_end(p);
    }

    /// The end of a line at `p`: an `Nl` token unless §1.3 continues the line.
    fn line_end(&mut self, p: Pos) {
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
        if !self.src[self.chars[self.i].0..].contains("*/") {
            // never closed: it ends with its line instead of swallowing the file
            while !matches!(self.peek(), None | Some('\n')) {
                self.adv();
            }
            return;
        }
        self.adv();
        self.adv();
        let mut line_break = None;
        while let Some(c) = self.peek() {
            if c == '*' && self.peek_at(1) == Some('/') {
                self.adv();
                self.adv();
                break;
            }
            if c == '\n' && line_break.is_none() {
                line_break = Some(self.pos());
            }
            self.adv();
        }
        // a comment over several lines ends its line like a line break
        if let Some(q) = line_break {
            self.line_end(q);
        }
    }

    /// A word. Letters outside ASCII are `L1` once per word; the word stays one token.
    fn ident(&mut self) {
        let p = self.pos();
        let start = self.i;
        let mut foreign = None;
        while let Some(c) = self.peek().filter(|&c| c.is_alphanumeric() || c == '_') {
            if !c.is_ascii() && foreign.is_none() {
                foreign = Some((c, self.pos()));
            }
            self.adv();
        }
        let text: String = self.chars[start..self.i].iter().map(|&(_, c)| c).collect();
        if let Some((c, q)) = foreign {
            // a name is reported where it first occurs, not at every use
            if !self.foreign_words.contains(&text) {
                self.foreign_words.push(text.clone());
                self.err(
                    "L1",
                    q,
                    format!("`{c}` is only allowed in strings and comments"),
                    Some("names use ASCII letters, digits and _"),
                );
            }
        }
        let tok = match KEYWORDS.iter().find(|&&k| k == text) {
            Some(k) => Tok::Kw(k),
            None => Tok::Ident(text),
        };
        self.push(tok, p);
    }

    /// Number literals exactly as in spec §1.4; every other form is `L8` and still
    /// one token, so the parser sees no follow-up error.
    fn number(&mut self) {
        let p = self.pos();
        let start = self.i;
        if self.peek() == Some('0') && matches!(self.peek_at(1), Some('x' | 'X' | 'b' | 'B')) {
            let mark = self.peek_at(1).unwrap_or('x');
            let (radix, prefix, kind) = if mark.eq_ignore_ascii_case(&'x') {
                (16, "0x", "hex")
            } else {
                (2, "0b", "binary")
            };
            self.adv();
            self.adv();
            let underscore = self.peek() == Some('_');
            let digits = self.take_digits(|c| c.is_digit(radix));
            if self.bad_tail(p, kind) {
                return;
            }
            if digits.is_empty() {
                let msg = format!("`{prefix}` without digits");
                self.err("L8", p, msg, None);
            } else if mark.is_ascii_uppercase() || underscore {
                let msg = format!("write the prefix as `{prefix}`, directly followed by a digit");
                let hint = format!("write {prefix}{}", digits.to_lowercase());
                self.err("L8", p, msg, Some(&hint));
            }
            self.push_int(&digits, radix, p);
            return;
        }
        let int_part = self.take_digits(|c| c.is_ascii_digit());
        let mut text = int_part.clone();
        let mut float = false;
        if self.peek() == Some('.') && matches!(self.peek_at(1), Some(c) if c.is_ascii_digit()) {
            float = true;
            self.adv();
            text.push('.');
            text.push_str(&self.take_digits(|c| c.is_ascii_digit()));
        }
        let exp_digit = |c: Option<char>| matches!(c, Some(c) if c.is_ascii_digit());
        if matches!(self.peek(), Some('e' | 'E'))
            && (exp_digit(self.peek_at(1))
                || (matches!(self.peek_at(1), Some('+' | '-')) && exp_digit(self.peek_at(2))))
        {
            float = true;
            self.adv();
            text.push('e');
            if let Some(s @ ('+' | '-')) = self.peek() {
                text.push(s);
                self.adv();
            }
            while let Some(c) = self.peek().filter(|c| c.is_ascii_digit()) {
                text.push(c);
                self.adv();
            }
            if self.peek() == Some('_') {
                self.skip_word();
                self.err("L8", p, "`_` is not allowed in an exponent", None);
                self.push(Tok::Float(text), p);
                return;
            }
        }
        if self.bad_tail(p, "decimal") {
            return;
        }
        if float {
            self.push(Tok::Float(text), p);
            return;
        }
        let raw: String = self.chars[start..self.i].iter().map(|&(_, c)| c).collect();
        if raw.len() > 1 && raw.starts_with('0') {
            let trimmed = int_part.trim_start_matches('0');
            let hint = format!(
                "write {}; octal literals do not exist",
                if trimmed.is_empty() { "0" } else { trimmed }
            );
            self.err("L8", p, "integer literal with a leading zero", Some(&hint));
        }
        self.push_int(&int_part, 10, p);
    }

    /// Letters, digits or `_` directly after a number: one `L8` for the whole word.
    fn bad_tail(&mut self, p: Pos, kind: &str) -> bool {
        let Some(c) = self.peek().filter(|&c| c.is_alphanumeric() || c == '_') else {
            return false;
        };
        let exponent = kind == "decimal" && matches!(c, 'e' | 'E');
        self.skip_word();
        if exponent && matches!(self.peek(), Some('+' | '-')) {
            self.adv();
        }
        let msg = if exponent {
            "exponent without digits".to_string()
        } else if kind != "decimal" && c.is_ascii_alphanumeric() {
            format!("`{c}` is not a {kind} digit")
        } else {
            format!("a number cannot be directly followed by `{c}`")
        };
        self.err("L8", p, msg, None);
        self.push(Tok::Int(0), p);
        true
    }

    fn skip_word(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_alphanumeric() || c == '_') {
            self.adv();
        }
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
                    // the line ends here: brackets opened on it cannot hold the next lines
                    while matches!(self.brackets.last(), Some('(' | '[')) {
                        self.brackets.pop();
                    }
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
                let mut hex = String::new();
                while let Some(c) = self.peek().filter(|c| c.is_ascii_hexdigit()) {
                    hex.push(c);
                    self.adv();
                }
                if self.peek() == Some('}') && !hex.is_empty() {
                    self.adv();
                    match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        Some(c) => lit.push(c),
                        None => {
                            let msg = format!("`\\u{{{hex}}}` is not a valid character");
                            self.err("L6", p, msg, None);
                        }
                    }
                } else {
                    self.err(
                        "L6",
                        p,
                        "invalid unicode escape",
                        Some("write hex digits in braces: \\u{1F600}"),
                    );
                    while !matches!(self.peek(), None | Some('\n' | '"' | '}')) {
                        self.adv();
                    }
                    if self.peek() == Some('}') {
                        self.adv();
                    }
                }
                return;
            }
            None | Some('\n') => return, // the caller reports the open string (L5)
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
                Some('"') if !self.rest_of_line_has('}') => {
                    // the string ends here; the `{` was meant literally or never closed
                    self.err(
                        "L6",
                        open,
                        "`{` in a string is not closed",
                        Some("close it with `}`, or write \\{ for a literal brace"),
                    );
                    return None;
                }
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
        let mut text = &self.src[a..b];
        self.adv();
        if let Some(k) = text.find(':') {
            let q = Pos {
                line: start.line,
                col: start.col + text[..k].chars().count() as u32,
                offset: start.offset + k as u32,
            };
            let msg = "format specifications like `{x:.2f}` are not available yet";
            self.err("S1", q, msg, None);
            text = &text[..k];
            if text.trim().is_empty() {
                return None;
            }
        }
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
                ")" if self.brackets.last() == Some(&'(') => {
                    self.brackets.pop();
                }
                "]" if self.brackets.last() == Some(&'[') => {
                    self.brackets.pop();
                }
                "}" => {
                    // also closes `(`/`[` left open inside the block
                    while let Some(b) = self.brackets.pop() {
                        if b == '{' {
                            break;
                        }
                    }
                }
                _ => {}
            }
            self.push(Tok::Op(op), p);
            return;
        }
        let c = self.peek().unwrap_or(' ');
        self.adv();
        let invisible = c.is_whitespace()
            || c.is_control()
            || matches!(c, '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}');
        let (msg, hint) = if invisible {
            let hint = match c {
                '\u{FEFF}' => "save the file as UTF-8 without BOM",
                c if c.is_whitespace() => "replace it with a normal space",
                _ => "remove it",
            };
            let msg = format!(
                "invisible character U+{:04X} is only allowed in strings and comments",
                c as u32
            );
            (msg, Some(hint))
        } else if c.is_ascii() {
            (format!("unexpected character `{c}`"), None)
        } else {
            (
                format!("`{c}` is only allowed in strings and comments"),
                None,
            )
        };
        self.err("L1", p, msg, hint);
    }

    fn rest_of_line_has(&self, want: char) -> bool {
        self.chars[self.i..]
            .iter()
            .map(|&(_, c)| c)
            .take_while(|&c| c != '\n')
            .any(|c| c == want)
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
        // one L1 per word, and the word stays one token
        assert_eq!(codes("größe"), ["L1"]);
        assert_eq!(kinds("größe").len(), 2);
        let d = &lex("\u{FEFF}x").1[0];
        assert!(d.msg.starts_with("invisible character U+FEFF"), "{}", d.msg);
        // \u{…} takes hex digits only
        assert_eq!(codes("\"\\u{1_F600}\""), ["L6"]);
        assert_eq!(codes("\"\\u{1F600}\""), [] as [&str; 0]);
        // an unclosed `{` ends at the string's end, without L5
        assert_eq!(codes("\"a {x\""), ["L6"]);
    }

    #[test]
    fn number_forms() {
        // every form outside the grammar of spec §1.4 is one L8, and one token
        for src in [
            "0X10", "0B11", "0x_ff", "0b_1", "0x", "0b", "1e1_0", "0b102", "0xfg", "1e", "1e+",
            "12ab", "0_1", "0_",
        ] {
            assert_eq!(codes(src), ["L8"], "{src}");
            assert_eq!(kinds(src).len(), 2, "{src}");
        }
        for src in [
            "0", "0.5", "1_000", "0xff_ff", "0b1_0", "1e10", "2.5e-3", "1_0.0_1",
        ] {
            assert_eq!(codes(src), [] as [&str; 0], "{src}");
        }
    }

    #[test]
    fn brackets_close_in_order() {
        // `}` closes a `(` left open in its block, so later lines get their Nl again
        let n = kinds("{ f(\n}\na\nb")
            .iter()
            .filter(|t| **t == Tok::Nl)
            .count();
        assert_eq!(n, 2);
        // an unclosed string ends its line and the brackets opened on it
        let n = kinds("f(\"abc\nb\nc")
            .iter()
            .filter(|t| **t == Tok::Nl)
            .count();
        assert_eq!(n, 2);
    }

    #[test]
    fn literals_and_interpolation() {
        assert_eq!(
            kinds("1_000 0xff 0b101 2.5 1e3")[..5],
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
