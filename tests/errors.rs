//! Part 2b: every diagnostic of results, optionals, `try`, `??`, `switch` over them
//! and tests (spec R10, R12, R13, R18, R28), one message per cause.

use dif::check_source;

/// Exactly one diagnostic: (code, line:col, message, hint).
fn one(src: &str) -> (&'static str, String, String, String) {
    let ds = check_source(src);
    assert_eq!(ds.len(), 1, "expected one diagnostic for {src:?}: {ds:#?}");
    let d = &ds[0];
    let at = format!("{}:{}", d.pos.line, d.pos.col);
    (
        d.code,
        at,
        d.msg.clone(),
        d.hint.clone().unwrap_or_default(),
    )
}

/// Checks the diagnostic of `src`: code, position, message, hint (empty = none).
#[track_caller]
fn bad(src: &str, code: &str, at: &str, msg: &str, hint: &str) {
    let got = one(src);
    assert_eq!(
        (got.0, got.1.as_str(), got.2.as_str(), got.3.as_str()),
        (code, at, msg, hint),
        "{src}"
    );
}

const PRE: &str = "enum E { Bad }\nenum F { Worse }\nint!E get() {\n    return 1\n}\n?int opt() {\n    return none\n}\nvoid!E act() {\n}\n\n";

/// `body` inside `void main()`, after some declarations (lines 1-11 are `PRE`).
fn in_main(body: &str) -> String {
    let lines: Vec<String> = body.lines().map(|l| format!("    {l}")).collect();
    format!("{PRE}void main() {{\n{}\n}}\n", lines.join("\n"))
}

#[test]
fn valid_program_with_every_form() {
    let src = include_str!("check/part2b.dif");
    assert_eq!(check_source(src), vec![], "{src}");
}

#[test]
fn types() {
    bad(
        "?void f() {\n}\n",
        "R12",
        "1:1",
        "?void has no value that could be missing",
        "use void!E",
    );
    bad(
        "??int f() {\n    return 1\n}\n",
        "S1",
        "1:1",
        "this type is nested too far",
        "a type has at most one ? and one !E",
    );
    bad(
        "enum E { Bad }\nint!E!E f() {\n    return 1\n}\n",
        "S1",
        "2:1",
        "this type is nested too far",
        "a type has at most one ? and one !E",
    );
    bad(
        "void main() {\n    void!int x = 1\n}\n",
        "R8",
        "2:5",
        "`void` is only a return type",
        "",
    );
    bad(
        "int!Nope f() {\n    return 1\n}\n",
        "R4",
        "1:1",
        "unknown type `Nope`",
        "",
    );
}

#[test]
fn places() {
    bad(
        &in_main("int n = none"),
        "R8",
        "13:13",
        "expected int, found none",
        "declare it as ?int n",
    );
    bad(
        &in_main("int n = err(E.Bad)"),
        "R8",
        "13:13",
        "expected int, found err(…)",
        "declare it as int!E n",
    );
    bad(
        &in_main("int n = some(3)"),
        "R8",
        "13:13",
        "expected int, found ?int",
        "declare it as ?int n",
    );
    bad(
        &in_main("print(none)"),
        "R12",
        "13:11",
        "`none` needs a place of type ?T",
        "",
    );
    bad(
        &in_main("print(err(E.Bad))"),
        "R12",
        "13:11",
        "`err(…)` needs a place of type T!E",
        "",
    );
    bad(
        &in_main("bool b = none == none"),
        "R12",
        "13:14",
        "`none` needs a place of type ?T",
        "",
    );
    bad(
        &in_main("?int x = some(opt())"),
        "R12",
        "13:14",
        "`some` needs a plain value, found ?int",
        "use opt() directly",
    );
    bad(
        &in_main("int!E r = err(F.Worse)"),
        "R8",
        "13:19",
        "expected E, found F",
        "",
    );
    bad(
        &in_main("int!E r = ok(1)"),
        "R12",
        "13:15",
        "`ok` is only a pattern in switch",
        "return the value directly: return 1",
    );
    bad(
        &in_main("?str s = 3"),
        "R8",
        "13:14",
        "expected ?str, found int",
        "",
    );
    // a plain value fits where ?T and T!E are expected
    assert_eq!(
        check_source(&in_main(
            "?int a = 3\nint!E b = 4\n?int c = some(5)\nprint(a)\nprint(b)\nprint(c)"
        )),
        vec![]
    );
}

