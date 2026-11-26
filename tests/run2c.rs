//! Part 2c at run time: aborts (R14), stdin, files, arguments, the environment and the exit
//! codes of the driver. Programs are built with all warnings as errors; with `DIF_SAN=1`
//! also under AddressSanitizer and UBSan, leaks included.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn work(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dif-run2c-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(name: &str, src: &str) -> (PathBuf, PathBuf) {
    let c = dif::compile("main.dif", src.as_bytes()).unwrap_or_else(|d| panic!("{d:#?}"));
    let dir = work(name);
    let (c_file, exe) = (dir.join("main.c"), dir.join("main"));
    std::fs::write(&c_file, c).unwrap();
    let san: &[&str] = if std::env::var_os("DIF_SAN").is_some() {
        &[
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=undefined",
            "-g",
        ]
    } else {
        &["-Werror"]
    };
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let out = Command::new(cc)
        .args(["-std=c11", "-O2", "-ffp-contract=off", "-Wall", "-Wextra"])
        .args(san)
        .arg("-o")
        .arg(&exe)
        .arg(&c_file)
        .arg("-lm")
        .output()
        .expect("a C compiler (cc) is needed for these tests");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    (dir, exe)
}

struct Run {
    out: String,
    err: String,
    code: i32,
}

/// Builds and runs a program with `stdin` (bytes), arguments and extra environment.
fn run_with(name: &str, src: &str, stdin: &[u8], args: &[&str], env: &[(&str, &str)]) -> Run {
    let (dir, exe) = build(name, src);
    let mut child = Command::new(&exe)
        .args(args)
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let r = child.wait_with_output().unwrap();
    let _ = std::fs::remove_dir_all(dir);
    Run {
        out: String::from_utf8_lossy(&r.stdout).into_owned(),
        err: String::from_utf8_lossy(&r.stderr).into_owned(),
        code: r.status.code().unwrap_or(-1),
    }
}

fn run(name: &str, src: &str) -> Run {
    run_with(name, src, b"", &[], &[])
}

fn in_main(body: &str) -> String {
    let lines: Vec<String> = body.lines().map(|l| format!("    {l}")).collect();
    format!("void main() {{\n{}\n}}\n", lines.join("\n"))
}

