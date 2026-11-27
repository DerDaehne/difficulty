//! Stable message ids and fixes of `dif check --json` (spec §0.3a, §0.5): every id of the
//! compiler is listed in the spec (and the other way round), has the form `<Code>.<name>`,
//! and applying a `fix` removes the message it belongs to.

use std::collections::BTreeSet;
use std::path::Path;

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn read(rel: &str) -> String {
    std::fs::read_to_string(Path::new(ROOT).join(rel)).unwrap()
}

/// `<Code>.<name>`: code letter and digits, then a lowercase name with digits and `_`.
fn well_formed(id: &str) -> bool {
    let Some((code, name)) = id.split_once('.') else {
        return false;
    };
    let mut c = code.chars();
    matches!(c.next(), Some('H' | 'L' | 'R' | 'S'))
        && code.len() > 1
        && c.all(|x| x.is_ascii_digit())
        && name.starts_with(|x: char| x.is_ascii_lowercase())
        && name
            .chars()
            .all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == '_')
}

/// Every string literal in the compiler sources that is shaped like an id.
fn ids_in_sources() -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for f in [
        "lexer",
        "parser",
        "checker",
        "checker/p2c",
        "checker/lit",
        "checker/generic",
        "diag",
    ] {
        let text = read(&format!("src/{f}.rs"));
        // a quoted word right after a quote: robust against escaped quotes elsewhere
        for (i, _) in text.match_indices('"') {
            if let Some(part) = text[i + 1..].split('"').next() {
                if well_formed(part) {
                    ids.insert(part.to_string());
                }
            }
        }
    }
    ids
}

/// The ids of the table in §0.5.
fn ids_in_spec() -> BTreeSet<String> {
    read("docs/spec.md")
        .lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .filter_map(|l| l.split('`').next())
        .filter(|id| well_formed(id))
        .map(String::from)
        .collect()
}

#[test]
fn ids_match_the_spec() {
    let (src, spec) = (ids_in_sources(), ids_in_spec());
    let missing: Vec<_> = src.difference(&spec).collect();
    let stale: Vec<_> = spec.difference(&src).collect();
    assert!(
        missing.is_empty(),
        "ids missing from spec §0.5: {missing:?}"
    );
    assert!(
        stale.is_empty(),
        "ids in spec §0.5 the compiler lacks: {stale:?}"
    );
    assert!(src.len() >= 100);
}

