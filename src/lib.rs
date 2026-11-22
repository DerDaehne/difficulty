//! The `dif` compiler: lexer → parser → checker → (later) C emitter.

pub mod ast;
pub mod checker;
pub mod diag;
pub mod lexer;
pub mod parser;

use diag::Diag;

/// Checks one source file; returns all diagnostics in source order. Names and
/// types are checked only when the file has no lexical or syntax errors, so a
/// broken statement causes no follow-up diagnostics.
pub fn check_file(file: &str, src: &str) -> Vec<Diag> {
    let (toks, mut diags) = lexer::lex(src);
    let (module, pdiags) = parser::parse(src, &toks, &diags);
    diags.extend(pdiags);
    if diags.is_empty() {
        diags = checker::check(file, src, &module);
    }
    diags.sort_by_key(|d| d.pos.offset);
    diags
}

/// `check_file` for a file named `main.dif`, as in the examples of the spec.
pub fn check_source(src: &str) -> Vec<Diag> {
    check_file("main.dif", src)
}

/// Like `check_file`, starting from raw bytes (invalid UTF-8 is `L1`).
pub fn check_bytes(file: &str, bytes: &[u8]) -> Vec<Diag> {
    match lexer::decode(bytes) {
        Ok(src) => check_file(file, src),
        Err(d) => vec![d],
    }
}
