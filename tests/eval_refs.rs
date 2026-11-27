//! Every `eval/tasks/<name>/ref.dif` builds and passes all tests of its task byte for byte.

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn references_pass_their_tests() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmp = std::env::temp_dir().join(format!("dif-eval-refs-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut tasks: Vec<_> = std::fs::read_dir(root.join("eval/tasks")).unwrap().map(|e| e.unwrap().path()).collect();
    tasks.sort();
    assert_eq!(tasks.len(), 5);
    for task in tasks {
        let name = task.file_name().unwrap().to_str().unwrap().to_string();
        let src = std::fs::read_to_string(task.join("ref.dif")).unwrap_or_else(|_| panic!("{name}: no ref.dif"));
        let c = dif::compile("ref.dif", src.as_bytes()).unwrap_or_else(|d| panic!("{name}: {d:#?}"));
        let (c_file, exe) = (tmp.join(format!("{name}.c")), tmp.join(&name));
        std::fs::write(&c_file, c).unwrap();
        let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
        let st = Command::new(cc)
            .args(["-std=c11", "-O2", "-ffp-contract=off", "-Wall", "-Wextra", "-Werror", "-o"])
            .arg(&exe)
            .arg(&c_file)
            .arg("-lm")
            .status()
            .unwrap();
        assert!(st.success(), "{name}: C compiler failed");
        let mut ins: Vec<_> = std::fs::read_dir(task.join("tests")).unwrap().map(|e| e.unwrap().path()).collect();
        ins.retain(|p| p.extension().is_some_and(|e| e == "in"));
        assert!(!ins.is_empty());
        for i in ins {
            let mut child = Command::new(&exe).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
            child.stdin.take().unwrap().write_all(&std::fs::read(&i).unwrap()).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "{name}/{}: exit {:?}", i.display(), out.status);
            assert_eq!(out.stdout, std::fs::read(i.with_extension("out")).unwrap(), "{name}/{}", i.display());
        }
    }
}
