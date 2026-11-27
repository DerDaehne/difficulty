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

/// Like `check_bytes`; a file without errors gets its hints (`H` codes, spec §0.1) instead.
pub fn check_hints(file: &str, bytes: &[u8]) -> Vec<Diag> {
    match lexer::decode(bytes) {
        Ok(src) => on_big_stack(|| match front(file, src) {
            Ok((_, info)) => info.hints,
            Err(d) => d,
        }),
        Err(d) => vec![d],
    }
}

/// C source of a program, or its diagnostics (a program needs `void main()`, R1).
pub fn compile(file: &str, bytes: &[u8]) -> Result<String, Vec<Diag>> {
    let src = lexer::decode(bytes).map_err(|d| vec![d])?;
    on_big_stack(|| {
        let (module, info) = front(file, src)?;
        if let Some(d) = checker::main_diag(&module) {
            return Err(vec![d]);
        }
        Ok(emit_c::emit(file, src, &module, &info))
    })
}

fn front(file: &str, src: &str) -> Result<(ast::Module, checker::Info), Vec<Diag>> {
    let (toks, mut diags) = lexer::lex(src);
    let (module, pdiags) = parser::parse(src, &toks, &diags);
    diags.extend(pdiags);
    if diags.is_empty() {
        let (cdiags, info) = checker::check(file, src, &module);
        if cdiags.is_empty() {
            return Ok((module, info));
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

/// One `test` block of a module, in the order of the C test binary (R18).
pub struct TestMeta {
    pub name: String,
    /// Line of the word `test`.
    pub line: u32,
    pub panics: Option<String>,
}

/// C source of the test binary of a module and its tests. Unlike `compile` it does not
/// need `void main()`; a `main` that is there must still be well formed (R1).
pub fn compile_tests(file: &str, bytes: &[u8]) -> Result<(String, Vec<TestMeta>), Vec<Diag>> {
    let src = lexer::decode(bytes).map_err(|d| vec![d])?;
    on_big_stack(|| {
        let (module, info) = front(file, src)?;
        let has_main = module
            .decls
            .iter()
            .any(|d| matches!(d, ast::Decl::Func(f) if f.name == "main" && f.recv.is_none()));
        if let Some(d) = checker::main_diag(&module).filter(|_| has_main) {
            return Err(vec![d]);
        }
        let tests = (module.decls.iter())
            .filter_map(|d| match d {
                ast::Decl::Test(t) => Some(TestMeta {
                    name: t.name.clone(),
                    line: t.pos.line,
                    panics: t.panics.as_ref().map(|p| p.0.clone()),
                }),
                _ => None,
            })
            .collect();
        Ok((emit_c::emit_tests(file, src, &module, &info), tests))
    })
}
