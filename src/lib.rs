//! The `dif` compiler: lexer → parser → (later) checker → C emitter.

pub mod ast;
pub mod diag;
pub mod lexer;
pub mod parser;

use diag::Diag;

/// Lexes and parses one source file; returns all diagnostics in source order.
pub fn check_source(src: &str) -> Vec<Diag> {
    let (toks, mut diags) = lexer::lex(src);
    let (_module, pdiags) = parser::parse(src, &toks, &diags);
    diags.extend(pdiags);
    diags.sort_by_key(|d| d.pos.offset);
    diags
}

/// Like `check_source`, starting from raw bytes (invalid UTF-8 is `L1`).
pub fn check_bytes(bytes: &[u8]) -> Vec<Diag> {
    match lexer::decode(bytes) {
        Ok(src) => check_source(src),
        Err(d) => vec![d],
    }
}