#[test]
fn equality_r28() {
    bad(
        &in_main("?int x = 4\nprint(x == 3)"),
        "R8",
        "14:13",
        "cannot apply `==` to ?int and int",
        "x == some(3)",
    );
    bad(
        &in_main("?int x = 4\nprint(3 != x)"),
        "R8",
        "14:13",
        "cannot apply `!=` to int and ?int",
        "some(3) != x",
    );
    bad(
        &in_main("int!E r = 4\nprint(r == 3)"),
        "R8",
        "14:13",
        "cannot apply `==` to int!E and int",
        "use switch to look at the value",
    );
    bad(
        &in_main("?int x = 4\nprint(x < x)"),
        "R8",
        "14:13",
        "cannot apply `<` to ?int and ?int",
        "",
    );
    bad(
        &in_main("?int x = 4\n?str y = \"a\"\nprint(x == y)"),
        "R8",
        "15:13",
        "cannot apply `==` to ?int and ?str",
        "",
    );
    bad(
        &in_main("int n = 3\nprint(n == none)"),
        "R8",
        "14:16",
        "expected int, found none",
        "",
    );
    // `none` and `err(e)` take the type of the other operand
    assert_eq!(
        check_source(&in_main(
            "?int x = 4\nint!E r = 4\nprint(x == none)\nprint(none != x)\nprint(r == err(E.Bad))\nprint(x == some(4))"
        )),
        vec![]
    );
}

#[test]
fn try_r12() {
    let get = "int!E get() {\n    return 1\n}\n";
    bad(
        &format!("enum E {{ Bad }}\n{get}int f() {{\n    return try get()\n}}\n"),
        "R12",
        "6:12",
        "`try` needs a function that returns T!E or ?T",
        "change the return type to int!E, or use ?? or switch",
    );
    bad(
        &format!("{PRE}void f() {{\n    try opt()\n}}\n"),
        "R12",
        "13:5",
        "`try` needs a function that returns T!E or ?T",
        "use ?? or switch",
    );
    bad(
        &format!("{PRE}const int X = try get()\n"),
        "R12",
        "12:15",
        "`try` needs a function that returns T!E or ?T",
        "",
    );
    bad(
        &format!("{PRE}int!F f() {{\n    return try get()\n}}\n"),
        "R12",
        "13:12",
        "`try` gives E, but the function returns int!F",
        "translate it with try … or F.V(…)",
    );
    bad(
        &format!("{PRE}int!F f() {{\n    return try opt()\n}}\n"),
        "R12",
        "13:12",
        "`try` gives none, but the function returns int!F",
        "translate it with try … or F.V(…)",
    );
    bad(
        &format!("{PRE}?int f() {{\n    return try get()\n}}\n"),
        "R12",
        "13:12",
        "`try` gives E, but the function returns ?int",
        "handle the error with ?? or switch",
    );
    bad(
        &format!("{PRE}int f(int n) {{\n    return try n\n}}\n"),
        "R12",
        "13:12",
        "`try` needs a result of type T!E or ?T, found int",
        "remove try",
    );
    bad(
        &format!("{PRE}?int f() {{\n    return try opt() or none\n}}\n"),
        "R12",
        "13:22",
        "`or` needs a function that returns U!F",
        "remove the or part, or return int!F",
    );
    bad(
        &format!("{PRE}int!F f() {{\n    return try get() or E.Bad\n}}\n"),
        "R8",
        "13:25",
        "expected F, found E",
        "",
    );
    // R12 `or err(E e): …` and the short form `or F.V`
    let g = "enum G {\n    Wrap(E e)\n    Pair(E e, int n)\n    Text(str s)\n}\n";
    bad(
        &format!("{PRE}{g}int!G f() {{\n    return try get() or err(F x): G.Wrap(E.Bad)\n}}\n"),
        "R8",
        "18:29",
        "`err` holds an E",
        "or err(E x): …",
    );
    bad(
        &format!("{PRE}{g}int!G f() {{\n    return try opt() or err(E x): G.Wrap(x)\n}}\n"),
        "R12",
        "18:29",
        "`try` on ?int has no error to bind",
        "remove err(…):",
    );
    bad(
        &format!("{PRE}{g}int!G f() {{\n    return try get() or G.Text\n}}\n"),
        "R8",
        "18:25",
        "`G.Text` holds str, but the cause is E",
        "write it out: or err(E e): G.Text(…)",
    );
    bad(
        &format!("{PRE}{g}int!G f() {{\n    return try get() or G.Pair\n}}\n"),
        "R8",
        "18:25",
        "`G.Pair` has 2 values, but the cause is one",
        "write it out: or err(E e): G.Pair(…)",
    );
    bad(
        &format!("{PRE}{g}int!G f() {{\n    int n = try get() or err(E x): G.Wrap(x)\n    return n + x\n}}\n"),
        "R4",
        "19:16",
        "unknown name `x`",
        "did you mean f?",
    );
    bad(
        "void main() {\n    int x = 1 or 2\n}\n",
        "S1",
        "2:15",
        "unexpected `or`",
        "or belongs to try: try f() or E.V(…)",
    );
    // `try` of a call as a statement is the call (R3); its result still counts (R13)
    bad(
        &format!("{PRE}int!E f() {{\n    try get()\n    return 1\n}}\n"),
        "R13",
        "13:5",
        "result of `try get()` is never used",
        "use it, or discard it explicitly with `_ = try get()`",
    );
    let ok = format!(
        "{PRE}int!F f() {{\n    int a = try get() or F.Worse\n    int b = try opt() or F.Worse\n    return a + b\n}}\n"
    );
    assert_eq!(check_source(&ok), vec![]);
}

