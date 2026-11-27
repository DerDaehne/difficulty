//! `dif test` (R18): selection, one process per test, exit codes, `--json`, and the
//! error exit of `void!E main` (R12). The spec examples are in tests/golden.rs.

use std::path::PathBuf;
use std::process::Command;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("dif-cli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Dir(d)
    }

    fn file(&self, path: &str, text: &str) {
        let p = self.0.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    /// Runs `dif <args>` in the directory: (stdout, stderr, exit code).
    fn dif(&self, args: &[&str]) -> (String, String, i32) {
        let r = Command::new(env!("CARGO_BIN_EXE_dif"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap();
        let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        (
            text(&r.stdout),
            text(&r.stderr),
            r.status.code().unwrap_or(-1),
        )
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const ADD: &str = "int add(int a, int b) {
    return a + b
}

test \"addition\" {
    expect(add(2, 3) == 5)
}

test \"subtraction\" {
    expect(add(5, -3) == 2)
}
";

#[test]
fn passing_tests_exit_zero() {
    let d = Dir::new("pass");
    d.file("add.dif", ADD);
    let (out, err, code) = d.dif(&["test", "add.dif"]);
    let want = "ok   add.dif:5 test \"addition\"\nok   add.dif:9 test \"subtraction\"\ntest result: 2 passed, 0 failed\n";
    assert_eq!((out.as_str(), err.as_str(), code), (want, "", 0));
}

#[test]
fn filter_selects_by_name_and_needs_a_path() {
    let d = Dir::new("filter");
    d.file("add.dif", ADD);
    let (out, _, code) = d.dif(&["test", ".", "sub"]);
    let want = "ok   add.dif:9 test \"subtraction\"\ntest result: 1 passed, 0 failed\n";
    assert_eq!((out.as_str(), code), (want, 0));
    // case counts
    let (out, err, code) = d.dif(&["test", ".", "Sub"]);
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("", "error: no test matches \"Sub\"\n", 1)
    );
}

#[test]
fn no_tests_is_an_error() {
    let d = Dir::new("none");
    d.file("a.dif", "void main() {\n    print(1)\n}\n");
    let (out, err, code) = d.dif(&["test"]);
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("", "error: no tests found\n", 1)
    );
}

#[test]
fn files_that_do_not_compile_stop_the_run() {
    let d = Dir::new("broken");
    d.file("a_good.dif", ADD);
    d.file("b_bad.dif", "test \"x\" {\n    expect(y == 1)\n}\n");
    let (out, err, code) = d.dif(&["test", "."]);
    assert_eq!((out.as_str(), code), ("", 2));
    assert!(err.starts_with("b_bad.dif:2:12: error[R4]"), "{err}");
    // a wrong call is exit 2 as well
    assert_eq!(d.dif(&["test", "--bogus"]).2, 2);
    assert_eq!(d.dif(&["test", "a", "b", "c"]).2, 2);
    assert_eq!(d.dif(&["test", "missing.dif"]).2, 2);
}

#[test]
fn directories_are_searched_in_path_order() {
    let d = Dir::new("tree");
    d.file("z.dif", "test \"z\" {\n    expect(true)\n}\n");
    d.file("sub/m.dif", "test \"m\" {\n    expect(true)\n}\n");
    d.file("a.dif", "test \"a\" {\n    expect(true)\n}\n");
    d.file("notes.txt", "ignored");
    let (out, _, code) = d.dif(&["test"]);
    let want = "ok   a.dif:1 test \"a\"\nok   sub/m.dif:1 test \"m\"\nok   z.dif:1 test \"z\"\ntest result: 3 passed, 0 failed\n";
    assert_eq!((out.as_str(), code), (want, 0));
}

#[test]
fn a_crash_or_timeout_hits_only_its_test() {
    let d = Dir::new("crash");
    d.file(
        "c.dif",
        "int down(int n) {
    return down(n + 1) + 1
}

int spin() {
    while true {
    }
}

test \"fine\" {
    expect(1 == 1)
}

test \"stack\" {
    print(\"before\")
    print(down(0))
}

test \"spins\" {
    print(spin())
}

test \"overflow\" panics \"integer overflow\" {
    int big = 9223372036854775807
    print(big + 1)
}
",
    );
    let (out, _, code) = d.dif(&["test", "c.dif"]);
    let want = "ok   c.dif:10 test \"fine\"
FAIL c.dif:14 test \"stack\"
    stack overflow
    output:
        before
FAIL c.dif:19 test \"spins\"
    timed out after 10 s
