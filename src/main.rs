use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod dif_test;

const USAGE: &str = "usage: dif check [--json] <file.dif>
       dif run <file.dif>
       dif build <file.dif> [-o <output>]
       dif test [path] [filter] [--json]
       dif version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("check") => check(&args[1..]),
        Some("run") => run(&args[1..]),
        Some("build") => build(&args[1..]),
        Some("test") => dif_test::run(&args[1..], usage, find_cc, work_dir, report),
        Some("version" | "--version" | "-v") => {
            println!("dif {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        _ => Err(usage("")),
    };
    result.unwrap_or_else(|code| code)
}

fn usage(msg: &str) -> ExitCode {
    if !msg.is_empty() {
        eprintln!("dif: {msg}");
    }
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

/// Options of one command: the file, `--json` and `-o <output>` where allowed.
struct Opts {
    file: String,
    json: bool,
    out: Option<String>,
}

fn opts(args: &[String], json_ok: bool, out_ok: bool) -> Result<Opts, ExitCode> {
    let mut o = Opts {
        file: String::new(),
        json: false,
        out: None,
    };
    let mut files = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" if json_ok => o.json = true,
            "-o" if out_ok => {
                o.out = Some(
                    it.next()
                        .ok_or_else(|| usage("-o needs a file name"))?
                        .clone(),
                )
            }
            s if s.starts_with('-') => return Err(usage(&format!("unknown option {s}"))),
            _ => files.push(a.clone()),
        }
    }
    match files.as_slice() {
        [f] => o.file = f.clone(),
        _ => return Err(usage("")),
    }
    Ok(o)
}

fn read(file: &str) -> Result<Vec<u8>, ExitCode> {
    std::fs::read(file).map_err(|e| {
        eprintln!("dif: cannot read {file}: {e}");
        ExitCode::from(2)
    })
}

fn report(file: &str, diags: &[dif::diag::Diag], json: bool) {
    for d in diags {
        if json {
            println!("{}", d.json(file));
        } else {
            eprintln!("{}", d.render(file));
        }
    }
}

fn check(args: &[String]) -> Result<ExitCode, ExitCode> {
    let o = opts(args, true, false)?;
    let diags = dif::check_bytes(&o.file, &read(&o.file)?);
    report(&o.file, &diags, o.json);
    Ok(if diags.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn build(args: &[String]) -> Result<ExitCode, ExitCode> {
    let o = opts(args, false, true)?;
    let out = o.out.clone().unwrap_or_else(|| {
        Path::new(&o.file)
            .file_stem()
            .map_or("a.out".into(), |s| s.to_string_lossy().into_owned())
    });
    let built = compile(&o.file, Path::new(&out));
    let _ = std::fs::remove_dir_all(work_dir()?);
    built?;
    Ok(ExitCode::SUCCESS)
}

fn run(args: &[String]) -> Result<ExitCode, ExitCode> {
    let o = opts(args, false, false)?;
    let dir = work_dir()?;
    let exe = dir.join("program");
    let built = compile(&o.file, &exe);
    let status = built.and_then(|()| {
        Command::new(&exe).status().map_err(|e| {
            eprintln!("dif: cannot start the program: {e}");
            ExitCode::from(2)
        })
    });
    let _ = std::fs::remove_dir_all(&dir);
    let status = status?;
    // a program ended by a signal has no exit code; 2 like a panic
    if let Some(sig) = std::os::unix::process::ExitStatusExt::signal(&status) {
        eprintln!("dif: program ended by signal {sig}");
    }
    Ok(ExitCode::from(status.code().unwrap_or(2) as u8))
}

fn work_dir() -> Result<PathBuf, ExitCode> {
    let dir = std::env::temp_dir().join(format!("dif-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| {
        eprintln!("dif: cannot create {}: {e}", dir.display());
        ExitCode::from(2)
    })?;
    Ok(dir)
}

/// `.dif` → C → native program at `out`.
fn compile(file: &str, out: &Path) -> Result<(), ExitCode> {
    let c = dif::compile(file, &read(file)?).map_err(|diags| {
        report(file, &diags, false);
        ExitCode::FAILURE
    })?;
    let cc = find_cc()?;
    let dir = work_dir()?;
    let c_file = dir.join("main.c");
    std::fs::write(&c_file, c).map_err(|e| {
        eprintln!("dif: cannot write {}: {e}", c_file.display());
        ExitCode::from(2)
    })?;
    let result = Command::new(&cc)
        .args([
            "-std=c11",
            "-O2",
            "-fno-strict-aliasing",
            "-ffp-contract=off",
            "-s",
            "-o",
        ])
        .arg(out)
        .arg(&c_file)
        .arg("-lm")
        .output();
    let _ = std::fs::remove_file(&c_file);
    match result {
        Ok(r) if r.status.success() => Ok(()),
        Ok(r) => {
            eprintln!("dif: internal error: the C compiler `{cc}` rejected the generated code");
            eprintln!("{}", String::from_utf8_lossy(&r.stderr));
            Err(ExitCode::from(3))
        }
        Err(e) => {
            eprintln!("dif: cannot run the C compiler `{cc}`: {e}");
            Err(ExitCode::from(2))
        }
    }
}

/// `$CC`, else the first of cc, gcc, clang that answers `--version`.
fn find_cc() -> Result<String, ExitCode> {
    let candidates: Vec<String> = match std::env::var("CC") {
        Ok(cc) if !cc.is_empty() => vec![cc],
        _ => ["cc", "gcc", "clang"].map(String::from).to_vec(),
    };
    for cc in &candidates {
        let ok = Command::new(cc)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            return Ok(cc.clone());
        }
    }
    eprintln!(
        "dif: no C compiler found (tried {}); install gcc or clang, or set CC",
        candidates.join(", ")
    );
    Err(ExitCode::from(2))
}