#[test]
fn coalesce_r12() {
    bad(
        &in_main("int n = 3\nprint(n ?? 0)"),
        "R12",
        "14:13",
        "`??` needs a ?T or T!E on the left, found int",
        "remove the ?? and its default",
    );
    bad(
        &in_main("print(opt() ?? \"no\")"),
        "R8",
        "13:20",
        "expected int, found str",
        "",
    );
    let ok = in_main("print(get() ?? 1)\nprint(opt() ?? get() ?? 2)");
    assert_eq!(check_source(&ok), vec![]);
}

#[test]
fn use_r13() {
    bad(
        &in_main("act()"),
        "R13",
        "13:5",
        "result of `act()` is never used",
        "use it with try, ?? or switch, or discard it explicitly with `_ = act()`",
    );
    bad(
        &in_main("opt()"),
        "R13",
        "13:5",
        "result of `opt()` is never used",
        "use it with try, ?? or switch, or discard it explicitly with `_ = opt()`",
    );
    // even a function with an effect must have its result used
    let src = "enum E { Bad }\nint!E loud() {\n    print(1)\n    return 1\n}\nvoid main() {\n    loud()\n}\n";
    bad(
        src,
        "R13",
        "7:5",
        "result of `loud()` is never used",
        "use it with try, ?? or switch, or discard it explicitly with `_ = loud()`",
    );
    assert_eq!(
        check_source(&in_main(
            "_ = act()\n_ = opt()\nint a = get() ?? 0\nprint(a)"
        )),
        vec![]
    );
}

