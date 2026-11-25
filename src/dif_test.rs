//! `dif test [path] [filter] [--json]` (R18): every test runs as its own process of a
//! test binary built from the C of its file (`./prog <k>` runs test `k`).

use std::io::Read;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use dif::diag::json_str;
use dif::TestMeta;

const TIMEOUT: Duration = Duration::from_secs(10);

/// A module with the tests it declares.
struct Unit {
    file: String,
    src: Vec<u8>,
    c: String,
    tests: Vec<TestMeta>,
}

/// Where a test failed: line, column, byte offset.
type Place = (u32, u32, u32);

struct Failure {
    kind: &'static str,
    place: Option<Place>,
    message: String,
    values: Vec<(String, String)>,
    output: String,
}

/// `Err` for a test that failed.
type Outcome = Result<(), Failure>;

pub fn run(
    args: &[String],
    usage: impl Fn(&str) -> ExitCode,
    find_cc: impl Fn() -> Result<String, ExitCode>,
    work_dir: impl Fn() -> Result<PathBuf, ExitCode>,
    report: impl Fn(&str, &[dif::diag::Diag], bool),
) -> Result<ExitCode, ExitCode> {
    let (mut json, mut words) = (false, Vec::new());
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            s if s.starts_with('-') => return Err(usage(&format!("unknown option {s}"))),
            _ => words.push(a.as_str()),
        }
    }
    if words.len() > 2 {
        return Err(usage(""));
    }
    let path = words.first().copied().unwrap_or(".");
    let filter = words.get(1).copied();

    let mut files = Vec::new();
    collect(Path::new(path), &mut files).map_err(|e| {
        eprintln!("dif: cannot read {path}: {e}");
        ExitCode::from(2)
    })?;
    files.sort();

    // compile every file first: a file that does not compile stops the run (exit 2)
    let (mut units, mut broken) = (Vec::new(), false);
    for f in &files {
        let file = f.strip_prefix("./").unwrap_or(f).display().to_string();
        let src = std::fs::read(f).map_err(|e| {
            eprintln!("dif: cannot read {file}: {e}");
            ExitCode::from(2)
        })?;
        match dif::compile_tests(&file, &src) {
            Ok((c, tests)) => units.push(Unit {
                file,
                src,
                c,
                tests,
            }),
            Err(diags) => {
                report(&file, &diags, json);
                broken = true;
            }
        }
    }
    if broken {
        return Err(ExitCode::from(2));
    }

    let chosen = |t: &TestMeta| filter.is_none_or(|x| t.name.contains(x));
    let total: usize = units.iter().map(|u| u.tests.len()).sum();
    if units.iter().all(|u| !u.tests.iter().any(chosen)) {
        match filter {
            Some(x) if total > 0 => eprintln!("error: no test matches \"{x}\""),
            _ => eprintln!("error: no tests found"),
        }
        return Err(ExitCode::FAILURE);
    }

    let cc = find_cc()?;
    let dir = work_dir()?;
    let built = par_map(units.len(), |i| {
        let u = &units[i];
        if !u.tests.iter().any(chosen) {
            return Ok(PathBuf::new());
        }
        build(&cc, &dir, i, &u.c)
    });
    let mut exes = Vec::new();
    for b in built {
        match b {
            Ok(p) => exes.push(p),
            Err(e) => {
                eprintln!("{e}");
                let _ = std::fs::remove_dir_all(&dir);
                return Err(ExitCode::from(3));
            }
        }
    }

    let jobs: Vec<(usize, usize)> = (units.iter().enumerate())
        .flat_map(|(i, u)| (0..u.tests.len()).map(move |k| (i, k)))
        .filter(|&(i, k)| chosen(&units[i].tests[k]))
        .collect();
    let results = par_map(jobs.len(), |j| {
        let (i, k) = jobs[j];
        run_test(&exes[i], k, &units[i], &units[i].tests[k])
    });
    let _ = std::fs::remove_dir_all(&dir);

    let failed = results.iter().filter(|r| r.is_err()).count();
    let passed = results.len() - failed;
    let mut out = String::new();
    if json {
        out.push_str("{\"tests\":[");
        for (n, (&(i, k), r)) in jobs.iter().zip(&results).enumerate() {
            let (u, t) = (&units[i], &units[i].tests[k]);
            if n > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"file\":{},\"line\":{},\"name\":{},\"status\":\"{}\"",
                json_str(&u.file),
                t.line,
                json_str(&t.name),
                if r.is_ok() { "ok" } else { "fail" }
            ));
            if let Err(f) = r {
                out.push_str(&json_failure(f));
            }
            out.push('}');
        }
        out.push_str(&format!("],\"passed\":{passed},\"failed\":{failed}}}\n"));
    } else {
        for (&(i, k), r) in jobs.iter().zip(&results) {
            let (u, t) = (&units[i], &units[i].tests[k]);
            let head = format!("{}:{} test \"{}\"", u.file, t.line, escape(&t.name));
            match r {
                Ok(()) => out.push_str(&format!("ok   {head}\n")),
                Err(f) => {
                    out.push_str(&format!("FAIL {head}\n"));
                    match f.place {
                        Some((l, c, _)) => {
                            out.push_str(&format!("    {}:{l}:{c}: {}\n", u.file, f.message))
                        }
                        None => out.push_str(&format!("    {}\n", f.message)),
                    }
                    for (e, v) in &f.values {
                        out.push_str(&format!("        {e} = {v}\n"));
                    }
                    if !f.output.is_empty() {
                        out.push_str("    output:\n");
                        for l in f.output.split_terminator('\n') {
                            let pad = if l.is_empty() { "" } else { "        " };
                            out.push_str(&format!("{pad}{l}\n"));
                        }
                    }
                }
            }
        }
        out.push_str(&format!("test result: {passed} passed, {failed} failed\n"));
    }
    print!("{out}");
    Ok(if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// A test name as written in the source: `"`, `\` and control characters escaped.
fn escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' | '\\' => o.extend(['\\', c]),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            '\0' => o.push_str("\\0"),
            c if c < ' ' => o.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn json_failure(f: &Failure) -> String {
    let at = match f.place {
        Some((l, c, o)) => format!("\"line\":{l},\"column\":{c},\"offset\":{o}"),
        None => "\"line\":null,\"column\":null,\"offset\":null".into(),
    };
    let values: Vec<String> = (f.values.iter())
        .map(|(e, v)| format!("{{\"expr\":{},\"value\":{}}}", json_str(e), json_str(v)))
        .collect();
    format!(
        ",\"failure\":{{\"kind\":\"{}\",{at},\"message\":{},\"values\":[{}],\"output\":{}}}",
        f.kind,
        json_str(&f.message),
        values.join(","),
        json_str(&f.output)
    )
}

/// A file as it is named, or the `.dif` files of a directory, recursively.
fn collect(p: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !p.is_dir() {
        std::fs::metadata(p)?;
        out.push(p.to_path_buf());
        return Ok(());
    }
    for e in std::fs::read_dir(p)? {
        let p = e?.path();
        if p.is_dir() {
            collect(&p, out)?;
        } else if p.extension().is_some_and(|e| e == "dif") {
            out.push(p);
        }
    }
    Ok(())
}

/// `f(0..n)` on all cores; results in order.
fn par_map<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<T>>> = (0..n).map(|_| Mutex::new(None)).collect();
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..workers.min(n) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    break;
                }
                let v = f(i);
                *slots[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(v);
            });
        }
    });
    slots
        .into_iter()
        .filter_map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner()))
        .collect()
}