ok   c.dif:23 test \"overflow\"
test result: 2 passed, 2 failed
";
    assert_eq!((out.as_str(), code), (want, 1));
    let (json, _, _) = d.dif(&["test", "c.dif", "stack", "--json"]);
    assert!(
        json.contains("\"failure\":{\"kind\":\"crash\",\"line\":null,\"column\":null,\"offset\":null,\"message\":\"stack overflow\",\"values\":[],\"output\":\"before\\n\"}"),
        "{json}"
    );
}

#[test]
fn failures_show_values_text_and_position() {
    let d = Dir::new("values");
    d.file(
        "v.dif",
        "struct P {
    int x
    str s
}

?int none_of() {
    return none
}

test \"shown\" {
    P p = P{x: 1, s: \"a\\nb\"}
    expect(p.x == 1 && p.s == \"a\"
        && none_of() != none)
}

test \"try none\" {
    int v = try none_of()
    expect(v == 1)
}

test \"expect inside a loop\" {
    for int i in range(3) {
        expect(i < 2)
    }
}

test \"wrong panic\" panics \"zzz\" {
    int z = 0
    print(1 / z)
}

test \"no panic\" panics \"zzz\" {
    expect(true)
}
",
    );
    let (out, _, code) = d.dif(&["test", "v.dif"]);
    let want = "FAIL v.dif:10 test \"shown\"
    v.dif:12:5: expect failed: p.x == 1 && p.s == \"a\" && none_of() != none
        p.x = 1
        p.s = \"a\\nb\"
FAIL v.dif:16 test \"try none\"
    v.dif:17:13: try failed: none
FAIL v.dif:21 test \"expect inside a loop\"
    v.dif:23:9: expect failed: i < 2
        i = 2
FAIL v.dif:27 test \"wrong panic\"
    expected a panic containing \"zzz\", got \"division by zero\"
FAIL v.dif:32 test \"no panic\"
    expected a panic containing \"zzz\", but the test finished
test result: 0 passed, 5 failed
";
    assert_eq!((out.as_str(), code), (want, 1));
}

#[test]
fn run_and_build_leave_tests_out_and_main_errors_exit_one() {
    let d = Dir::new("run");
    d.file(
        "m.dif",
        "enum E {
    Bad(str why)
}

void!E fail() {
    return err(E.Bad(\"no \\\"way\\\"\"))
}

void!E main() {
    print(\"before\")
    try fail()
    print(\"after\")
}

test \"never runs\" {
    expect(false)
}
",
    );
    let (out, err, code) = d.dif(&["run", "m.dif"]);
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("before\n", "error: E.Bad(\"no \\\"way\\\"\")\n", 1)
    );
    d.file(
        "s.dif",
        "void!str main() {\n    return err(\"plain text\")\n}\n",
    );
    let (out, err, code) = d.dif(&["run", "s.dif"]);
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        ("", "error: plain text\n", 1)
    );
    // a test file without main still checks
    assert_eq!(d.dif(&["check", "m.dif"]).2, 0);
}

#[test]
fn ledger_example_runs_and_tests() {
    let d = Dir::new("ledger");
    let ledger = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/ledger.dif");
    let (out, err, code) = d.dif(&["run", ledger]);
    let want = "100\nerr(TxErr.Frozen)\nerr(TxErr.Empty)\nAccount{owner: \"nobody\", balance: 0, frozen: false}\n";
    assert_eq!(
        (out.as_str(), err.as_str(), code),
        (want, "error: TxErr.Insufficient(380)\n", 1)
    );
    let (out, _, code) = d.dif(&["test", ledger]);
    assert!(out.ends_with("test result: 6 passed, 0 failed\n"), "{out}");
    assert_eq!(code, 0);
}

#[test]
fn names_are_escaped_blank_output_lines_are_bare_and_build_cleans_up() {
    let d = Dir::new("polish");
    d.file(
        "p.dif",
        "test \"a\\nb \\\"q\\\"\" {
    print(\"x\\n\\ny\")
    expect(false)
}
",
    );
    let (out, _, _) = d.dif(&["test", "p.dif"]);
    assert!(out.contains("test \"a\\nb \\\"q\\\"\"\n"), "{out}");
    assert!(
        out.contains("    output:\n        x\n\n        y\n"),
        "{out}"
    );
    // a C compiler that always fails still leaves nothing behind
    d.file("m.dif", "void main() {\n}\n");
    let tmp = d.0.join("tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    for cc in ["true", "false"] {
        let r = Command::new(env!("CARGO_BIN_EXE_dif"))
            .current_dir(&d.0)
            .env("TMPDIR", &tmp)
            .env("CC", cc)
            .args(["build", "m.dif"])
            .output()
            .unwrap();
        let _ = r;
        assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0, "CC={cc}");
    }
}
