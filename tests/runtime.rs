//! Runs the C tests of the runtime library `rt/dif_rt.h` (`tests/rt/*.c`).
//!
//! Each file is compiled with `-std=c11 -Wall -Wextra -Werror` and run: with the C compiler of the
//! driver (`$CC`, else cc/gcc/clang), with gcc and clang when they exist, and once with
//! AddressSanitizer + UndefinedBehaviorSanitizer including the leak check. `perf*.c` runs only
//! optimized and without sanitizers.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn works(cc: &str) -> bool {
    Command::new(cc)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Same choice as the driver: `$CC`, else cc, gcc, clang.
fn default_cc() -> String {
    if let Ok(cc) = std::env::var("CC") {
        if !cc.is_empty() {
            return cc;
        }
    }
    ["cc", "gcc", "clang"]
        .iter()
        .find(|c| works(c))
        .expect("no C compiler found (set CC)")
        .to_string()
}

fn sources(perf: bool) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join("tests/rt"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "c"))
        .filter(|p| p.file_name().unwrap().to_str().unwrap().starts_with("perf") == perf)
        .collect();
    v.sort();
    v
}

/// Compiles and runs every source file; collects all failures into one panic.
fn run_all(tag: &str, cc: &str, flags: &[&str], perf: bool) {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("rt-{tag}"));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut bad = String::new();
    for src in sources(perf) {
        let name = src.file_stem().unwrap().to_str().unwrap().to_string();
        let exe = tmp.join(&name);
        let cr = Command::new(cc)
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-g"])
            .args(flags)
            .arg("-I")
            .arg(root().join("rt"))
            .arg(&src)
            .arg("-lm")
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap();
        if !cr.status.success() {
            bad += &format!(
                "[{tag}] {name}: compile failed\n{}\n",
                String::from_utf8_lossy(&cr.stderr)
            );
            continue;
        }
        let rr = Command::new(&exe)
            .current_dir(root().join("tests/rt"))
            .env("DIF_TEST_TMP", &tmp)
            .env("ASAN_OPTIONS", "detect_leaks=1:abort_on_error=0")
            .output()
            .unwrap();
        if !rr.status.success() {
            bad += &format!(
                "[{tag}] {name}: {}\n{}{}\n",
                rr.status,
                String::from_utf8_lossy(&rr.stdout),
                String::from_utf8_lossy(&rr.stderr)
            );
        }
    }
    assert!(bad.is_empty(), "{bad}");
}

#[test]
fn strict_default_compiler() {
    run_all("cc", &default_cc(), &["-O2"], false);
}

#[test]
fn strict_gcc() {
    if works("gcc") {
        run_all("gcc", "gcc", &["-O2"], false);
    } else {
        eprintln!("gcc not found, skipped");
    }
}

#[test]
fn strict_clang() {
    if works("clang") {
        run_all("clang", "clang", &["-O2"], false);
    } else {
        eprintln!("clang not found, skipped (nix shell nixpkgs#clang)");
    }
}

/// The runtime itself must be leak free: LSan stays on.
#[test]
fn sanitizers() {
    run_all(
        "asan",
        &default_cc(),
        &[
            "-O1",
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=all",
        ],
        false,
    );
}

#[test]
fn sanitizers_clang() {
    if works("clang") {
        run_all(
            "asan-clang",
            "clang",
            &[
                "-O1",
                "-fsanitize=address,undefined",
                "-fno-sanitize-recover=all",
            ],
            false,
        );
    } else {
        eprintln!("clang not found, skipped");
    }
}

#[test]
fn performance() {
    run_all("perf", &default_cc(), &["-O2"], true);
}
