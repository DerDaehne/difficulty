//! The `dif` compiler: lexer → parser → checker → C emitter.

pub mod ast;
pub mod checker;
pub mod diag;
pub mod emit_c;
pub mod lexer;
pub mod live;
pub mod parser;

use diag::Diag;

/// Checks one source file; returns all diagnostics in source order. Names and
/// types are checked only when the file has no lexical or syntax errors, so a
/// broken statement causes no follow-up diagnostics.
pub fn check_file(file: &str, src: &str) -> Vec<Diag> {
    on_big_stack(|| front(file, src).err().unwrap_or_default())
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

/// C source of a program, or its diagnostics (a program needs `void main()`, R1).
pub fn compile(file: &str, bytes: &[u8]) -> Result<String, Vec<Diag>> {
    let src = lexer::decode(bytes).map_err(|d| vec![d])?;
    on_big_stack(|| {
        let (module, folded) = front(file, src)?;
        if let Some(d) = checker::main_diag(&module) {
            return Err(vec![d]);
        }
        Ok(emit_c::emit(file, &module, &folded))
    })
}

fn front(file: &str, src: &str) -> Result<(ast::Module, checker::Folded), Vec<Diag>> {
    let (toks, mut diags) = lexer::lex(src);
    let (module, pdiags) = parser::parse(src, &toks, &diags);
    diags.extend(pdiags);
    if diags.is_empty() {
        let (cdiags, folded) = checker::check(file, src, &module);
        if cdiags.is_empty() {
            return Ok((module, folded));
        }
        diags = cdiags;
    }
    diags.sort_by_key(|d| d.pos.offset);
    Err(diags)
}

/// Parser, checker and emitter recurse once per nesting level (bounded by the
/// S1/R4 limits); their own thread gives that depth room in debug builds too.
fn on_big_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn_scoped(s, f)
            .expect("cannot start the compiler thread")
            .join()
            .unwrap_or_else(|e| std::panic::resume_unwind(e))
    })
}