#[test]
fn switch_r10() {
    bad(
        &in_main("switch opt() {\ncase some(int h):\n    print(h)\n}"),
        "R10",
        "13:5",
        "switch does not handle none",
        "add case none: …, or default:",
    );
    bad(
        &in_main("switch get() {\ncase ok(int h):\n    print(h)\n}"),
        "R10",
        "13:5",
        "switch does not handle err",
        "add case err: …, or default:",
    );
    bad(
        &in_main("switch get() {\ncase 1:\n    print(1)\ndefault:\n    print(2)\n}"),
        "R8",
        "14:10",
        "case 1 is int, the switch is over int!E",
        "",
    );
    bad(
        &in_main("switch opt() {\ncase some:\n    print(1)\ncase none:\n    print(2)\ndefault:\n    print(3)\n}"),
        "R10",
        "18:5",
        "default is never reached: every variant is handled",
        "remove default",
    );
    bad(
        &in_main("switch opt() {\ncase none:\n    print(1)\ncase none:\n    print(2)\ndefault:\n    print(3)\n}"),
        "R10",
        "16:10",
        "case none is already handled at main.dif:14:10",
        "",
    );
    bad(
        &in_main("switch get() {\ncase some:\n    print(1)\ndefault:\n    print(2)\n}"),
        "R8",
        "14:10",
        "`some` needs a switch over ?T, found int!E",
        "",
    );
    bad(
        &in_main("int n = 1\nswitch n {\ncase ok:\n    print(1)\ndefault:\n    print(2)\n}"),
        "R8",
        "15:10",
        "`ok` needs a switch over T!E, found int",
        "",
    );
    bad(
        &in_main("switch get() {\ncase ok(str s):\n    print(s)\ncase err:\n    print(0)\n}"),
        "R8",
        "14:13",
        "`ok` holds an int",
        "case ok(int s):",
    );
    bad(
        &in_main("switch get() {\ncase ok:\n    print(1)\ncase err(int e):\n    print(0)\n}"),
        "R8",
        "16:14",
        "`err` holds an E",
        "case err(E e):",
    );
    bad(
        &in_main("switch act() {\ncase ok(int n):\n    print(n)\ncase err:\n    print(0)\n}"),
        "R10",
        "14:10",
        "`ok` has no value in void!E",
        "case ok:",
    );
    bad(
        &in_main("switch opt() {\ncase some:\n    print(1)\ncase none(int n):\n    print(n)\n}"),
        "R10",
        "16:10",
        "`none` has no value",
        "case none:",
    );
    bad(
        &in_main("switch opt() {\ncase some(int a), none:\n    print(1)\n}"),
        "R10",
        "14:10",
        "a case with several values cannot bind values",
        "",
    );
    // a binding is used in its block; `_` skips; no parentheses matches anything
    let ok = in_main(
        "switch get() {\ncase ok(int n):\n    print(n)\ncase err(_):\n    print(0)\n}\nswitch act() {\ncase ok:\n    print(1)\ncase err(E e):\n    print(e)\n}\nswitch opt() {\ncase some:\n    print(1)\ndefault:\n    print(2)\n}",
    );
    assert_eq!(check_source(&ok), vec![]);
}

#[test]
fn return_paths() {
    // a function returning T!E needs a value on every path; void!E does not
    bad(
        "enum E { Bad }\nint!E f(int n) {\n    if n > 0 {\n        return n\n    }\n}\n",
        "R8",
        "2:7",
        "`f` does not return a value on every path",
        "end it with return, e.g. return 0",
    );
    assert_eq!(
        check_source(
            "enum E { Bad }\nvoid!E f(int n) {\n    if n > 0 {\n        return err(E.Bad)\n    }\n}\n"
        ),
        vec![]
    );
}

#[test]
fn tests_r18() {
    bad(
        "void check(int n) {\n    expect(n > 0)\n}\n",
        "R18",
        "2:5",
        "`expect` is only for tests",
        "use assert(n > 0) in functions",
    );
    bad(
        "test \"sum\" {\n}\n\ntest \"sum\" {\n}\n",
        "R18",
        "4:6",
        "test \"sum\" is declared twice",
        "first declared at main.dif:1:6",
    );
    bad(
        "void f() {\n    test \"inner\" {\n    }\n}\n",
        "R18",
        "2:5",
        "tests are declared at the top level",
        "move it out of the function",
    );
    bad("test \"\" {\n}\n", "R18", "1:6", "a test needs a name", "");
    bad(
        "test \"a {1} b\" {\n}\n",
        "R18",
        "1:6",
        "a test name is plain text",
        "remove the { }",
    );
    bad(
        "test \"x\" panics \"\" {\n}\n",
        "R18",
        "1:17",
        "panics needs a message text",
        "give the part of the panic message to look for",
    );
    bad(
        "test \"x\" {\n    expect(1)\n}\n",
        "R9",
        "2:12",
        "condition is int, not bool",
        "compare explicitly: expect(1 != 0)",
    );
    bad(
        "test \"x\" {\n    return 1\n}\n",
        "R8",
        "2:12",
        "a test returns nothing; remove the value",
        "",
    );
    bad(
        "test \"x\" {\n    expect(1, 2)\n}\n",
        "R8",
        "2:5",
        "`expect` takes 1 argument, found 2",
        "",
    );
    // a test sees every declaration; `try` needs no error type there; `return` ends it
    let ok = "enum E { Bad }\nint!E get() {\n    return 1\n}\n\ntest \"a\" {\n    int n = try get()\n    if n == 1 {\n        return\n    }\n    expect(n == 2)\n}\n\ntest \"b\" panics \"overflow\" {\n    print(1)\n}\n";
    assert_eq!(check_source(ok), vec![]);
    // inside a test a lost change is reported like in a function (R6)
    bad(
        "test \"x\" {\n    int n = 1\n    n = 2\n}\n",
        "R6",
        "3:5",
        "`n` is changed but never read afterwards",
        "use the value, or remove the change",
    );
}

