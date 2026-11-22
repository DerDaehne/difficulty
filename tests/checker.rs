//! Names and types (spec R1, R2, R4, R8, R9, R11, R13, R14, R28, R29).

use dif::check_source;

/// The source wrapped in `void main() { … }`, each line indented by 4.
fn in_main(body: &str) -> String {
    let lines: Vec<String> = body.lines().map(|l| format!("    {l}")).collect();
    format!("void main() {{\n{}\n}}\n", lines.join("\n"))
}

/// Exactly one diagnostic: (code, message, hint).
fn one(src: &str) -> (&'static str, String, String) {
    let ds = check_source(src);
    assert_eq!(
        ds.len(),
        1,
        "expected one diagnostic for {src:?}, got {ds:#?}"
    );
    let d = &ds[0];
    (d.code, d.msg.clone(), d.hint.clone().unwrap_or_default())
}

fn clean(src: &str) {
    assert_eq!(check_source(src), vec![], "{src}");
}

#[test]
fn valid_programs() {
    clean("const int LIMIT = 2 * HALF\nconst int HALF = 50\n\nint twice(int n) {\n    return n * 2\n}\n\nvoid main() {\n    int total = 0\n    for int i in range(1, LIMIT) {\n        if i % 2 == 0 && i != 4 {\n            continue\n        } else if i > 90 {\n            break\n        }\n        total += twice(i)\n    }\n    str s = \"total {total}\" + \"!\"\n    bool fine = s != \"\" || !(total > 0)\n    _ = twice(1)\n    print(fine)\n}\n");
    clean("int sign(int n) {\n    if n < 0 {\n        return -1\n    } else if n == 0 {\n        return 0\n    } else {\n        return 1\n    }\n}\n\nvoid main() {\n    print(sign(-3))\n}\n");
    clean("int forever() {\n    while true {\n        print(1)\n    }\n}\n");
    // sibling blocks may reuse a name; parameters are local copies
    clean(&in_main(
        "if true {\n    int x = 1\n    print(x)\n}\nif true {\n    int x = 2\n    print(x)\n}",
    ));
    clean("void f(int n) {\n    n += 1\n    print(n)\n}\n");
    // constants are exact; only the result must fit (R8, R29)
    clean(&in_main("int a = -9223372036854775808\nint b = 18446744073709551615 - 9223372036854775808\nint c = 1 << 63\nint d = -7 / 2 + -7 % 3\nprint(a + b + c + d)"));
}

#[test]
fn names_r4() {
    let (code, msg, hint) = one(&in_main(
        "int n = 1\nif n > 0 {\n    int n = 2\n    print(n)\n}",
    ));
    assert_eq!(
        (code, msg.as_str()),
        ("R4", "`n` is already declared at main.dif:2:9")
    );
    assert_eq!(
        hint,
        "names are not redeclared inside their block; choose another name"
    );
    let (code, msg, hint) = one(&in_main("int total = 0\nprint(totl)"));
    assert_eq!(
        (code, msg.as_str(), hint.as_str()),
        ("R4", "unknown name `totl`", "did you mean total?")
    );
    // an unknown name is reported once
    assert_eq!(one(&in_main("print(x)\nprint(x)")).0, "R4");
    assert_eq!(
        one("const int A = 1\n\nvoid main() {\n    A = 2\n}\n").0,
        "R4"
    );
    assert_eq!(one(&in_main("const int k = 1\nk += 1")).0, "R4");
    assert_eq!(
        one(&in_main("int print = 1")).1,
        "`print` is a predefined name"
    );
    assert_eq!(one("void f() {\n}\n\nvoid f() {\n}\n").0, "R4");
    assert_eq!(one("void f(int f) {\n}\n").0, "R4");
    assert_eq!(
        one(&in_main("for int i in range(3) {\n    int i = 0\n}")).0,
        "R4"
    );
    assert_eq!(
        one("const int A = B\nconst int B = A\n").1,
        "`A` depends on itself"
    );
    assert_eq!(
        one("int f() {\n    return 1\n}\nconst int A = f()\n").1,
        "a global const needs a value known at compile time"
    );
    assert_eq!(one(&in_main("Int x = 1")).2, "did you mean int?");
}

#[test]
fn declarations_r2() {
    let (code, msg, hint) = one(&in_main("count = 1"));
    assert_eq!(
        (code, msg.as_str(), hint.as_str()),
        (
            "R2",
            "`count` is not declared",
            "declare it with a type: int count = 1"
        )
    );
}

