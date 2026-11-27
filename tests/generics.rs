//! Generics (R27): analysis of effects and last use inside generic bodies, one message per cause,
//! no invented names in messages, stable and collision-free names in the C code.

use std::process::Command;
use std::time::Instant;

const BUMP: &str = "[]T bump[T]([]T xs, int k, T v) {
    xs[k] = v
    return xs
}

void main() {
    []int xs = repeat(0, 1000000)
    for int i in range(1000) {
        xs = bump(xs, i, 1)
    }
    print(xs[0] + xs[999] + xs.len())
}
";

fn build_run(src: &str) -> (String, f64) {
    let c = dif::compile("main.dif", src.as_bytes()).unwrap_or_else(|d| panic!("{d:#?}"));
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("dif-generics-{}-{k}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
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
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let mut best = f64::MAX;
    let mut out = String::new();
    for _ in 0..5 {
        let t = Instant::now();
        let r = Command::new(&exe).output().unwrap();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
        out = String::from_utf8_lossy(&r.stdout).into_owned();
    }
    let _ = std::fs::remove_dir_all(&dir);
    (out, best)
}

fn ids(src: &str) -> Vec<String> {
    dif::check_source(src)
        .iter()
        .map(|d| format!("{} {}:{}", d.id, d.pos.line, d.pos.col))
        .collect()
}

/// The last use of a list passed through a generic function moves it, like for any function.
#[test]
fn last_use_moves_in_a_generic_function() {
    let (out, ms) = build_run(BUMP);
    assert_eq!(out, "1000002\n");
    assert!(ms < 40.0, "1000 x bump on 1 000 000 ints took {ms} ms");
}

/// The hint of a parameter that is changed without `mut` stands once, at the declaration, and
/// shows the generic header.
#[test]
fn hint_stands_once_at_the_declaration() {
    let hints: Vec<_> = dif::check_hints("main.dif", BUMP.as_bytes())
        .iter()
        .map(|d| format!("{} {}:{}", d.id, d.pos.line, d.pos.col))
        .collect();
    assert_eq!(hints, ["H1.param_copy 1:17"]);
    let two =
        format!("{BUMP}void other() {{\n    []str s = bump([\"a\"], 0, \"b\")\n    print(s)\n}}\n");
    let hints = dif::check_hints("main.dif", two.as_bytes());
    assert_eq!(hints.len(), 1, "{hints:?}");
    let hint = hints[0].hint.as_deref().unwrap_or("");
    assert!(hint.contains("bump[T](mut []T xs, int k, T v)"), "{hint}");
}

/// R6 and R13 hold in the generic text and for the callers: each cause is one message.
#[test]
fn effects_are_analysed_once() {
    let unneeded = "T first[T](mut []T xs) {
    return xs[0]
}

void main() {
    []int a = [1]
    []str b = [\"x\"]
    print(first(mut a))
    print(first(mut b))
}
";
    assert_eq!(ids(unneeded), ["R6.mut_unneeded 1:12"]);
    let lost = "void clear[T]([]T xs) {
    xs.push(xs[0])
}

void main() {
    []int a = [1]
    []str b = [\"x\"]
    clear(a)
    clear(b)
}
";
    // the change to the copy is lost once; the calls have no effect
    let got = ids(lost);
    assert_eq!(
        got.iter().filter(|g| g.starts_with("R6.")).count(),
        1,
        "{got:?}"
    );
    assert!(got.iter().all(|g| !g.starts_with("R28")), "{got:?}");
    let pure = "void nothing[T](T x) {
    T y = x
}

void main() {
    nothing(1)
    nothing(\"a\")
}
";
    assert_eq!(ids(pure), ["R13.no_effect 6:5", "R13.no_effect 7:5"]);
}

/// An error in a generic body is reported once, however many instances there are.
#[test]
fn a_body_error_is_reported_once() {
    let src = "struct Box[T] {
    T value
}

int Box[T].bad() {
    return self.value.foo
}

void main() {
    Box[int] a = Box{value: 1}
    Box[str] b = Box{value: \"x\"}
    Box[bool] c = Box{value: true}
    print(a.value)
    print(b.value)
    print(c.value)
}
";
    assert_eq!(ids(src), ["R8.no_field_type 6:12"]);
}

/// No message names a copy: no `$`, no `[` after a function name, no number prefix.
#[test]
fn messages_name_no_copy() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/err");
    let mut seen = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if !p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("generic_")
        {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        for d in dif::check_source(&src) {
            let text = format!("{} {}", d.msg, d.hint.unwrap_or_default());
            assert!(!text.contains('$'), "{}: {text}", p.display());
            seen += 1;
        }
    }
    assert!(seen >= 20, "only {seen} messages");
}

/// A user function whose name looks like the name of an instance does not clash with it.
#[test]
fn names_of_instances_are_free() {
    let src = "T max[ordered T](T a, T b) {
    return if a < b { b } else { a }
}

int max_int_(int a) {
    return a + 100
}

int d_0f_max_int_(int a) {
    return a + 1000
}

struct Pair[A, B] {
    A first
    B second
}

struct Pair_int_int_ {
    int x
}

void main() {
    print(max(1, 2))
    print(max_int_(1))
    print(d_0f_max_int_(1))
    Pair[int, int] p = Pair{first: 1, second: 2}
    Pair_int_int_ q = Pair_int_int_{x: 5}
    print(p)
    print(q)
}
";
    let (out, _) = build_run(src);
    assert_eq!(
        out,
        "2\n101\n1001\nPair{first: 1, second: 2}\nPair_int_int_{x: 5}\n"
    );
}

/// The same program gives the same C twice.
#[test]
fn c_code_is_stable() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ok/generic_stress.dif"),
    )
    .unwrap();
    let a = dif::compile("main.dif", src.as_bytes()).unwrap();
    let b = dif::compile("main.dif", src.as_bytes()).unwrap();
    assert_eq!(a, b);
}

/// A function type as type argument of a map key compiles and runs (R28: functions are hashable).
#[test]
fn function_type_as_map_key() {
    let src = "int size[T](map[T]int m) {
    return m.len()
}

void main() {
    map[(int(int))]int m = {}
    print(size(m))
}
";
    assert_eq!(ids(src), [] as [String; 0]);
    let (out, _) = build_run(src);
    assert_eq!(out, "0\n");
}

/// An unused type parameter gives its own message and no follow-up.
#[test]
fn unused_parameter_has_no_follow_up() {
    let src = "int unused[T, U](T x) {
    return 1
}

void main() {
    print(unused(1))
}
";
    assert_eq!(ids(src), ["R27.type_param_unused 1:15"]);
}

/// Instantiation that grows without end in width has its own message.
#[test]
fn instance_cap_has_its_own_id() {
    let src = "struct P[A] {
    A x
}

P[T] wrap[T](T x) {
    return P{x: x}
}

int f[T](T x) {
    return f(wrap(x)) + f([x])
}

void main() {
    print(f(1))
}
";
    let d = dif::check_source(src);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].id, "R27.instance_count");
    assert!(
        d[0].msg.ends_with("needs more than 4000 instances"),
        "{}",
        d[0].msg
    );
    assert!(d[0].hint.as_deref().unwrap_or("").contains("main.dif:"));
}
