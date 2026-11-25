//! The C backend: generated C compiles without warnings, programs run with the
//! semantics of R11, R14 and R29, and `dif run` reports panics and missing compilers.

use std::path::PathBuf;
use std::process::Command;

fn work(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dif-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Compiles with all warnings as errors, runs, and returns (stdout, stderr, exit code).
fn run(name: &str, src: &str) -> (String, String, i32) {
    let c = dif::compile("main.dif", src.as_bytes()).unwrap_or_else(|d| panic!("{d:#?}"));
    let dir = work(name);
    let (c_file, exe) = (dir.join("main.c"), dir.join("main"));
    std::fs::write(&c_file, c).unwrap();
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let out = Command::new(cc)
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-o"])
        .arg(&exe)
        .arg(&c_file)
        .output()
        .expect("a C compiler (cc) is needed for these tests");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = Command::new(&exe).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&r.stdout).into_owned(),
        String::from_utf8_lossy(&r.stderr).into_owned(),
        r.status.code().unwrap_or(-1),
    )
}

fn example(name: &str) -> String {
    std::fs::read_to_string(format!("{}/examples/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn examples_run() {
    assert_eq!(
        run("hello", &example("hello.dif")),
        ("Hallo, Welt\n".into(), String::new(), 0)
    );
    let (out, _, code) = run("fib", &example("fib.dif"));
    assert_eq!(code, 0);
    assert!(
        out.starts_with("fib(0) = 0\nfib(1) = 1\nfib(2) = 1\n"),
        "{out}"
    );
    assert!(out.ends_with("fib(9) = 34\n832040\n"), "{out}");
}

#[test]
fn semantics() {
    let src = r#"int f(int x) {
    print(x)
    return x
}

const int K = 2 * L
const int L = 21

void main() {
    int big = 9223372036854775807
    print(big +% 1)
    print(-7 / 2)
    print(-7 % 3)
    print(1 << 62)
    print(-9223372036854775808)
    print(f(1) + f(2))
    str s = "k={K} ok={K == 42} {f(3)}" + "!"
    print(s)
    print(s < "l")
    for int i in range(3) {
        if i == 1 {
            continue
        }
        i += 10
        print(i)
    }
    int n = 0
    while true {
        n += 1
        if n == 3 {
            break
        }
    }
    print(n)
}
"#;
    let (out, err, code) = run("semantics", src);
    let want = "-9223372036854775808\n-3\n-1\n4611686018427387904\n-9223372036854775808\n1\n2\n3\n3\nk=42 ok=true 3!\ntrue\n10\n12\n3\n";
    assert_eq!((out.as_str(), err.as_str(), code), (want, "", 0));
}

#[test]
fn panics() {
    let cases = [
        (
            "int big = 9223372036854775807\nprint(big + 1)",
            "integer overflow in + (main.dif:3:15)",
        ),
        (
            "int w = 64\nprint(1 << w)",
            "shift by 64 out of range 0..63 (main.dif:3:13)",
        ),
        (
            "int z = 0\nprint(10 / z)",
            "division by zero (main.dif:3:14)",
        ),
        (
            "int z = 0\nprint(10 % z)",
            "division by zero (main.dif:3:14)",
        ),
        (
            "int m = -9223372036854775807 - 1\nint d = -1\nprint(m / d)",
            "integer overflow in / (main.dif:4:13)",
        ),
        (
            "int m = -9223372036854775807 - 1\nprint(-m)",
            "integer overflow in - (main.dif:3:11)",
        ),
        (
            "int m = 3037000500\nprint(m * m)",
            "integer overflow in * (main.dif:3:13)",
        ),
    ];
    for (k, (body, msg)) in cases.iter().enumerate() {
        let lines: Vec<String> = body.lines().map(|l| format!("    {l}")).collect();
        let src = format!("void main() {{\n{}\n}}\n", lines.join("\n"));
        let (out, err, code) = run(&format!("panic{k}"), &src);
        assert_eq!(
            (out.as_str(), err.as_str(), code),
            ("", format!("panic: {msg}\n").as_str(), 2),
            "{body}"
        );
    }
    // INT_MIN % -1 is 0, not an abort (R29)
    let (out, _, _) = run("modmin", "void main() {\n    int m = -9223372036854775807 - 1\n    int d = -1\n    print(m % d)\n}\n");
    assert_eq!(out, "0\n");
}

#[test]
fn driver() {
    let dif = env!("CARGO_BIN_EXE_dif");
    let hello = format!("{}/examples/hello.dif", env!("CARGO_MANIFEST_DIR"));
    let r = Command::new(dif).args(["run", &hello]).output().unwrap();
    assert_eq!(
        (String::from_utf8_lossy(&r.stdout).as_ref(), r.status.code()),
        ("Hallo, Welt\n", Some(0))
    );
    let r = Command::new(dif)
        .args(["run", &hello])
        .env("CC", "no-such-cc")
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&r.stderr).contains("no C compiler found"));
    // build writes a program; a file without main is R1
    let dir = work("driver");
    let exe = dir.join("hello");
    let r = Command::new(dif)
        .args(["build", &hello, "-o"])
        .arg(&exe)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(
        Command::new(&exe).output().unwrap().stdout,
        b"Hallo, Welt\n"
    );
    let lib = dir.join("lib.dif");
    std::fs::write(&lib, "int f() {\n    return 1\n}\n").unwrap();
    let r = Command::new(dif).arg("run").arg(&lib).output().unwrap();
    assert_eq!(r.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&r.stderr).contains("error[R1]: no `main` function"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shapes_example_runs() {
    let (out, err, code) = run("shapes", &example("shapes.dif"));
    assert_eq!((err.as_str(), code), ("", 0));
    assert!(out.starts_with("Canvas{last: Shape.Rect(Point{x: 2, y: 5}, 4, 6), drawn: 3}\n"));
    assert!(out.ends_with("area 16 24\nshape\norigin\n"), "{out}");
}

#[test]
fn values_places_and_order() {
    let src = r#"
struct Box {
    int n
    int m = 5
}

int log(int n) {
    print("log {n}")
    return n
}

void add(mut Box b, int k) {
    b.n += k
}

int bump(mut Box b) {
    b.n += 10
    return b.n
}

void Box.twice() {
    self.n *= 2
}

int read(mut Box b, int seen, int later) {
    b.n += 1
    return seen * 1000 + later
}

void main() {
    Box a = Box{n: 1}
    Box copy = a
    add(mut a, 4)
    print("{a.n} {copy.n}")
    // the argument that reads `a` sees the value from before the call
    int r = read(mut a, a.n, bump(mut a))
    print("{r} {a.n}")
    // fields left to right, then the defaults
    Box b = Box{m: log(2), n: log(1)}
    print(b)
    a.twice()
    print(a.n)
    // only the chosen branch runs
    int v = if a.n > 0 { log(7) } else { log(8) }
    print(v)
    for int i in range(4) {
        switch i {
        case 1:
            continue
        default:
            print(i)
        }
    }
}
"#;
    let (out, err, code) = run("values", src);
    assert_eq!((err.as_str(), code), ("", 0));
    assert_eq!(
        out,
        "5 1\n5015 16\nlog 2\nlog 1\nBox{n: 1, m: 2}\n32\nlog 7\n7\n0\n2\n3\n"
    );
}

/// `n - c` after `if n < K { return }` needs no check, but only for a parameter that is
/// never changed; everything else keeps its panic.
#[test]
fn range_facts_keep_checks() {
    let src = r#"int fib(int n) {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

int big(int n) {
    if n < 2 {
        return 4611686018427387904
    }
    return big(n - 1) + big(n - 1)
}

int low(int n) {
    if n < 2 {
        return n
    }
    n = -9223372036854775807 - 1
    return n - 1
}

void main() {
    print(fib(20))
    print(big(1))
    print(low(5))
}
"#;
    let (out, err, code) = run("range", src);
    assert_eq!(out, "6765\n4611686018427387904\n");
    assert_eq!(err, "panic: integer overflow in - (main.dif:20:14)\n");
    assert_eq!(code, 2);
    let (out, err, code) = run("range2", &src.replace("big(1)", "big(2)"));
    assert_eq!(out, "6765\n");
    assert_eq!(err, "panic: integer overflow in + (main.dif:12:23)\n");
    assert_eq!(code, 2);
}

/// A parameter passed as `mut` is changed, so no bound survives and `n - 1` stays checked.
#[test]
fn range_facts_stop_at_mut_argument() {
    let src = r#"void smash(mut int p) {
    p = -9223372036854775807 - 1
}

int low(int n) {
    if n < 2 {
        return n
    }
    smash(mut n)
    return n - 1
}

void main() {
    print(low(5))
}
"#;
    let (out, err, code) = run("rangemut", src);
    assert_eq!((out.as_str(), code), ("", 2));
    assert_eq!(err, "panic: integer overflow in - (main.dif:10:14)\n");
}