#[test]
fn types_r8() {
    let (code, msg, hint) = one(&in_main("print(\"41\" + 1)"));
    assert_eq!(
        (code, msg.as_str()),
        ("R8", "cannot apply `+` to str and int")
    );
    assert_eq!(hint, "build text with interpolation: \"{a}{b}\"");
    for body in [
        "bool b = 1",
        "int x = \"a\"",
        "int x = 9223372036854775808",
        "int x = 1\nx += \"a\"",
        "str s = \"a\" - \"b\"",
        "bool b = true < false",
        "int x = 1\nx()",
        "print(1, 2)",
        "float x = 1",
    ] {
        let (code, ..) = one(&in_main(body));
        assert!(code == "R8" || code == "R4", "{body}: {code}");
    }
    assert_eq!(
        one("int f(int a) {\n    return a\n}\n\nvoid main() {\n    print(f(true))\n}\n").1,
        "expected int, found bool"
    );
    assert_eq!(
        one("int f(int a) {\n    return a\n}\n\nvoid main() {\n    print(f())\n}\n").1,
        "`f` takes 1 argument, found 0"
    );
    assert_eq!(
        one("int f(int n) {\n    if n > 0 {\n        return 1\n    }\n}\n").1,
        "`f` does not return a value on every path"
    );
    assert_eq!(one("int f() {\n    return\n}\n").0, "R8");
    assert_eq!(one("void f() {\n    return 1\n}\n").0, "R8");
    assert_eq!(one(&in_main("for int i, int x in range(3) {\n}")).0, "R8");
    assert_eq!(one(&in_main("for str s in range(3) {\n}")).0, "R8");
}

#[test]
fn conditions_r9() {
    let (code, msg, hint) = one(&in_main("int n = 3\nwhile n {\n    n -= 1\n}"));
    assert_eq!(
        (code, msg.as_str(), hint.as_str()),
        (
            "R9",
            "condition is int, not bool",
            "compare explicitly: while n != 0"
        )
    );
    assert_eq!(
        one(&in_main("str s = \"\"\nif s {\n}")).2,
        "compare explicitly: if s != \"\""
    );
    assert_eq!(
        one(&in_main("int n = 1\nbool b = n && true")).1,
        "operand of `&&` is int, not bool"
    );
    assert_eq!(one(&in_main("bool b = !1")).0, "R9");
}

#[test]
fn loops_r11_and_results_r13() {
    assert_eq!(one(&in_main("break")).1, "`break` outside a loop");
    assert_eq!(one(&in_main("int r = range(3)")).0, "R11");
    let (code, msg, hint) =
        one("int square(int n) {\n    return n * n\n}\n\nvoid main() {\n    square(3)\n}\n");
    assert_eq!(
        (code, msg.as_str()),
        ("R13", "result of `square(3)` is never used")
    );
    assert_eq!(
        hint,
        "use it, or discard it explicitly with `_ = square(3)`"
    );
    assert_eq!(
        one("void f() {\n}\n\nvoid main() {\n    _ = f()\n}\n").0,
        "R8"
    );
    assert_eq!(
        one("void f() {\n}\n\nvoid main() {\n    print(f())\n}\n").0,
        "R28"
    );
}

#[test]
fn certain_aborts_r14() {
    assert_eq!(one(&in_main("print(1 / 0)")).1, "division by zero");
    assert_eq!(
        one(&in_main("const int Z = 0\nprint(5 % Z)")).1,
        "division by zero"
    );
    assert_eq!(
        one(&in_main("print(1 << 64)")).1,
        "shift by 64 out of range 0..63"
    );
    // with a variable the abort is not certain at compile time: a runtime check (R14)
    clean(&in_main("int z = 0\nprint(1 / z)"));
}

#[test]
fn main_r1() {
    let m = |src: &str| {
        let (toks, _) = dif::lexer::lex(src);
        let (module, _) = dif::parser::parse(src, &toks, &[]);
        dif::checker::main_diag(&module).map(|d| d.msg)
    };
    assert_eq!(m("void f() {\n}\n").as_deref(), Some("no `main` function"));
    assert_eq!(
        m("int main() {\n    return 0\n}\n").as_deref(),
        Some("`main` must be `void main()`")
    );
    assert_eq!(m("void main() {\n}\n"), None);
}

#[test]
fn ten_thousand_lines_are_fast() {
    let mut src = String::new();
    for k in 0..1000 {
        src += &format!("int g{k}(int n) {{\n    int a = n * 2\n    if a > 10 {{\n        a -= 1\n    }}\n    for int i in range(a) {{\n        a += i\n    }}\n    return a\n}}\n\n");
    }
    src += "void main() {\n    print(g0(1))\n}\n";
    assert!(src.lines().count() >= 10_000);
    let t = std::time::Instant::now();
    clean(&src);
    // generous bound for debug builds; the release build is much faster
    assert!(t.elapsed().as_millis() < 1000, "{:?}", t.elapsed());
}

#[test]
fn long_chains_are_bounded() {
    // a long chain is a deep tree: one diagnostic instead of a stack overflow
    let sum = vec!["1"; 50_000].join(" + ");
    let ds = check_source(&in_main(&format!("print({sum})")));
    assert_eq!(ds.len(), 1, "{ds:?}");
    assert!(ds[0].msg.contains("more than 1024 operators"));
    clean(&in_main(&format!("print({})", vec!["1"; 1000].join(" + "))));
    let mut consts: String = (0..20_000)
        .map(|k| format!("const int A{k} = A{} + 1\n", k + 1))
        .collect();
    consts += "const int A20000 = 0\n";
    let ds = check_source(&consts);
    assert_eq!(ds.len(), 1, "{:?}", &ds[..ds.len().min(3)]);
    assert_eq!(ds[0].code, "R4");
    // a call with the wrong number of arguments is one mistake, not also R13
    assert_eq!(
        one("int f(int a) {\n    return a\n}\n\nvoid main() {\n    f(1, 2)\n}\n").0,
        "R8"
    );
}