fn build(cc: &str, dir: &Path, i: usize, c: &str) -> Result<PathBuf, String> {
    let (c_file, exe) = (dir.join(format!("t{i}.c")), dir.join(format!("t{i}")));
    std::fs::write(&c_file, c)
        .map_err(|e| format!("dif: cannot write {}: {e}", c_file.display()))?;
    let r = Command::new(cc)
        .args(["-std=c11", "-O1", "-fno-strict-aliasing", "-o"])
        .arg(&exe)
        .arg(&c_file)
        .output()
        .map_err(|e| format!("dif: cannot run the C compiler `{cc}`: {e}"))?;
    if r.status.success() {
        Ok(exe)
    } else {
        Err(format!(
            "dif: internal error: the C compiler `{cc}` rejected the generated code\n{}",
            String::from_utf8_lossy(&r.stderr)
        ))
    }
}

fn run_test(exe: &Path, k: usize, u: &Unit, t: &TestMeta) -> Outcome {
    let spawned = Command::new(exe)
        .arg(k.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = spawned.map_err(|e| crash(format!("cannot start the test: {e}"), ""))?;
    let drain = |mut p: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = p.read_to_end(&mut v);
            String::from_utf8_lossy(&v).into_owned()
        })
    };
    let out = child.stdout.take().map(|p| drain(Box::new(p)));
    let err = child.stderr.take().map(|p| drain(Box::new(p)));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() < TIMEOUT => std::thread::sleep(Duration::from_millis(2)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let join = |h: Option<std::thread::JoinHandle<String>>| {
        h.and_then(|h| h.join().ok()).unwrap_or_default()
    };
    let (output, stderr) = (join(out), join(err));
    let Some(status) = status else {
        return Err(crash_kind(
            "timeout",
            "timed out after 10 s".into(),
            &output,
        ));
    };
    if let Some(sig) = status.signal() {
        return Err(crash_kind(
            "crash",
            format!("killed by signal {sig}"),
            &output,
        ));
    }
    let code = status.code().unwrap_or(-1);
    let line = |prefix: &str| stderr.lines().rev().find_map(|l| l.strip_prefix(prefix));
    match code {
        3 => match line("dif-test-fail\t") {
            Some(rec) => Err(record(rec, &output)),
            None => Err(crash("the test failed without a report".into(), &output)),
        },
        2 => match line("panic: ").and_then(parse_panic) {
            Some((msg, l, c)) => match &t.panics {
                Some(want) if msg.contains(want.as_str()) => Ok(()),
                Some(want) => Err(Failure {
                    kind: "panics",
                    place: None,
                    message: format!("expected a panic containing \"{want}\", got \"{msg}\""),
                    values: Vec::new(),
                    output,
                }),
                None => Err(Failure {
                    kind: "panic",
                    place: Some((l, c, offset(&u.src, l, c))),
                    message: format!("panic: {msg}"),
                    values: Vec::new(),
                    output,
                }),
            },
            None => Err(crash(
                "exited with code 2 without a panic message".into(),
                &output,
            )),
        },
        0 => match &t.panics {
            Some(want) => Err(Failure {
                kind: "panics",
                place: None,
                message: format!("expected a panic containing \"{want}\", but the test finished"),
                values: Vec::new(),
                output,
            }),
            None => Ok(()),
        },
        n => Err(crash(format!("exited with code {n}"), &output)),
    }
}

