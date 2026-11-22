use std::process::ExitCode;

const USAGE: &str = "usage: dif check [--json] <file.dif>\n       dif version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") => check(&args[1..]),
        Some("version" | "--version" | "-v") => {
            println!("dif {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn check(args: &[String]) -> ExitCode {
    let mut json = false;
    let mut files = Vec::new();
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            o if o.starts_with('-') => {
                eprintln!("dif: unknown option {o}\n{USAGE}");
                return ExitCode::from(2);
            }
            _ => files.push(a),
        }
    }
    let [file] = files.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("dif: cannot read {file}: {e}");
            return ExitCode::from(2);
        }
    };
    let diags = dif::check_bytes(file, &bytes);
    for d in &diags {
        if json {
            println!("{}", d.json(file));
        } else {
            eprintln!("{}", d.render(file));
        }
    }
    if diags.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
