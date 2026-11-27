//! Fewer copies: the hint `H1.param_copy` of `dif check --hints`, and the
//! measurement that `xs = bump(xs, k)` does not copy the list on every call.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

const BUMP: &str = "[]int bump([]int xs, int k) {
    xs[k] = xs[k] + 1
    return xs
}

void bump_mut(mut []int xs, int k) {
    xs[k] = xs[k] + 1
}

void main() {
    []int xs = repeat(0, 1000000)
    for int i in range(1000) {
        xs = bump(xs, i)
    }
    print(xs[0] + xs[999] + xs.len())
}
";

fn work(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dif-copies-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Best of five wall times (ms) of the program and its output.
fn time(name: &str, src: &str) -> (f64, String) {
    let c = dif::compile("main.dif", src.as_bytes()).unwrap_or_else(|d| panic!("{d:#?}"));
    let dir = work(name);
    let (c_file, exe) = (dir.join("main.c"), dir.join("main"));
    std::fs::write(&c_file, c).unwrap();
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let built = Command::new(cc)
        .args(["-std=c11", "-O2", "-ffp-contract=off", "-o"])
        .arg(&exe)
        .arg(&c_file)
        .arg("-lm")
        .output()
        .unwrap();
    assert!(built.status.success());
    let mut best = f64::MAX;
    let mut out = String::new();
    for _ in 0..5 {
        let t = Instant::now();
        let r = Command::new(&exe).output().unwrap();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
        out = String::from_utf8_lossy(&r.stdout).into_owned();
    }
    let _ = std::fs::remove_dir_all(&dir);
    (best, out)
}

#[test]
fn last_use_does_not_copy() {
    let (ms, out) = time("move", BUMP);
    assert_eq!(out, "1000002\n"); // xs[0] 1 + xs[999] 1 + len
                                  // before the move this took ~150 ms (every call copied 8 MB); allow a loaded machine
    assert!(ms < 40.0, "1000 x bump on 1 000 000 ints took {ms} ms");
    // a caller that still needs the list pays the copy: the hint is about this case
    let keep = BUMP.replace(
        "xs = bump(xs, i)",
        "[]int ys = bump(xs, i)\n        xs[0] += ys[0]",
    );
    let (slow, _) = time("copy", &keep);
    assert!(slow > ms * 3.0, "{slow} ms against {ms} ms");
}

fn hints(src: &str) -> Vec<String> {
    dif::check_hints("main.dif", src.as_bytes())
        .iter()
        .map(|d| format!("{} {} {}", d.id, d.severity(), d.pos.line))
        .collect()
}

#[test]
fn hint_names_a_changed_parameter_without_mut() {
    assert_eq!(hints(BUMP), ["H1.param_copy note 1"]);
    // a changed int copies nothing; mut and unchanged parameters are fine
    let ok = "int f(int n) {
    n += 1
    return n
}

int g([]int xs) {
    return xs.len()
}

void h(mut []int xs) {
    xs.push(1)
}

void main() {
    print(f(1))
    print(g([1]))
}
";
    assert!(hints(ok).is_empty());
    // a text and a struct with a list count as well
    let more = "struct Bag {
    []int items
}

str shout(str s) {
    s += \"!\"
    return s
}

Bag add(Bag b, int n) {
    b.items.push(n)
    return b
}

void main() {
    print(shout(\"a\"))
}
";
    assert_eq!(
        hints(more),
        ["H1.param_copy note 5", "H1.param_copy note 10"]
    );
}

#[test]
fn hints_are_notes_and_do_not_fail_the_check() {
    let dir = work("cli");
    let file = dir.join("main.dif");
    std::fs::write(&file, BUMP).unwrap();
    let dif = env!("CARGO_BIN_EXE_dif");
    let plain = Command::new(dif).arg("check").arg(&file).output().unwrap();
    assert!(plain.status.success() && plain.stderr.is_empty());
    let r = Command::new(dif)
        .args(["check", "--hints"])
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(0));
    let err = String::from_utf8_lossy(&r.stderr);
    assert!(
        err.contains("main.dif:1:18: note[H1]: `xs` is changed here"),
        "{err}"
    );
    let j = Command::new(dif)
        .args(["check", "--hints", "--json"])
        .arg(&file)
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&j.stdout);
    assert!(
        out.contains("\"id\":\"H1.param_copy\",\"severity\":\"note\""),
        "{out}"
    );
    // an error still fails, and shows no hint
    std::fs::write(&file, "void main() {\n    int x = \"a\"\n}\n").unwrap();
    let bad = Command::new(dif)
        .args(["check", "--hints"])
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(1));
    let _ = std::fs::remove_dir_all(&dir);
}
