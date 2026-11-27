//! Diagnostics: one cause, one message, with a rule or lexical/syntax code.

/// A position in the source: line and column count from 1, columns in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
    pub offset: u32,
}

/// A mechanical repair: replace the bytes `[offset, end)` of the source with `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    pub offset: u32,
    pub end: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub code: &'static str,
    /// Stable kind of message, `<code>.<name>` (spec §0.3).
    pub id: &'static str,
    pub pos: Pos,
    pub msg: String,
    pub hint: Option<String>,
    pub fix: Option<Box<Fix>>,
}

impl Diag {
    /// `note` for a hint (code `H…`), else `error`.
    pub fn severity(&self) -> &'static str {
        if self.code.starts_with('H') {
            "note"
        } else {
            "error"
        }
    }

    /// `id` is `<code>.<name>`; a bare code (no name yet) is its own id, which the
    /// id test rejects.
    pub fn new(id: &'static str, pos: Pos, msg: impl Into<String>) -> Self {
        let code = id.split_once('.').map_or(id, |(c, _)| c);
        Diag {
            code,
            id,
            pos,
            msg: msg.into(),
            hint: None,
            fix: None,
        }
    }

    pub fn fix(mut self, offset: u32, end: u32, text: impl Into<String>) -> Self {
        self.fix = Some(Box::new(Fix {
            offset,
            end,
            text: text.into(),
        }));
        self
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn render(&self, file: &str) -> String {
        let mut s = format!(
            "{}:{}:{}: {}[{}]: {}",
            file,
            self.pos.line,
            self.pos.col,
            self.severity(),
            self.code,
            self.msg
        );
        if let Some(h) = &self.hint {
            s.push_str("\n  hint: ");
            s.push_str(h);
        }
        s
    }

    pub fn json(&self, file: &str) -> String {
        let mut s = format!(
            "{{\"v\":1,\"code\":{},\"id\":{},\"severity\":{},\"file\":{},\"line\":{},\"col\":{},\"offset\":{},\"message\":{}",
            json_str(self.code),
            json_str(self.id),
            json_str(self.severity()),
            json_str(file),
            self.pos.line,
            self.pos.col,
            self.pos.offset,
            json_str(&self.msg)
        );
        if let Some(h) = &self.hint {
            s.push_str(",\"hint\":");
            s.push_str(&json_str(h));
        }
        if let Some(f) = &self.fix {
            s.push_str(&format!(
                ",\"fix\":{{\"offset\":{},\"end\":{},\"text\":{}}}",
                f.offset,
                f.end,
                json_str(&f.text)
            ));
        }
        s.push('}');
        s
    }
}

pub fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_and_json() {
        let d = Diag::new(
            "R13.unused_result",
            Pos {
                line: 6,
                col: 5,
                offset: 70,
            },
            "result of `f(3)` is never used",
        )
        .hint("use it, or discard it explicitly with `_ = f(3)`");
        assert_eq!(
            d.render("main.dif"),
            "main.dif:6:5: error[R13]: result of `f(3)` is never used\n  hint: use it, or discard it explicitly with `_ = f(3)`"
        );
        assert_eq!(
            d.json("main.dif"),
            "{\"v\":1,\"code\":\"R13\",\"id\":\"R13.unused_result\",\"severity\":\"error\",\"file\":\"main.dif\",\"line\":6,\"col\":5,\"offset\":70,\"message\":\"result of `f(3)` is never used\",\"hint\":\"use it, or discard it explicitly with `_ = f(3)`\"}"
        );
    }
}
