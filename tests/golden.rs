//! Golden tests: `tests/ok/*.dif` run and print `*.out`; `tests/err/*.dif` give
//! exactly the diagnostic named in their first line (`// expect: error[R13] 4:5`).
//! The examples of docs/spec.md are checked the same way (§0.2).

use std::path::Path;
use std::process::Command;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// Builds and runs a program; returns (stdout, stderr, exit code).
fn run(name: &str, src: &str) -> (String, String, i32) {
    let c = dif::compile("main.dif", src.as_bytes()).unwrap_or_else(|d| panic!("{name}: {d:#?}"));
    let dir = std::env::temp_dir().join(format!("dif-golden-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
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
        "{name}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = Command::new(&exe).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    (
        text(&r.stdout),
        text(&r.stderr),
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
    let all = files("tests/ok");
    assert!(all.len() >= 10);
    for f in all {
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        let src = std::fs::read_to_string(&f).unwrap();
        let want = std::fs::read_to_string(f.with_extension("out")).unwrap();
        assert_eq!(run(&name, &src), (want, String::new(), 0), "{name}");
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

/// Every ```dif block of the spec, by kind: valid module, snippet (inside
/// `void main()`), `error Rn` (exactly that diagnostic), `panic "message"`.
#[test]
fn spec_examples() {
    let spec = std::fs::read_to_string(Path::new(ROOT).join("docs/spec.md")).unwrap();
    let mut lines = spec.lines();
    let mut n = 0;
    while let Some(l) = lines.next() {
        let Some(kind) = l.strip_prefix("```dif") else {
            continue;
        };
        let body: Vec<&str> = lines.by_ref().take_while(|l| *l != "```").collect();
        n += 1;
        let kind = kind.trim();
        let src = if kind.starts_with("snippet") || kind.starts_with("panic") {
            let inner: Vec<String> = body.iter().map(|l| format!("    {l}")).collect();
            format!("void main() {{\n{}\n}}\n", inner.join("\n"))
        } else {
            body.join("\n") + "\n"
        };
        let codes: Vec<&str> = dif::check_source(&src).iter().map(|d| d.code).collect();
        if let Some(code) = kind.strip_prefix("error ") {
            assert_eq!(codes, [code], "spec example {n}:\n{src}");
        } else {
            assert_eq!(codes, [] as [&str; 0], "spec example {n}:\n{src}");
        }
        if let Some(msg) = kind.strip_prefix("panic ") {
            let (_, err, code) = run(&format!("spec{n}"), &src);
            assert_eq!(code, 2, "spec example {n}");
            assert!(
                err.starts_with(&format!("panic: {}", msg.trim_matches('"'))),
                "spec example {n}: {err}"
            );
        } else if kind.is_empty() || kind.starts_with("snippet") {
            // valid examples also build and run (modules without main only check)
            if src.contains("void main()") {
                let (_, err, code) = run(&format!("spec{n}"), &src);
                assert_eq!((err.as_str(), code), ("", 0), "spec example {n}");
            }
        }
    }
    assert!(n >= 20, "only {n} examples found");
}