fn crash(message: String, output: &str) -> Failure {
    crash_kind("crash", message, output)
}

fn crash_kind(kind: &'static str, message: String, output: &str) -> Failure {
    Failure {
        kind,
        place: None,
        message,
        values: Vec::new(),
        output: output.into(),
    }
}

/// `msg (file:line:col)` of a panic line.
fn parse_panic(s: &str) -> Option<(String, u32, u32)> {
    let (msg, at) = s.strip_suffix(')')?.rsplit_once(" (")?;
    let mut parts = at.rsplitn(3, ':');
    let col = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    Some((msg.to_string(), line, col))
}

/// Byte offset of a line and column (in characters, from 1).
fn offset(src: &[u8], line: u32, col: u32) -> u32 {
    let text = String::from_utf8_lossy(src);
    let mut at = 0;
    for (n, l) in text.split_inclusive('\n').enumerate() {
        if n + 1 == line as usize {
            let chars: usize = l.chars().take(col as usize - 1).map(char::len_utf8).sum();
            return (at + chars) as u32;
        }
        at += l.len();
    }
    at as u32
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(c) => out.push(c),
            None => {}
        }
    }
    out
}

/// A report line of the test binary: `kind line col offset payload…`.
fn record(rec: &str, output: &str) -> Failure {
    let f: Vec<&str> = rec.split('\t').collect();
    let num = |i: usize| f.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let payload = |i: usize| f.get(i).map(|s| unescape(s)).unwrap_or_default();
    let (kind, place) = (
        if f[0] == "try" { "try" } else { "expect" },
        (num(1), num(2), num(3)),
    );
    let (message, values) = if kind == "try" {
        (format!("try failed: {}", payload(4)), Vec::new())
    } else {
        let values = (f.iter().skip(5).collect::<Vec<_>>())
            .chunks(2)
            .map(|p| {
                (
                    unescape(p[0]),
                    p.get(1).map(|v| unescape(v)).unwrap_or_default(),
                )
            })
            .collect();
        (format!("expect failed: {}", payload(4)), values)
    };
    Failure {
        kind,
        place: Some(place),
        message,
        values,
        output: output.into(),
    }
}
