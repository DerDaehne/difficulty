//! Diagnostics of `dif check` for the first language subset: codes, positions,
//! hints, and the rule "one cause, one message" (spec §0.1).

use dif::check_source;
use dif::diag::Diag;

fn codes(src: &str) -> Vec<&'static str> {
    check_source(src).into_iter().map(|d| d.code).collect()
}

/// The source wrapped in `void main() { … }`, each line indented by 4.
fn in_main(body: &str) -> String {
    let lines: Vec<String> = body.lines().map(|l| format!("    {l}")).collect();
    format!("void main() {{\n{}\n}}\n", lines.join("\n"))
}

/// Exactly one diagnostic.
fn one(src: &str) -> Diag {
    let ds = check_source(src);
    assert_eq!(
        ds.len(),
        1,
        "expected one diagnostic for {src:?}, got {ds:#?}"
    );
    ds.into_iter().next().unwrap()
}

fn hint(d: &Diag) -> &str {
    d.hint.as_deref().unwrap_or("")
}

const NONE: [&str; 0] = [];

#[test]
fn examples_are_clean() {
    for f in ["examples/hello.dif", "examples/fib.dif"] {
        let path = format!("{}/{f}", env!("CARGO_MANIFEST_DIR"));
        let src = std::fs::read_to_string(&path).unwrap();
        assert_eq!(check_source(&src), vec![], "{f}");
    }
    let src = "int add(int a,\n        int b) {\n    return a +\n        b\n}\n\nvoid main() {\n    print(\"sum {add(1, 2)}\")\n}\n";
    assert_eq!(codes(src), NONE);
}

#[test]
fn foreign_syntax() {
    let d = one(&in_main("int i = 0\ni++"));
    assert_eq!((d.code, hint(&d)), ("L7", "use i += 1"));
    let d = one(&in_main("var x = 5\nprint(x)"));
    assert_eq!(
        (d.code, d.pos.line, d.pos.col, hint(&d)),
        ("R2", 2, 5, "int x = 5")
    );
    let d = one("fn add(a: int) {\n}\n");
    assert_eq!(d.code, "R2");
    assert!(hint(&d).contains("return type comes first"));
    let d = one(&in_main("for (int i = 0; i < 3; i++) {\n    print(i);\n}"));
    assert_eq!(
        (d.code, d.pos.col, hint(&d)),
        ("R11", 5, "for int i in range(3)")
    );
    let d = one(&in_main("for (int i = 2; i < n; i += 1) {\n}"));
    assert_eq!(hint(&d), "for int i in range(2, n)");
    assert_eq!(codes("/* x */\nvoid main() {\n}\n"), ["L2"]);
}

#[test]
fn elif_where_it_is_written() {
    // B3: after `}` on the same line, on the next line, and alone
    for body in [
        "int x = 1\nif x > 0 {\n} elif x < 0 {\n}",
        "int x = 1\nif x > 0 {\n}\nelif x < 0 {\n}",
        "elif x {\n}",
    ] {
        let d = one(&in_main(body));
        assert_eq!((d.code, hint(&d)), ("R3", "use else if"), "{body}");
    }
    // the branch after `elif` is still read: its errors are its own
    assert_eq!(
        codes(&in_main(
            "int x = 1\nif x > 0 {\n} elif x < 0 {\n    var y = 2\n}"
        )),
        ["R3", "R2"]
    );
}

#[test]
fn precedence_errors() {
    let d = one(&in_main("bool b = 1 < 2 < 3"));
    assert_eq!((d.code, hint(&d)), ("S2", "write 1 < 2 && 2 < 3"));
    let d = one(&in_main("bool b = 1 < 2 < 3 < 4"));
    assert_eq!(hint(&d), "write 1 < 2 && 2 < 3 && 3 < 4");
    let d = one(&in_main("bool b = true && false || true"));
    assert_eq!(
        (d.code, hint(&d)),
        ("S3", "add parentheses: (true && false) || true")
    );
    let d = one(&in_main("bool x = true\nbool b = x || x || x && x"));
    assert_eq!(hint(&d), "add parentheses: x || x || (x && x)");
    assert_eq!(codes(&in_main("bool b = (true && false) || true")), NONE);
    assert_eq!(codes(&in_main("bool b = 1 & 2 == 2")), NONE);
}