/// The sources of all error examples: the `dif error` blocks of the spec and tests/err.
fn error_sources() -> Vec<(String, String)> {
    let mut v = Vec::new();
    let spec = read("docs/spec.md");
    let lines: Vec<&str> = spec.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        i += 1;
        if !l.starts_with("```dif error") {
            continue;
        }
        let start = i;
        while lines[i] != "```" {
            i += 1;
        }
        v.push((
            format!("spec line {start}"),
            lines[start..i].join("\n") + "\n",
        ));
    }
    let mut files: Vec<_> = std::fs::read_dir(Path::new(ROOT).join("tests/err"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    for f in files {
        v.push((
            f.display().to_string(),
            std::fs::read_to_string(&f).unwrap(),
        ));
    }
    v
}

#[test]
fn every_diagnostic_has_a_listed_id_and_fixes_work() {
    let spec = ids_in_spec();
    let (mut n, mut fixes) = (0, BTreeSet::new());
    for (name, src) in error_sources() {
        for d in dif::check_source(&src) {
            n += 1;
            assert!(well_formed(d.id), "{name}: bad id {:?}", d.id);
            assert!(
                d.id.starts_with(&format!("{}.", d.code)),
                "{name}: {}",
                d.id
            );
            assert!(
                spec.contains(d.id),
                "{name}: id {} is not in the spec",
                d.id
            );
            let Some(f) = &d.fix else { continue };
            let mut fixed = src.clone();
            fixed.replace_range(f.offset as usize..f.end as usize, &f.text);
            let count = |s: &str| dif::check_source(s).iter().filter(|x| x.id == d.id).count();
            assert!(
                count(&fixed) < count(&src),
                "{name}: the fix of {} leaves the message:\n{fixed}",
                d.id
            );
            fixes.insert(d.id);
        }
    }
    assert!(n >= 40, "only {n} diagnostics");
    assert!(fixes.len() >= 5, "fixes seen: {fixes:?}");
}

/// Every fix the spec's table promises is produced by at least one of the examples below.
#[test]
fn fix_examples() {
    let cases: &[(&str, &str, &str)] = &[
        ("L1.invisible", "void main() {\n    int x\u{200B} = 1\n}\n", "void main() {\n    int x = 1\n}\n"),
        ("L3.semicolon", "void main() {\n    print(\"a\");\n}\n", "void main() {\n    print(\"a\")\n}\n"),
        ("L6.single_rbrace", "void main() {\n    print(\"a}\")\n}\n", "void main() {\n    print(\"a\\}\")\n}\n"),
        ("L7.incdec", "void main() {\n    int i = 0\n    i++\n    print(i)\n}\n", "void main() {\n    int i = 0\n    i += 1\n    print(i)\n}\n"),
        ("L8.prefix_case", "void main() {\n    print(0X1F)\n}\n", "void main() {\n    print(0x1f)\n}\n"),
        ("R2.decl_needs_type", "void main() {\n    var x = 5\n    print(x)\n}\n", "void main() {\n    int x = 5\n    print(x)\n}\n"),
        ("R3.body_next_line", "void main() {\n    if true\n    {\n        print(1)\n    }\n}\n", "void main() {\n    if true {\n        print(1)\n    }\n}\n"),
        ("R3.else_next_line", "void main() {\n    if true {\n        print(1)\n    }\n    else {\n        print(2)\n    }\n}\n", "void main() {\n    if true {\n        print(1)\n    } else {\n        print(2)\n    }\n}\n"),
        ("R3.elif", "void main() {\n    if true {\n        print(1)\n    } elif false {\n        print(2)\n    }\n}\n", "void main() {\n    if true {\n        print(1)\n    } else if false {\n        print(2)\n    }\n}\n"),
        ("R9.condition", "void main() {\n    int n = 3\n    if n {\n        print(n)\n    }\n}\n", "void main() {\n    int n = 3\n    if n != 0 {\n        print(n)\n    }\n}\n"),
        ("R6.mut_missing", "void bump(mut int n) {\n    n += 1\n}\n\nvoid main() {\n    int x = 0\n    bump(x)\n    print(x)\n}\n", "void bump(mut int n) {\n    n += 1\n}\n\nvoid main() {\n    int x = 0\n    bump(mut x)\n    print(x)\n}\n"),
        ("R6.mut_extra", "void show(int n) {\n    print(n)\n}\n\nvoid main() {\n    int x = 0\n    show(mut x)\n}\n", "void show(int n) {\n    print(n)\n}\n\nvoid main() {\n    int x = 0\n    show(x)\n}\n"),
        ("R6.mut_unneeded", "int sum(mut int a, int b) {\n    return a + b\n}\n", "int sum(int a, int b) {\n    return a + b\n}\n"),
        ("R18.expect_outside_test", "void main() {\n    expect(1 == 1)\n}\n", "void main() {\n    assert(1 == 1)\n}\n"),
        ("S1.struct_lit_header", "struct Point {\n    int x\n}\n\nvoid main() {\n    Point p = Point{x: 0}\n    if p == Point{x: 0} {\n        print(p.x)\n    }\n}\n", "struct Point {\n    int x\n}\n\nvoid main() {\n    Point p = Point{x: 0}\n    if p == (Point{x: 0}) {\n        print(p.x)\n    }\n}\n"),
        ("S1.assign_in_cond", "void main() {\n    int n = 3\n    if n = 3 {\n        print(n)\n    }\n}\n", "void main() {\n    int n = 3\n    if n == 3 {\n        print(n)\n    }\n}\n"),
    ];
    for (id, src, want) in cases {
        let d = dif::check_source(src)
            .into_iter()
            .find(|d| d.id == *id)
            .unwrap_or_else(|| panic!("{id}: no such diagnostic for\n{src}"));
        let f = d.fix.unwrap_or_else(|| panic!("{id}: no fix"));
        let mut fixed = src.to_string();
        fixed.replace_range(f.offset as usize..f.end as usize, &f.text);
        assert_eq!(&fixed, want, "{id}");
        assert_eq!(dif::check_source(&fixed), vec![], "{id}: {fixed}");
    }
}

#[test]
fn json_example_of_the_spec() {
    let spec = read("docs/spec.md");
    let line = spec
        .lines()
        .find(|l| l.starts_with("{\"v\":1,\"code\":\"R2\""))
        .unwrap();
    let src = "void main() {\n    var x = 5\n    print(x)\n}\n";
    let d = dif::check_source(src).remove(0);
    assert_eq!(d.json("main.dif"), line);
}

/// `i++` gets a fix only as a statement of its own, not inside an expression.
#[test]
fn incdec_fix_only_as_statement() {
    let fix_of = |body: &str| {
        let src = format!("void main() {{\n    int i = 0\n{body}\n    print(i)\n}}\n");
        let ds: Vec<_> = dif::check_source(&src)
            .into_iter()
            .filter(|d| d.id == "L7.incdec")
            .collect();
        assert_eq!(ds.len(), 1, "{body}");
        ds[0].fix.is_some()
    };
    assert!(fix_of("    i++"));
    assert!(fix_of("    --i"));
    assert!(!fix_of("    int j = i++"));
    assert!(!fix_of("    print(i++)"));
    assert!(!fix_of("    int j = --i"));
}

/// Every diagnostic call in the sources names its id (`"R15.name"`), not a bare code; found by
/// reading the source, so branches no test reaches are covered too.
#[test]
fn no_bare_codes_in_sources() {
    let mut files = vec![];
    for dir in ["src", "src/checker"] {
        for e in std::fs::read_dir(Path::new(ROOT).join(dir)).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    let mut bare = vec![];
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        for call in ["err(", "Diag::new("] {
            for (i, _) in text.match_indices(call) {
                let rest = text[i + call.len()..].trim_start();
                if let Some(lit) = rest.strip_prefix('"') {
                    let lit = lit.split('"').next().unwrap_or("");
                    if !well_formed(lit) {
                        bare.push(format!("{}: {lit}", f.display()));
                    }
                }
            }
        }
    }
    assert!(bare.is_empty(), "diagnostics without an id: {bare:?}");
}

/// A fix never loses comments or changes what the program does (R25).
#[test]
fn named_argument_fixes_are_safe() {
    let ids = |src: &str| {
        let d = dif::check_source(src);
        d.iter()
            .map(|d| (d.id, d.fix.is_some()))
            .collect::<Vec<_>>()
    };
    let head = "int f(int a, int b = 2, int d = 3) {\n    return a + b + d\n}\n";
    let call = |c: &str| format!("{head}void main() {{\n    print({c})\n}}\n");
    // comments and line breaks between the arguments: no fix
    let multi = call("f(\n        1, // one\n        d: 7, // seven\n        b: 5 // five\n    )");
    assert_eq!(ids(&multi), [("R25.name_order", false)]);
    assert_eq!(ids(&call("f(1, d: 7, b: 5)")), [("R25.name_order", true)]);
    // already given by position: no fix that would move the value
    assert_eq!(ids(&call("f(1, a: 4)")), [("R25.name_twice", false)]);
    assert_eq!(ids(&call("f(a: 4)")), [("R25.name_without_default", true)]);
    assert_eq!(ids(&call("f(b: 4)")), [("R8.arg_count", false)]);
}