#[test]
fn main_forms_r1() {
    let m = |src: &str| {
        let (toks, _) = dif::lexer::lex(src);
        let (module, _) = dif::parser::parse(src, &toks, &[]);
        dif::checker::main_diag(&module).map(|d| d.msg)
    };
    assert_eq!(m("enum E { A }\nvoid!E main() {\n}\n"), None);
    assert_eq!(
        m("int!E main() {\n    return 1\n}\n").as_deref(),
        Some("`main` must return void or void!E")
    );
    assert_eq!(
        m("?int main() {\n    return 1\n}\n").as_deref(),
        Some("`main` must return void or void!E")
    );
}

#[test]
fn lost_changes_with_try_r6() {
    // `try` may end the function: a changed `mut` parameter is read there
    let ok = "enum E { Bad }\nint!E get() {\n    return 1\n}\n\nvoid!E fill(mut int n) {\n    n = 5\n    int a = try get()\n    n = a\n}\n\nvoid main() {\n    int k = 0\n    _ = fill(mut k)\n    print(k)\n}\n";
    assert_eq!(check_source(ok), vec![]);
    // the value after a `try` still overwrites: a change before it is lost only when
    // no path reads it
    let lost = "enum E { Bad }\nint!E get() {\n    return 1\n}\n\nint!E f() {\n    int n = 1\n    n = 2\n    n = try get()\n    return n\n}\n";
    bad(
        lost,
        "R6",
        "8:5",
        "`n` is changed but never read afterwards",
        "use the value, or remove the change",
    );
}

#[test]
fn lost_changes_per_field_r6() {
    let pre = "struct In {\n    int z\n}\n\nstruct P {\n    int x\n    int y\n    In inner\n    []int xs\n}\n\n";
    let ok = |body: &str| {
        let src = format!("{pre}void f(P p, int i) {{\n{body}}}\n");
        assert_eq!(check_source(&src), vec![], "{body}");
    };
    let lost = |body: &str, msg: &str| {
        let src = format!("{pre}void f(P p, int i) {{\n    P q = p\n{body}    print(q.y)\n}}\n");
        let d = check_source(&src);
        assert!(d.iter().any(|m| m.msg.contains(msg)), "{body}: {d:?}");
    };
    // field after field: reading `p.y` does not read `p.x`
    lost(
        "    q.x = 1\n",
        "`q.x` is changed but never read afterwards",
    );
    lost(
        "    q.inner.z = 1\n    print(q.inner.z)\n    q.inner.z = 2\n",
        "`q.inner.z`",
    );
    lost("    q.x = 1\n    q.x = 2\n    print(q.x)\n", "`q.x`");
    // the part, a part of it, or the whole containing it reads the change
    ok("    P q = p\n    q.x = 1\n    print(q.x)\n");
    ok("    P q = p\n    q.inner.z = 1\n    print(q.inner)\n");
    ok("    P q = p\n    q.inner.z = 1\n    print(q)\n");
    ok("    P q = p\n    q.x = 1\n    P r = q\n    print(r)\n");
    ok("    P q = p\n    q.xs[i] = 1\n    print(q.xs)\n");
    ok("    P q = p\n    q.xs[i] = 1\n    print(q.xs[0])\n");
    // over a loop
    ok("    P q = p\n    for int j in range(3) {\n        print(q.x)\n        q.x += 1\n    }\n");
    lost(
        "    for int j in range(3) {\n        q.x = j\n        print(q.y)\n    }\n",
        "`q.x`",
    );
    // over try
    let t =
        "enum E { Bad }\nint!E get() {\n    return 1\n}\n\nstruct P {\n    int x\n    int y\n}\n\n";
    let src =
        format!("{t}void!E f(mut P p) {{\n    p.x = 1\n    int a = try get()\n    p.y = a\n}}\n");
    assert_eq!(check_source(&src), vec![]);
}
