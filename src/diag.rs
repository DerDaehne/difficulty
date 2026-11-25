//! Diagnostics: one cause, one message, with a rule or lexical/syntax code.

/// A position in the source: line and column count from 1, columns in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub code: &'static str,
    pub pos: Pos,
    pub msg: String,
    pub hint: Option<String>,
}

impl Diag {
    pub fn new(code: &'static str, pos: Pos, msg: impl Into<String>) -> Self {
        Diag {
            code,
            pos,
            msg: msg.into(),
            hint: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn render(&self, file: &str) -> String {
        let mut s = format!(
            "{}:{}:{}: error[{}]: {}",
            file, self.pos.line, self.pos.col, self.code, self.msg
        );
        if let Some(h) = &self.hint {
            s.push_str("\n  hint: ");
            s.push_str(h);
        }
        s
    }

    pub fn json(&self, file: &str) -> String {
        let mut s = format!(
            "{{\"v\":1,\"code\":{},\"severity\":\"error\",\"file\":{},\"line\":{},\"col\":{},\"offset\":{},\"message\":{}",
            json_str(self.code),
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
            "R13",
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
            "{\"v\":1,\"code\":\"R13\",\"severity\":\"error\",\"file\":\"main.dif\",\"line\":6,\"col\":5,\"offset\":70,\"message\":\"result of `f(3)` is never used\",\"hint\":\"use it, or discard it explicitly with `_ = f(3)`\"}"
        );
    }
}
