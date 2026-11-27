//! Golden tests: `tests/ok/*.dif` run and print `*.out`; `tests/err/*.dif` give
//! exactly the diagnostic named in their first line (`// expect: error[R13] 4:5`).
//! The examples of docs/spec.md are checked the same way (§0.2).

use std::path::Path;
use std::process::Command;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// A fresh directory for one build (tests run in parallel and may use the same names).
fn unique_dir(name: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("dif-golden-{}-{k}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Builds and runs a program; returns (stdout, stderr, exit code). With `san` the program is
/// built with AddressSanitizer and UBSan, leaks included.
fn run_in(name: &str, src: &str, san: bool) -> (String, String, i32) {
    let c = dif::compile("main.dif", src.as_bytes()).unwrap_or_else(|d| panic!("{name}: {d:#?}"));
    let dir = unique_dir(name);
    let (c_file, exe) = (dir.join("main.c"), dir.join("main"));
    std::fs::write(&c_file, c).unwrap();
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let san: &[&str] = if san {
        &[
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=undefined",
            "-g",
        ]
    } else {
        &["-Werror"]
    };
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
        "{name}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = Command::new(&exe).current_dir(&dir).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    (
        text(&r.stdout),
        text(&r.stderr),
        r.status.code().unwrap_or(-1),
    )
}

/// `dif test main.dif [--json]` in a fresh directory; returns (stdout, exit code).
fn dif_test(name: &str, src: &str, json: bool) -> (String, i32) {
    let dir = unique_dir(name);
    std::fs::write(dir.join("main.dif"), src).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dif"));
    cmd.current_dir(&dir).args(["test", "main.dif"]);
    if json {
        cmd.arg("--json");
    }
    let r = cmd.output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&r.stdout).into_owned(),
        r.status.code().unwrap_or(-1),
    )
}

fn files(dir: &str) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(Path::new(ROOT).join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "dif"))
        .collect();
    v.sort();
    v
}

#[test]
fn ok_programs_print_their_output() {
    ok_programs(false);
}

/// The same programs under AddressSanitizer and UBSan: no leak, no undefined behaviour.
#[test]
fn ok_programs_leak_nothing() {
    ok_programs(true);
}

fn ok_programs(san: bool) {
    let all = files("tests/ok");
    assert!(all.len() >= 10);
    for f in all {
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(&f).unwrap();
        let want = std::fs::read_to_string(f.with_extension("out")).unwrap();
        assert_eq!(run_in(&name, &src, san), (want, String::new(), 0), "{name}");
    }
}

#[test]
fn err_programs_give_their_diagnostic() {
    let all = files("tests/err");
    assert!(all.len() >= 10);
    for f in all {
        let src = std::fs::read_to_string(&f).unwrap();
        let want = src
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("// expect: "))
            .unwrap_or_else(|| panic!("{}: first line must be // expect: …", f.display()));
        let got: Vec<String> = dif::check_source(&src)
            .iter()
            .map(|d| format!("error[{}] {}:{}", d.code, d.pos.line, d.pos.col))
            .collect();
        assert_eq!(got, [want], "{}", f.display());
    }
}

#[test]
fn spec_examples() {
    let n = doc_examples("docs/spec.md", "spec", false);
    assert!(n >= 20, "only {n} examples found");
}

/// Every example of the spec once more under AddressSanitizer and UBSan.
#[test]
fn spec_examples_leak_nothing() {
    let n = doc_examples("docs/spec.md", "sanspec", true);
    assert!(n >= 20, "only {n} examples found");
}

#[test]
fn llms_txt() {
    let size = std::fs::metadata(Path::new(ROOT).join("llms.txt"))
        .unwrap()
        .len();
    assert!(
        size <= 24 * 1024,
        "llms.txt has {size} bytes, more than 24 KB"
    );
    assert!(doc_examples("llms.txt", "llms", false) >= 2);
}

#[test]
fn readme_examples() {
    assert!(doc_examples("README.md", "readme", false) >= 9);
}

/// Sections of the spec whose examples are checked but not built and run;
/// a section is listed here only while its backend is missing.
const CHECK_ONLY: &[&str] = &["### R26 ", "### R13, Teil 2d"];

/// Sections of part 2d (function literals, block argument, list methods with a function argument):
/// their examples need the compiler of part 2d and are skipped until it exists; the compiler
/// change removes them here.
const PENDING_2D: &[&str] = &["### 4.7 "];

/// The list methods with a function argument (`filter`, `map`, `sort_by`, `any`, `all`) come with
/// their own change; examples of part 2d that call one are skipped until then.
const PENDING_LIST_METHODS: &[&str] = &[".filter(", ".map(", ".sort_by(", ".any(", ".all("];