#[test]
fn aborts_of_part_2c() {
    // (program text, message with position); line 1 of the program is `void main() {`
    let cases = [
        (
            "u8 a = 200\nu8 b = 100\nprint(a + b)",
            "integer overflow in + (main.dif:4:13)",
        ),
        (
            "i8 a = -128\ni8 b = -1\nprint(a / b)",
            "integer overflow in / (main.dif:4:13)",
        ),
        (
            "i8 a = -128\nprint(-a)",
            "integer overflow in - (main.dif:3:11)",
        ),
        (
            "u8 a = 1\nu8 w = 8\nprint(a << w)",
            "shift by 8 out of range 0..7 (main.dif:4:13)",
        ),
        (
            "i32 m = -2147483648\nprint(m.abs())",
            "integer overflow in abs (main.dif:3:13)",
        ),
        (
            "int z = 0\nprint(10 / z)",
            "division by zero (main.dif:3:14)",
        ),
        (
            "[]int xs = [1, 2, 3]\nint i = -1\nprint(xs[i])",
            "index -1 out of bounds for length 3 (main.dif:4:13)",
        ),
        (
            "[3]int a = [1, 2, 3]\nint i = 3\nprint(a[i])",
            "index 3 out of bounds for length 3 (main.dif:4:12)",
        ),
        (
            "[]int xs = [1]\nxs[1] = 2\nprint(xs)",
            "index 1 out of bounds for length 1 (main.dif:3:7)",
        ),
        (
            "[]int xs = [1]\nprint(xs.remove(1))\nprint(xs)",
            "index 1 out of bounds for length 1 (main.dif:3:14)",
        ),
        (
            "[]int xs = [1]\nxs.insert(2, 5)\nprint(xs)",
            "insert index 2 out of bounds for length 1 (main.dif:3:8)",
        ),
        (
            "[]int xs = [1, 2]\nprint(xs.slice(1, 3))",
            "slice 1..3 out of bounds for length 2 (main.dif:3:14)",
        ),
        (
            "[]int xs = [1, 2]\nprint(xs.slice(2, 1))",
            "slice 2..1 out of bounds for length 2 (main.dif:3:14)",
        ),
        (
            "[2]int a = [1, 2]\nint k = -1\nprint(a.slice(k, 1))",
            "slice -1..1 out of bounds for length 2 (main.dif:4:13)",
        ),
        (
            "print(\"ab\".slice(0, 3))",
            "slice 0..3 out of bounds for length 2 (main.dif:2:16)",
        ),
        (
            "print(\"äö\".slice(1, 2))",
            "slice 1..2 is not on character boundaries (main.dif:2:16)",
        ),
        (
            "print(repeat(0, -1))",
            "repeat count -1 is negative (main.dif:2:11)",
        ),
        (
            "print(\"a\".repeat(-2))",
            "repeat count -2 is negative (main.dif:2:15)",
        ),
        (
            "print(\"a\".split(\"\"))",
            "split needs a non-empty separator (main.dif:2:15)",
        ),
        (
            "print(\"a\".replace(\"\", \"b\"))",
            "replace needs a non-empty text to find (main.dif:2:15)",
        ),
        (
            "int n = 300\nprint(u8(n))",
            "300 does not fit in u8 (main.dif:3:11)",
        ),
        (
            "int n = -1\nprint(u64(n))",
            "-1 does not fit in u64 (main.dif:3:11)",
        ),
        (
            "f64 x = 2.5\nprint(int(x))",
            "2.5 is not an exact int (main.dif:3:11)",
        ),
        (
            "f64 x = 0.0 / 0.0\nprint(i8(x))",
            "NaN is not an exact i8 (main.dif:3:11)",
        ),
        (
            "f64 x = 1e30\nprint(int(x))",
            "1e30 does not fit in int (main.dif:3:11)",
        ),
        (
            "int n = 9007199254740993\nprint(f64(n))",
            "9007199254740993 is not an exact f64 (main.dif:3:11)",
        ),
        (
            "int n = 55296\nprint(rune(n))",
            "55296 is not a valid rune (main.dif:3:11)",
        ),
        (
            "map[f64]int m = {}\nm[0.0 / 0.0] = 1\nprint(m)",
            "NaN cannot be a map key (main.dif:3:6)",
        ),
        (
            "map[f64]int m = {}\nprint(m[0.0 / 0.0])",
            "NaN cannot be a map key (main.dif:3:12)",
        ),
        (
            "map[f64]int m = {}\nprint(m.has(0.0 / 0.0))",
            "NaN cannot be a map key (main.dif:3:13)",
        ),
        (
            "map[f64]int m = {}\nprint(m.remove(0.0 / 0.0))\nprint(m)",
            "NaN cannot be a map key (main.dif:3:13)",
        ),
        (
            "map[[]f64]int m = {}\nprint(m[[1.0, 0.0 / 0.0]])",
            "NaN cannot be a map key (main.dif:3:12)",
        ),
        ("assert(1 > 2)", "assertion failed: 1 > 2 (main.dif:2:5)"),
        (
            "int x = 3\nassert(x == 4 ||\n    x == 5)",
            "assertion failed: x == 4 || x == 5 (main.dif:3:5)",
        ),
        (
            "os.exit(256)",
            "exit code 256 out of range 0..255 (main.dif:2:8)",
        ),
    ];
    let mut bad = Vec::new();
    for (k, (body, msg)) in cases.iter().enumerate() {
        let needs_os = body.contains("os.");
        let src = if needs_os {
            format!("use std.os\n\n{}", in_main(body))
        } else {
            in_main(body)
        };
        // `use` lines shift the positions by two lines
        let shift = if needs_os { 2 } else { 0 };
        let want = if shift == 0 {
            msg.to_string()
        } else {
            msg.replace("main.dif:2:", "main.dif:4:")
        };
        let r = run(&format!("abort{k}"), &src);
        if (r.out.as_str(), r.err.as_str(), r.code) != ("", format!("panic: {want}\n").as_str(), 2)
        {
            bad.push(format!(
                "{body}\n  want {want}\n  got  {:?} {}",
                r.err, r.code
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn overlapping_mut_elements_abort() {
    let src = r#"void swap(mut int a, mut int b) {
    int t = a
    a = b
    b = t
}

void main() {
    []int xs = [1, 2, 3]
    int i = 0
    for int j in [1, 2, 0] {
        swap(mut xs[i], mut xs[j])
        print(xs)
    }
}
"#;
    let r = run("overlap", src);
    assert_eq!(r.out, "[2, 1, 3]\n[3, 1, 2]\n");
    assert_eq!(
        r.err,
        "panic: overlapping mut arguments: index 0 (main.dif:11:9)\n"
    );
    assert_eq!(r.code, 2);
}

#[test]
fn out_of_memory_has_no_position() {
    // AddressSanitizer refuses such a request itself, with its own report
    if std::env::var_os("DIF_SAN").is_some() {
        return;
    }
    // a list of 4 * 10^12 arrays of 64 bytes cannot be allocated (R14)
    let src = in_main("[8]int z = [0, 0, 0, 0, 0, 0, 0, 0]\n[][8]int big = repeat(z, 4000000000000)\nprint(big.len())");
    let r = run("oom", &src);
    assert_eq!(r.out, "");
    assert_eq!(r.err, "panic: out of memory (main.dif)\n");
    assert_eq!(r.code, 2);
}

const LINES: &str = r#"use std.io

void!io.IoErr main() {
    int n = 0
    for int i, str line in try io.lines() {
        print("{i}: [{line}]")
        n += 1
        if line == "stop" {
            break
        }
    }
    ?str next = try io.read_line()
    print(next)
    for str rest in try io.lines() {
        print("rest [{rest}]")
    }
    print(try io.read_line())
    print(n)
}
"#;

#[test]
fn stdin_is_one_stream_of_lines() {
    let r = run_with("lines", LINES, b"a\r\nb\n\nstop\nafter\r\nlast\r", &[], &[]);
    assert_eq!(
        r.out,
        "0: [a]\n1: [b]\n2: []\n3: [stop]\nsome(\"after\")\nrest [last]\nnone\n4\n"
    );
    assert_eq!((r.err.as_str(), r.code), ("", 0));
    let r = run_with("lines0", LINES, b"", &[], &[]);
    assert_eq!(r.out, "none\nnone\n0\n");
}

#[test]
fn invalid_utf8_on_stdin_is_an_error_and_ends_the_loop() {
    let src = r#"use std.io

void!io.IoErr main() {
    for str!io.IoErr line in io.lines() {
        print(line)
    }
    print("next: {io.read_line()}")
}
"#;
    let r = run_with("badline", src, b"ok\n\xff\xfe\nnext line\n", &[], &[]);
    assert_eq!(
        r.out,
        "ok(\"ok\")\nerr(IoErr.InvalidUtf8(\"<stdin>\"))\nnext: ok(some(\"next line\"))\n"
    );
    let src = r#"use std.io

void!io.IoErr main() {
    for str line in try io.lines() {
        print(line)
    }
}
"#;
    let r = run_with("badline2", src, b"ok\n\xff\n", &[], &[]);
    assert_eq!(r.out, "ok\n");
    assert_eq!(r.err, "error: IoErr.InvalidUtf8(\"<stdin>\")\n");
    assert_eq!(r.code, 1);
}

#[test]
fn files_args_env_and_exit() {
    let dir = work("filesdata");
    let path = dir.join("out.txt");
    let path = path.to_str().unwrap();
    let src = r#"use std.io
use std.os

void!io.IoErr main() {
    []str args = os.args()
    str path = args[0]
    try io.write_file(path, "one\ntwo\n")
    str text = try io.read_file(path)
    print(text.lines())
    switch io.read_file("{path}.missing") {
    case ok(str t):
        print(t)
    case err(io.IoErr e):
        print(e)
    }
    switch io.write_file("/nonexistent-dir/x", "a") {
    case ok:
        print("written")
    case err(io.IoErr e):
        print(e)
    }
    print(args.len())
    print(os.env("DIF_TEST_VAR") ?? "unset")
    print(os.env("DIF_TEST_EMPTY"))
    print(os.env("DIF_NO_SUCH_VARIABLE"))
    io.eprint("to stderr")
    io.eprint([1, 2])
    os.exit(args.len() + 1)
}
"#;
    let r = run_with(
        "files",
        src,
        b"",
        &[path, "second"],
        &[("DIF_TEST_VAR", "set"), ("DIF_TEST_EMPTY", "")],
    );
    let missing = format!("{path}.missing");
    assert_eq!(
        r.out,
        format!("[\"one\", \"two\"]\nIoErr.NotFound(\"{missing}\")\nIoErr.NotFound(\"/nonexistent-dir/x\")\n2\nset\nsome(\"\")\nnone\n")
    );
    assert_eq!(r.err, "to stderr\n[1, 2]\n");
    assert_eq!(r.code, 3);
    // a file that is not UTF-8 is an error, never a replacement
    std::fs::write(path, b"\xff\xfe").unwrap();
    let src = r#"use std.io
use std.os

void main() {
    []str args = os.args()
    switch io.read_file(args[0]) {
    case ok(str t):
        print(t)
    case err(io.IoErr e):
        print(e)
    }
}
"#;
    let r = run_with("badfile", src, b"", &[path], &[]);
    assert_eq!(r.out, format!("IoErr.InvalidUtf8(\"{path}\")\n"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn invalid_utf8_in_arguments_and_environment_aborts() {
    use std::os::unix::ffi::OsStrExt;
    let (dir, exe) = build(
        "badargs",
        "use std.os\n\nvoid main() {\n    print(\"start\")\n    print(os.args())\n}\n",
    );
    let r = Command::new(&exe)
        .arg("ok")
        .arg(std::ffi::OsStr::from_bytes(b"\xff"))
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&r.stdout), "start\n");
    assert_eq!(
        String::from_utf8_lossy(&r.stderr),
        "panic: invalid UTF-8 in argument 1 (main.dif:5:14)\n"
    );
    assert_eq!(r.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(dir);
    let (dir, exe) = build(
        "badenv",
        "use std.os\n\nvoid main() {\n    print(os.env(\"BAD\"))\n}\n",
    );
    let r = Command::new(&exe)
        .env("BAD", std::ffi::OsStr::from_bytes(b"\xff"))
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&r.stderr),
        "panic: invalid UTF-8 in environment variable BAD (main.dif:4:14)\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn driver_links_the_math_library() {
    // `%` on f64 is fmod, `floor` and `round` are libm too: `dif build` must pass -lm
    let dir = work("libm");
    let src = dir.join("m.dif");
    std::fs::write(
        &src,
        "void main() {\n    f64 x = 7.5\n    print(x % 2)\n    print(x.floor())\n    print(x.round())\n    print((2.0).sqrt())\n}\n",
    )
    .unwrap();
    let exe = dir.join("m");
    let r = Command::new(env!("CARGO_BIN_EXE_dif"))
        .args(["build"])
        .arg(&src)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let r = Command::new(&exe).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&r.stdout),
        "1.5\n7\n8\n1.4142135623730951\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn driver_exit_code_3_is_a_c_compiler_that_rejects_the_code() {
    let dir = work("exit3");
    let cc = dir.join("fakecc");
    std::fs::write(
        &cc,
        "#!/bin/sh\n[ \"$1\" = \"--version\" ] && exit 0\necho 'fake error' >&2\nexit 1\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&cc, std::fs::Permissions::from_mode(0o755)).unwrap();
    let hello = format!("{}/examples/hello.dif", env!("CARGO_MANIFEST_DIR"));
    let r = Command::new(env!("CARGO_BIN_EXE_dif"))
        .args(["run", &hello])
        .env("CC", &cc)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(3));
    let err = String::from_utf8_lossy(&r.stderr);
    assert!(
        err.contains("internal error") && err.contains("fake error"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(dir);
}