#[test]
fn statement_and_block_rules() {
    let d = one("print(\"Hallo\")\n");
    assert_eq!((d.code, d.pos.line, d.pos.col), ("R1", 1, 1));
    let d = one(&in_main("int n = 3\nif n > 0\n{\n    n -= 1\n}"));
    assert_eq!(
        (d.code, d.pos.line, d.pos.col, hint(&d)),
        ("R3", 3, 13, "if n > 0 {")
    );
    assert_eq!(codes(&in_main("int a = 1 int b = 2")), ["R3"]);
    assert_eq!(codes(&in_main("int a = 1;")), ["L3"]);
    assert_eq!(codes(&in_main("1 + 2")), ["S4"]);
    assert_eq!(codes("int x = 1\n"), ["R4"]);
    assert_eq!(codes(&in_main("int n")), ["R2"]);
    assert_eq!(codes(&in_main("if true {\n}\nelse {\n}")), ["R3"]);
}

#[test]
fn one_cause_one_message() {
    // B2: each of these is one mistake and gets one diagnostic
    let cases: &[(&str, &str, &str)] = &[
        ("for i in range(10) {\n}", "R2", "for int i in range(10)"),
        ("bool ok = true\nprint(ok)", "S1", "choose another name"),
        ("int test = 1\nprint(test)", "S1", "choose another name"),
        (
            "print(\"abc)\nprint(1)",
            "L5",
            "add `\"` at the end of the string",
        ),
        ("print(1\nprint(2)", "S1", "add `)`"),
        (
            "print(\"a {x\")",
            "L6",
            "close it with `}`, or write \\{ for a literal brace",
        ),
        (
            "int größe = 1\nprint(größe)",
            "L1",
            "names use ASCII letters, digits and _",
        ),
        (
            "print(1))",
            "S1",
            "it has no matching opening bracket; remove it",
        ),
        (
            "int a = 1\nint b = 2\na = b = 1",
            "S1",
            "an assignment has no value; assign each variable on its own line",
        ),
        ("int x = 1\nif x = 1 {\n}", "S1", "if x == 1 {"),
        (
            "def helper():\n    return 1",
            "R2",
            "the return type comes first: int add(int a, int b)",
        ),
    ];
    for (body, code, h) in cases {
        let d = one(&in_main(body));
        assert_eq!((d.code, hint(&d)), (*code, *h), "{body}");
    }
    // the missing `)` is reported where it was opened
    let d = one(&in_main("print(1\nprint(2)"));
    assert_eq!((d.pos.line, d.pos.col), (2, 10));
    // headers in other spellings: one R2 with the whole header
    let d = one("int add(a: int, b: int) {\n    return a + b\n}\n");
    assert_eq!((d.code, hint(&d)), ("R2", "int add(int a, int b) {"));
    let d = one("void add(int a, int b) -> int {\n    return a + b\n}\n");
    assert_eq!((d.code, hint(&d)), ("R2", "int add(int a, int b) {"));
    let d = one("void main(void) {\n}\n");
    assert_eq!((d.code, hint(&d)), ("S1", "void main() {"));
    let d = one("void main {\n}\n");
    assert_eq!((d.code, hint(&d)), ("S1", "void main() {"));
    // top level
    let d = one("let x = 5\n");
    assert_eq!((d.code, hint(&d)), ("R2", "const int x = 5"));
    let d = one("int x\n");
    assert_eq!(
        (d.code, hint(&d)),
        ("R2", "give it a value: const int x = 0")
    );
    let d = one("}\nvoid main() {\n}\n");
    assert_eq!((d.code, d.pos.line), ("S1", 1));
    assert_eq!(
        codes("def main():\n    print(\"hi\")\n\nvoid f() {\n}\n"),
        ["R2"]
    );
}