/// Every ```dif block of a document, by kind: valid module, snippet (inside
/// `void main()`), `error Rn` (exactly that diagnostic), `panic "message"`.
/// Returns the number of examples checked.
fn doc_examples(doc: &str, tag: &str, san: bool) -> usize {
    let spec = std::fs::read_to_string(Path::new(ROOT).join(doc)).unwrap();
    let lines: Vec<&str> = spec.lines().collect();
    let mut i = 0;
    let mut n = 0;
    let mut section = "";
    while i < lines.len() {
        let l = lines[i];
        i += 1;
        if l.starts_with("### ") {
            section = l;
        }
        let Some(kind) = l.strip_prefix("```dif") else {
            continue;
        };
        let start = i;
        while i < lines.len() && lines[i] != "```" {
            i += 1;
        }
        let body = &lines[start..i];
        i += 1;
        // an error example may be followed by its exact output in a plain block
        let mut k = i;
        while k < lines.len() && lines[k].is_empty() {
            k += 1;
        }
        let expected: Option<String> = (matches!(kind.trim(), "test")
            || kind.trim().starts_with("error"))
        .then_some(())
        .filter(|_| lines.get(k) == Some(&"```"))
        .map(|_| {
            let from = k + 1;
            let mut to = from;
            while lines[to] != "```" {
                to += 1;
            }
            lines[from..to].join("\n")
        });
        if PENDING_2D.iter().any(|p| section.starts_with(p)) {
            continue;
        }
        if PENDING_LIST_METHODS
            .iter()
            .any(|m| body.iter().any(|l| l.contains(m)))
        {
            continue;
        }
        let check_only = CHECK_ONLY.iter().any(|p| section.starts_with(p));
        n += 1;
        let kind = kind.trim();
        let src = if kind.starts_with("snippet") || kind.starts_with("panic") {
            let inner: Vec<String> = body.iter().map(|l| format!("    {l}")).collect();
            format!("void main() {{\n{}\n}}\n", inner.join("\n"))
        } else {
            body.join("\n") + "\n"
        };
        let diags = dif::check_source(&src);
        let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
        if let Some(code) = kind.strip_prefix("error ") {
            assert_eq!(codes, [code], "spec example {n}:\n{src}");
            if let Some(want) = &expected {
                assert_eq!(diags[0].render("main.dif"), *want, "{doc} example {n}");
            }
        } else {
            assert_eq!(codes, [] as [&str; 0], "spec example {n}:\n{src}");
        }
        if check_only {
            continue;
        }
        if let Some(msg) = kind.strip_prefix("panic ") {
            let (_, err, code) = run_in(&format!("{tag}{n}"), &src, san);
            assert_eq!(code, 2, "spec example {n}");
            assert!(
                err.starts_with(&format!("panic: {}", msg.trim_matches('"'))),
                "spec example {n}: {err}"
            );
        } else if let Some(text) = kind.strip_prefix("fails ") {
            // `void!E main()` ends with `error: <text>` on stderr and exit code 1 (R12)
            let (_, err, code) = run_in(&format!("{tag}{n}"), &src, san);
            assert_eq!(code, 1, "spec example {n}");
            assert_eq!(
                err,
                format!("error: {}\n", text.trim_matches('"')),
                "{doc} example {n}"
            );
        } else if kind == "test" {
            // `dif test main.dif` prints the block that follows; with `--json`, the line after it
            let want = expected.expect("a `dif test` example is followed by its output");
            let (out, code) = dif_test(&format!("{tag}{n}"), &src, false);
            assert_eq!((out, code), (want + "\n", 1), "{doc} example {n}");
            let json = lines[k..]
                .iter()
                .take(12)
                .find(|l| l.starts_with("{\"tests\":"));
            if let Some(json) = json {
                let (out, code) = dif_test(&format!("{tag}{n}j"), &src, true);
                assert_eq!(
                    (out.as_str(), code),
                    (&format!("{json}\n")[..], 1),
                    "{doc} example {n}"
                );
            }
        } else if kind.is_empty() || kind.starts_with("snippet") {
            // valid examples also build and run (modules without main only check)
            if src.contains("void main()") {
                let (out, err, code) = run_in(&format!("{tag}{n}"), &src, san);
                assert_eq!((err.as_str(), code), ("", 0), "{doc} example {n}");
                // `Ausgabe: `a`, `b`.` (or `Output: …`) after the example: one printed line per item
                if let Some(l) = lines
                    .get(k)
                    .and_then(|l| l.strip_prefix("Ausgabe: ").or(l.strip_prefix("Output: ")))
                {
                    let items: Vec<&str> = l.split('`').skip(1).step_by(2).collect();
                    assert_eq!(out, items.join("\n") + "\n", "{doc} example {n}");
                }
            }
        }
    }
    n
}