#[test]
fn missing_brace() {
    // at the end of the file: once, at the innermost open block
    let d = one("void main() {\n    if true {\n");
    assert_eq!(
        (d.code, d.pos.line, d.pos.col, hint(&d)),
        ("S1", 2, 13, "add `}`")
    );
    // before the next function: once, and the next function is read normally
    let src = "void main() {\n    if true {\n    }\n\nvoid f() {\n    var y = 1\n}\n";
    let ds = check_source(src);
    assert_eq!(ds.iter().map(|d| d.code).collect::<Vec<_>>(), ["S1", "R2"]);
    assert_eq!((ds[0].pos.line, hint(&ds[0])), (1, "add `}` before line 5"));
}

#[test]
fn literals_and_characters() {
    // B4 and N5: forms outside the grammar are one L8, no follow-up error
    for lit in ["0X10", "0x_ff", "1e1_0", "0b102", "0xfg", "1e", "1e+"] {
        assert_eq!(codes(&in_main(&format!("int x = {lit}"))), ["L8"], "{lit}");
    }
    assert_eq!(codes(&in_main("print(\"\\u{1_F600}\")")), ["L6"]);
    // invisible characters are named, not printed
    let d = one("\u{FEFF}void main() {\n}\n");
    assert_eq!(d.code, "L1");
    assert!(d.msg.contains("U+FEFF"), "{}", d.msg);
    let d = one(&in_main("int x =\u{A0}1"));
    assert!(d.msg.contains("U+00A0"), "{}", d.msg);
}

#[test]
fn hints_fix_the_error() {
    // N3: S4 hints do not suggest new errors
    let d = one(&in_main("int a = 1\na == 1"));
    assert_eq!((d.code, hint(&d)), ("S4", "to assign, write a = 1"));
    let d = one(&in_main("print"));
    assert_eq!(hint(&d), "to call it, write print(…)");
    // N7: L7 also for prefix forms
    let d = one(&in_main("int i = 0\n++i"));
    assert_eq!((d.code, hint(&d)), ("L7", "use i += 1"));
    let d = one(&in_main("int i = --1"));
    assert_eq!(hint(&d), "write -(-1)");
}

#[test]
fn comments_and_strings_cut_cleanly() {
    // a block comment over two lines ends its line; an unclosed one ends with its line
    assert_eq!(codes(&in_main("int a = 1 /* x\n*/ int b = 2")), ["L2"]);
    assert_eq!(codes(&in_main("/* x\nint a = 1")), ["L2"]);
    // `\` at the end of the line is part of the open string, not a second error
    assert_eq!(codes(&in_main("print(\"abc\\\nprint(1)")), ["L5"]);
    // format specifications come later: one message, the expression is still read
    let d = one(&in_main("int x = 1\nprint(\"{x:.2f}\")"));
    assert_eq!((d.code, d.pos.line, d.pos.col), ("S1", 3, 14));
    // an operator at the start of a line says why
    let d = one(&in_main("int a = 1\nint b = a\n    * 2"));
    assert!(
        hint(&d).starts_with("a line that starts with an operator"),
        "{d:?}"
    );
}

#[test]
fn no_crash_on_broken_input() {
    // B1: `for (` at the end of the file (the block of main is not closed either)
    assert_eq!(codes("void main() {\n    for ("), ["S1", "R11"]);
    assert_eq!(codes("void main() {\n    for(/*"), ["S1", "R11", "L2"]);
    assert_eq!(codes(&in_main("for (")), ["R11"]);
    for src in [
        "",
        "\n\n",
        "// only a comment",
        "}",
        "{",
        "(",
        "\"",
        "void",
        "void main(",
        "int x =",
    ] {
        let _ = check_source(src);
    }
}

#[test]
fn deep_nesting_is_an_error_not_a_crash() {
    // N1
    let deep = format!("_ = {}1{}", "(".repeat(10_000), ")".repeat(10_000));
    assert_eq!(codes(&in_main(&deep)), ["S1"]);
    let ifs = "if true {\n".repeat(10_000) + &"}\n".repeat(10_000);
    let ds = check_source(&in_main(&ifs));
    assert_eq!(ds.len(), 1);
    assert!(ds[0].msg.contains("nesting is deeper than 256 levels"));
}

#[test]
fn declaration_cut_off_by_end_of_file_is_one_message() {
    for src in [
        "int x =",
        "void main() {\n    int x =",
        "void main() {\n    int x =\n",
    ] {
        assert_eq!(one(src).code, "S1", "{src:?}");
    }
}
