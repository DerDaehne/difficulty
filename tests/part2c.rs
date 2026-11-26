//! Part 2c in frontend and checker: types, literals, index, conversions, numbers per
//! width, builtin methods, `use std.…`, loops over collections and results (R2, R5, R6,
//! R8, R11, R14, R16, R28–R31, §4). One rule, one valid and one failing case each.

use dif::check_source;

/// The source wrapped in `void main() { … }`, each line indented by 4.
fn in_main(body: &str) -> String {
    let lines: Vec<String> = body.lines().map(|l| format!("    {l}")).collect();
    format!("void main() {{\n{}\n}}\n", lines.join("\n"))
}

/// All diagnostics as `line:col code message`, hint after ` | `.
fn diags(src: &str) -> Vec<String> {
    check_source(src)
        .iter()
        .map(|d| {
            let hint = d
                .hint
                .as_ref()
                .map(|h| format!(" | {h}"))
                .unwrap_or_default();
            format!("{}:{} {} {}{hint}", d.pos.line, d.pos.col, d.code, d.msg)
        })
        .collect()
}

fn ok(src: &str) {
    assert_eq!(diags(src), Vec::<String>::new(), "{src}");
}

/// `body` in main fails with exactly this diagnostic (line 1 of the body is line 2).
fn bad(body: &str, want: &str) {
    assert_eq!(diags(&in_main(body)), [want], "{body}");
}

#[test]
fn types_of_part_2c() {
    ok(&in_main("[]int a = []\n[3]int b = [1, 2, 3]\nmap[str]int c = {}\n[]?int d = [1, none]\n?[]int e = none\n[](int!str) f = []\n[2][2]u8 g = [[1, 2], [3, 4]]\nmap[str][]rune h = {}\nprint(\"{a}{b}{c}{d}{e}{f}{g}{h}\")"));
    ok("const int N = 4\nstruct G {\n    [N]i8 cells = [0, 0, 0, 0]\n}\n\nvoid main() {\n    print(G{})\n}\n");
    ok(&in_main("i8 a = -128\ni16 b = 1\ni32 c = 2\nu8 d = 255\nu16 e = 3\nu32 f = 4\nu64 g = 18446744073709551615\nf64 h = 1\nrune i = 'x'\nprint(\"{a}{b}{c}{d}{e}{f}{g}{h}{i}\")"));
    // a type cannot be written with a second `?` or `!E`
    assert_eq!(diags("[]??int f() {\n    return []\n}\n").len(), 1);
    // `[]int!E` is a list or an error, `[](int!E)` a list of results
    ok("[]int!str f() {\n    return [1]\n}\n\nvoid main() {\n    _ = f()\n}\n");
    assert_eq!(
        diags("void f([0]int a) {\n    print(a)\n}\n"),
        ["1:8 R5 the length of an array is a constant of at least 1"]
    );
    assert_eq!(
        diags("void f(f32 a) {\n    print(a)\n}\n"),
        ["1:8 S1 `f32` is not available in v0 | use f64"]
    );
    // a type containing itself through an array, not through a list or map
    assert_eq!(
        diags("struct Node {\n    [2]Node kids\n}\n"),
        ["1:8 R5 `Node` contains itself | a value has a fixed size; keep linked values in a list"]
    );
    ok("struct Node {\n    []Node kids = []\n    map[str]Node named = {}\n}\n");
}

#[test]
fn literals() {
    ok(&in_main("rune a = 'a'\nrune b = '\\n'\nrune c = '\\''\nrune d = '\\u{1F600}'\nrune e = 'é'\nrune f = '\"'\nprint(\"{a}{b}{c}{d}{e}{f}\")"));
    ok(&in_main("f64 a = 1.5\nf64 b = 1e3\nf64 c = 0.0\nf64 d = 5\nf64 e = 1_000.5e-2\nprint(\"{a}{b}{c}{d}{e}\")"));
    assert_eq!(diags(&in_main("rune a = ''")), ["2:14 L9 a rune literal holds exactly one character | write a character between the quotes: 'a'"]);
    assert_eq!(diags(&in_main("rune a = 'ab'")), ["2:14 L9 a rune literal holds exactly one character | use a string for more than one character: \"ab\""]);
    assert_eq!(
        diags(&in_main("rune a = 'a")),
        ["2:14 L9 rune literal is not closed | add ' at the end"]
    );
    // a letter with an accent as a second code point is two characters
    assert_eq!(
        diags(&in_main("rune a = 'e\u{301}'"))[0][..4].to_string(),
        "2:14"
    );
    assert_eq!(
        diags(&in_main("rune a = '\\u{D800}'")),
        ["2:15 L6 not a Unicode character"]
    );
    assert_eq!(
        diags(&in_main("str a = \"\\u{110000}\"")),
        ["2:14 L6 not a Unicode character"]
    );
    assert_eq!(diags(&in_main("print(5.abs())")), ["2:11 L8 a method on a number literal needs parentheses | put the number in parentheses: (5).abs()"]);
    bad(
        "f64 a = 1e400",
        "2:13 R8 expected f64, found 1e400 | the value does not fit in f64",
    );
    bad(
        "f64 a = 1e-400",
        "2:13 R8 expected f64, found 1e-400 | the value does not fit in f64",
    );
    bad("int a = 2.0", "2:13 R8 expected int, found f64 | int(2.0)");
    bad("rune a = 97", "2:14 R8 expected rune, found int | rune(97)");
    bad(
        "u8 a = 256",
        "2:12 R8 expected u8, found 256 | the value does not fit in u8",
    );
    bad(
        "u8 a = -1",
        "2:12 R8 expected u8, found -1 | the value does not fit in u8",
    );
    bad(
        "i8 a = 128",
        "2:12 R8 expected i8, found 128 | the value does not fit in i8",
    );
    ok(&in_main(
        "i8 a = -128\nu8 b = 100 + 155\nf64 c = 7 / 2\nprint(\"{a}{b}{c}\")",
    ));
    bad(
        "u8 a = 100 + 156",
        "2:12 R8 expected u8, found 256 | the value does not fit in u8",
    );
}

#[test]
fn format_specifications() {
    ok(&in_main(
        "f64 x = 2.5\nprint(\"{x:g} {x:.2f} {x:.0f} {x:.20f}\")",
    ));
    assert_eq!(
        diags(&in_main("f64 x = 2.5\nprint(\"{x:q}\")")),
        ["3:15 S1 unknown format `q` | formats are g and .Nf, e.g. {x:.2f}"]
    );
    assert_eq!(
        diags(&in_main("f64 x = 2.5\nprint(\"{x:.f}\")")),
        ["3:15 S1 unknown format `.f` | formats are g and .Nf, e.g. {x:.2f}"]
    );
    assert_eq!(
        diags(&in_main("f64 x = 2.5\nprint(\"{x:5}\")")),
        ["3:15 S1 unknown format `5` | formats are g and .Nf, e.g. {x:.2f}"]
    );
    assert_eq!(
        diags(&in_main("f64 x = 2.5\nprint(\"{x:.21f}\")")),
        ["3:15 S1 precision at most 20"]
    );
    bad(
        "int n = 1\nprint(\"{n:g}\")",
        "3:13 R8 the format g needs f64, found int | f64(n)",
    );
}

#[test]
fn numbers_per_width() {
    bad(
        "int a = 1\ni32 b = 2\nprint(a + b)",
        "4:13 R8 cannot apply `+` to int and i32 | a + int(b)",
    );
    bad(
        "u8 a = 1\nu16 b = 2\nprint(a < b)",
        "4:13 R8 cannot apply `<` to u8 and u16 | a < u8(b)",
    );
    bad(
        "int n = 3\nf64 x = n",
        "3:13 R8 expected f64, found int | f64(n)",
    );
    bad(
        "u8 a = 1\nint n = a",
        "3:13 R8 expected int, found u8 | int(a)",
    );
    bad(
        "u8 a = 1\nprint(-a)",
        "3:11 R8 cannot apply `-` to u8 | use 0 -% a for wrapping, or a signed type",
    );
    bad(
        "f64 a = 1.5\nprint(a & a)",
        "3:13 R8 cannot apply `&` to f64 and f64 | use ints for bit operations",
    );
    bad(
        "f64 a = 1.5\nprint(~a)",
        "3:11 R8 cannot apply `~` to f64 | use ints for bit operations",
    );
    bad(
        "print(true < false)",
        "2:16 R8 cannot apply `<` to bool and bool",
    );
    bad(
        "rune r = 'a'\nprint(r + 1)",
        "3:13 R8 cannot apply `+` to rune and int | rune(int(r) + 1)",
    );
    bad(
        "str s = \"a\"\nrune r = 'a'\nprint(s + r)",
        "4:13 R8 cannot apply `+` to str and rune | build text with interpolation: \"{s}{r}\"",
    );
    ok(&in_main("u8 a = 200\nu8 b = a +% 100\nu8 c = a + 5\nf64 x = 1.5\nf64 y = x % 2 + -x\ni8 d = -128\nprint(\"{b}{c}{y}{d}\")\nprint(a >> 1)\nprint(~a)\nprint('a' < 'b')"));
    // constants: a certain abort is R14, the exact result must fit
    bad("print(1 / 0)", "2:13 R14 division by zero");
    ok(&in_main("u8 a = 1\nprint(a / 0)")); // aborts at run time
    ok(&in_main("u8 a = 255\nprint(a + 1)")); // aborts at run time
    bad("print(1 << 64)", "2:13 R14 shift by 64 out of range 0..63");
    ok(&in_main("u8 a = 1\nprint(a << 8)")); // not constant: aborts at run time
    ok(&in_main(
        "print(1.0 / 0.0)\nprint(0.1 + 0.2)\nprint(1e300 * 1e300)",
    ));
    bad(
        "print(1 + 2.5 + 1 << 2)",
        "2:23 R8 cannot apply `<<` to f64 and f64 | use ints for bit operations",
    );
}

#[test]
fn conversions() {
    ok(&in_main("int n = 300\nprint(n.to_u8())\nprint(f64(n) / 8)\nprint(i8(-128))\nrune r = 'A'\nprint(rune(int(r) + 1))\nprint(str(r))\nprint(int('é'))\nprint(u8(n))\nprint(int(2.0))\nprint(f64(1))"));
    bad("print(int(2.5))", "2:11 R14 2.5 is not an exact int | round explicitly first with round(), trunc(), floor() or ceil()");
    bad("print(int(1.0 / 0.0))", "2:11 R14 inf is not an exact int | round explicitly first with round(), trunc(), floor() or ceil()");
    bad("print(int(1e30))", "2:11 R14 1e30 does not fit in int | round explicitly first with round(), trunc(), floor() or ceil()");
    bad("print(u8(300))", "2:11 R14 300 does not fit in u8");
    bad(
        "print(u8('\\u{1F600}'))",
        "2:11 R14 128512 does not fit in u8",
    );
    bad("print(rune(55296))", "2:11 R14 55296 is not a valid rune");
    bad(
        "print(f64(9007199254740993))",
        "2:11 R14 9007199254740993 is not an exact f64",
    );
    bad(
        "int n = int(\"12\")",
        "2:13 R31 cannot convert str to int | use \"12\".parse_int()",
    );
    bad(
        "str s = \"1.5\"\nprint(f64(s))",
        "3:11 R31 cannot convert str to f64 | use s.parse_f64()",
    );
    bad(
        "int n = 1\nprint(str(n))",
        "3:11 R31 cannot convert int to str | build text with interpolation: \"{n}\"",
    );
    bad(
        "int n = 1\nprint(bool(n))",
        "3:11 R31 cannot convert int to bool | compare explicitly: n != 0",
    );
    bad(
        "bool b = true\nprint(int(b))",
        "3:11 R31 cannot convert bool to int | use if b { 1 } else { 0 }",
    );
    assert_eq!(
        diags("struct P {\n    int x\n}\n\nvoid main() {\n    print(P(1))\n}\n"),
        ["6:11 R31 cannot convert int to P"]
    );
    bad(
        "print(int(1, 2))",
        "2:11 R8 `int` takes 1 argument, found 2",
    );
}

#[test]
fn index_and_places() {
    ok(&in_main("[]int xs = [1, 2, 3]\nxs[0] = 5\nxs[1] += 2\n[2][2]int g = [[1, 2], [3, 4]]\ng[1][0] = 7\nprint(\"{xs[2]}{g}\")"));
    bad(
        "[3]int a = [1, 2, 3]\nprint(a[3])",
        "3:12 R14 index 3 out of bounds for length 3",
    );
    bad(
        "[3]int a = [1, 2, 3]\nprint(a[-1])",
        "3:12 R14 index -1 out of bounds for length 3",
    );
    ok(&in_main(
        "[]int a = [1, 2, 3]\nint i = 5\nprint(a[3])\nprint(a[i])",
    )); // aborts at run time
    bad(
        "[]int a = [1]\nu8 i = 0\nprint(a[i])",
        "4:13 R8 expected int, found u8 | int(i)",
    );
    bad(
        "str s = \"x\"\nprint(s[0])",
        "3:11 R8 cannot index str | use s.slice(a, b) or s.runes()",
    );
    bad("int n = 1\nprint(n[0])", "3:11 R8 cannot index int");
    bad(
        "map[str]int m = {}\nprint(m[1])",
        "3:13 R8 expected str, found int",
    );
    // the element of a map is no place
    bad(
        "map[str]int m = {}\nm[\"a\"] += 1",
        "3:12 R8 cannot apply `+=` to ?int and int | m[\"a\"] = (m[\"a\"] ?? 0) + 1",
    );
    let s = "struct P {\n    int x\n}\n\nvoid main() {\n    map[str]P m = {\"a\": P{x: 1}}\n    m[\"a\"].x = 2\n    print(m)\n}\n";
    assert_eq!(diags(s), ["7:5 R30 a map entry is changed by assigning it | read it, change the copy, and store it: m[k] = v"]);
    bad("map[str][]int m = {}\nm[\"a\"].push(1)", "3:5 R30 a map entry is changed by assigning it | read it, change the copy, and store it: m[k] = v");
    ok(&in_main(
        "map[str][]int m = {}\nm[\"a\"] = [1]\nprint(m[\"a\"] ?? [])",
    ));
}

#[test]
fn element_places_and_overlap() {
    let swap = "void swap(mut int a, mut int b) {\n    int t = a\n    a = b\n    b = t\n}\n\n";
    ok(&format!("{swap}void main() {{\n    []int xs = [1, 2]\n    int i = 0\n    swap(mut xs[0], mut xs[1])\n    swap(mut xs[i], mut xs[1])\n    print(xs)\n}}\n"));
    assert_eq!(
        diags(&format!("{swap}void main() {{\n    []int xs = [1, 2]\n    swap(mut xs[0], mut xs[0])\n    print(xs)\n}}\n")),
        ["9:21 R6 `xs` goes into this call changeable twice | pass it changeable only once"]
    );
    assert_eq!(
        diags(&format!("{swap}void main() {{\n    [][]int xs = [[1], [2]]\n    swap(mut xs[0][0], mut xs[0][0])\n    print(xs)\n}}\n")).len(),
        1
    );
    // a change to an element is a change to the list (R6): it must be read afterwards
    bad(
        "[]int xs = [1, 2]\nxs[0] = 1",
        "3:5 R6 `xs` is changed but never read afterwards | use the value, or remove the change",
    );
    bad(
        "[]int xs = [1, 2]\nxs.push(3)",
        "3:5 R6 `xs` is changed but never read afterwards | use the value, or remove the change",
    );
    assert_eq!(
        diags("void f([]int xs) {\n    xs.push(1)\n    print(xs)\n}\n\nvoid g([]int xs) {\n    xs[0] = 1\n}\n"),
        ["7:5 R6 the change to `xs` is lost: `xs` is a copy | take it as mut: void g(mut []int xs)"]
    );
    ok("void g(mut []int xs) {\n    xs[0] = 1\n}\n");
}

#[test]
fn list_and_map_literals() {
    ok(&in_main(
        "[]u8 b = [1, 2]\nu8 x = 7\nprint(\"{b}{[x, 1]}{[1, x]}{[1, 2]}\")",
    ));
    // the type of a literal without place does not depend on the order
    ok(&in_main("print([1, 2.5])\nprint([2.5, 1])"));
    bad(
        "int x = 1\nprint([x, 2.5])",
        "3:15 R8 expected int, found f64 | int(2.5)",
    );
    bad(
        "int x = 1\nprint([2.5, x])",
        "3:12 R8 expected int, found f64 | int(2.5)",
    );
    bad("print([1, \"a\"])", "2:12 R8 expected str, found int");
    bad("print([])", "2:11 R8 `[]` needs a place of type []T");
    bad("print({})", "2:11 R8 `{}` needs a place of type map[K]V");
    bad(
        "[3]int a = [1, 2]",
        "2:16 R8 [3]int needs 3 elements, found 2 | use []int for a list that can have any length",
    );
    bad("[]int a = [1, \"x\"]", "2:19 R8 expected int, found str");
    bad(
        "map[str]int m = {\"a\": 1, \"b\": \"c\"}",
        "2:35 R8 expected int, found str",
    );
    bad(
        "map[str]int m = {\"a\": 1, \"b\": 2, \"a\": 3}",
        "2:38 R30 duplicate key \"a\" | first used at main.dif:2:22",
    );
    ok(&in_main("map[str]int m = {\"a\": 1, \"b\": 2}\nmap[int]str n = {1: \"a\", 2: \"b\"}\nprint(\"{m}{n}\")"));
    ok(&in_main("map[str]int m = {\n    \"a\": 1,\n    \"b\": 2,\n}\n[]int xs = [\n    1,\n    2,\n]\nprint(\"{m}{xs}\")"));
    ok(&in_main(
        "print(repeat(0, 3))\n[]u8 b = repeat(0, 2)\nprint(b)",
    ));
    ok("struct S {\n    []int a = []\n    map[str]int m = {}\n}\n");
}

#[test]
fn builtin_methods() {
    ok(include_str!("check/part2c.dif"));
    assert_eq!(
        diags(&in_main("[]int xs = [1]\nxs.pushh(2)")),
        ["3:5 R4 `[]int` has no method `pushh` | did you mean push?"]
    );
    assert_eq!(
        diags(&in_main("str s = \"a\"\nprint(s.trimm())")),
        ["3:11 R4 `str` has no method `trimm` | did you mean trim?"]
    );
    assert_eq!(
        diags(&in_main("map[str]int m = {}\nprint(m.hass(\"a\"))")),
        ["3:11 R4 `map[str]int` has no method `hass` | did you mean has?"]
    );
    assert_eq!(
        diags(&in_main("f64 x = 1.5\nprint(x.rond())")),
        ["3:11 R4 `f64` has no method `rond` | did you mean round?"]
    );
    assert_eq!(
        diags(&in_main("u8 x = 1\nprint(x.abs())")),
        ["3:11 R4 `u8` has no method `abs`"]
    );
    assert_eq!(
        diags(&in_main("[3]int a = [1, 2, 3]\na.push(4)")),
        ["3:5 R4 `[3]int` has no method `push` | use a list: []int"]
    );
    assert_eq!(
        diags(&in_main("[]int a = [1]\nprint(a.join(\",\"))")),
        ["3:11 R4 `[]int` has no method `join`"]
    );
    bad(
        "[]int xs = [1]\nxs.push(\"s\")",
        "3:13 R8 expected int, found str",
    );
    bad(
        "[]int xs = [1]\nxs.push(1, 2)",
        "3:5 R8 `push` takes 1 argument, found 2",
    );
    bad(
        "[]int xs = [1]\nprint(xs.slice(0))",
        "3:11 R8 `slice` takes 2 arguments, found 1",
    );
    // sort needs an ordered element type, at the name of the method
    let s = "struct P {\n    int x\n}\n\nvoid main() {\n    []P ps = [P{x: 2}, P{x: 1}]\n    ps.sort()\n    print(ps)\n}\n";
    assert_eq!(diags(s), ["7:8 R28 `sort` needs an ordered element type, found P | only numbers, rune and str can be sorted"]);
    // methods that change self need a place (R7) that is not a const (R4)
    assert_eq!(diags("[]int make() {\n    return [1, 2]\n}\n\nvoid main() {\n    make().push(3)\n}\n"), ["6:5 R7 `[]int.push` changes self and needs a variable | store the value in a variable first"]);
    bad(
        "const []int xs = [1]\nxs.push(2)",
        "3:5 R4 cannot change const `xs` | `[]int.push` changes self",
    );
    bad("[]int xs = [1]\nxs.pop()", "3:5 R13 result of `xs.pop()` is never used | use it with try, ?? or switch, or discard it explicitly with `_ = xs.pop()`");
    ok(&in_main(
        "[]int xs = [1]\nint n = xs.remove(0)\nprint(n)\nxs.push(n)\nprint(xs)",
    ));
    bad("[]int xs = [1]\nprint(xs)\nxs.len()", "4:5 R13 result of `xs.len()` is never used | use it, or discard it explicitly with `_ = xs.len()`");
}

#[test]
fn std_modules() {
    ok(include_str!("check/part2c.dif"));
    assert_eq!(
        diags("void main() {\n    print(1)\n}\n\nuse std.io\n"),
        ["5:1 R16 `use` comes before all other declarations | move it to the top of the file"]
    );
    assert_eq!(
        diags("use std.io\nuse std.io\n\nvoid main() {\n    print(1)\n}\n"),
        ["2:1 R16 `std.io` is already used | remove this line"]
    );
    assert_eq!(
        diags("use std.json\n"),
        ["1:9 R16 unknown standard module `json` | available: io, os"]
    );
    assert_eq!(
        diags("void main() {\n    for str line in io.lines() {\n        print(line)\n    }\n}\n"),
        ["2:21 R16 `io` is not imported | add use std.io at the top of the file"]
    );
    assert_eq!(
        diags("void main() {\n    print(os.args().len())\n}\n"),
        ["2:11 R16 `os` is not imported | add use std.os at the top of the file"]
    );
    assert_eq!(
        diags("use std.io\n\nvoid main() {\n    int io = 1\n    print(io)\n}\n"),
        ["4:9 R4 `io` is a predefined name | choose another name"]
    );
    assert_eq!(
        diags("void main() {\n    int io = 1\n    print(io)\n}\n"),
        Vec::<String>::new()
    );
    assert_eq!(
        diags("use std.io\n\nvoid main() {\n    print(io.nothing())\n}\n"),
        ["4:11 R4 `io` has no function `nothing`"]
    );
    assert_eq!(
        diags("use std.io\n\nvoid main() {\n    print(io.read_fil(\"a\"))\n}\n"),
        ["4:11 R4 `io` has no function `read_fil` | did you mean read_file?"]
    );
    // reading is a stream: `io.lines()` only as the source of a loop, results need `try`
    assert_eq!(diags("use std.io\n\nvoid main() {\n    print(io.lines())\n}\n"), ["4:11 R11 `io.lines()` can only be the source of a for loop | for str line in try io.lines() { … }"]);
    assert_eq!(
        diags("use std.io\n\nvoid!io.IoErr main() {\n    for str line in io.lines() {\n        print(line)\n    }\n}\n"),
        ["4:13 R8 expected str, found str!io.IoErr | for str line in try io.lines()"]
    );
    assert_eq!(
        diags("use std.io\n\nvoid main() {\n    for str line in try io.lines() {\n        print(line)\n    }\n}\n"),
        ["4:21 R12 `try` needs a function that returns T!E or ?T | change the return type to void!io.IoErr, or use ?? or switch"]
    );
    assert_eq!(
        diags("use std.io\n\nvoid main() {\n    io.write_file(\"a\", \"b\")\n}\n").len(),
        1
    );
}

#[test]
fn loops_over_collections() {
    ok(&in_main("[]int xs = [1, 2]\nfor int x in xs {\n    print(x)\n}\nfor int i, int x in xs {\n    print(i + x)\n}\nmap[str]int m = {\"a\": 1}\nfor str k, int v in m {\n    print(\"{k}{v}\")\n}\n[2]str a = [\"a\", \"b\"]\nfor str s in a {\n    print(s)\n}"));
    bad(
        "int n = 3\nfor int x in n {\n    print(x)\n}",
        "3:18 R11 cannot loop over int | for int i in range(n)",
    );
    bad(
        "for rune r in \"abc\" {\n    print(r)\n}",
        "2:19 R11 cannot loop over str | for rune r in s.runes()",
    );
    bad(
        "for int x in true {\n    print(x)\n}",
        "2:18 R11 cannot loop over bool",
    );
    bad(
        "[]int xs = [1]\nfor str s in xs {\n    print(s)\n}",
        "3:13 R8 expected str, found int",
    );
    bad(
        "[]int xs = [1]\nfor int i, str x in xs {\n    print(i)\n}",
        "3:20 R8 expected str, found int",
    );
    bad(
        "map[str]int counts = {}\nfor str k in counts {\n    print(k)\n}",
        "3:18 R11 a loop over a map needs key and value | for str k, int v in counts",
    );
    bad(
        "map[str]int m = {}\nfor int i, int v in m {\n    print(i + v)\n}",
        "3:13 R8 expected int, found str",
    );
    // the loop works on a snapshot: pushing into the source is fine
    ok(&in_main(
        "[]int xs = [1]\nfor int x in xs {\n    xs.push(x)\n}\nprint(xs)",
    ));
}

#[test]
fn loops_over_results() {
    let head = "enum E { Bad }\n\n";
    ok(&format!("{head}int!E total([](int!E) rs) {{\n    int sum = 0\n    for int n in try rs {{\n        sum += n\n    }}\n    return sum\n}}\n"));
    ok(&format!("{head}enum F {{ Wrapped }}\n\nint!F total([](int!E) rs) {{\n    int sum = 0\n    for int n in try rs or F.Wrapped {{\n        sum += n\n    }}\n    return sum\n}}\n"));
    ok(&format!(
        "{head}void f([](int!E) rs) {{\n    for int!E r in rs {{\n        print(r)\n    }}\n}}\n"
    ));
    assert_eq!(
        diags(&format!("{head}void main() {{\n    [](int!E) rs = []\n    for int n in rs {{\n        print(n)\n    }}\n}}\n")),
        ["5:13 R8 expected int, found int!E | for int n in try rs"]
    );
    assert_eq!(
        diags(&format!("{head}void main() {{\n    [](int!E) rs = []\n    for int n in try rs {{\n        print(n)\n    }}\n}}\n")),
        ["5:18 R12 `try` needs a function that returns T!E or ?T | change the return type to void!E, or use ?? or switch"]
    );
    assert_eq!(
        diags(&format!(
            "{head}void f([]int xs) {{\n    for int n in try xs {{\n        print(n)\n    }}\n}}\n"
        )),
        ["4:18 R12 `try` needs a result of type T!E or ?T, found int | remove try"]
    );
}

#[test]
fn switch_over_widths_and_runes() {
    ok(&in_main("u8 b = 3\nswitch b {\ncase 1, 2:\n    print(1)\ndefault:\n    print(0)\n}\nrune r = 'x'\nswitch r {\ncase 'a':\n    print(1)\ndefault:\n    print(0)\n}"));
    bad(
        "u8 b = 3\nswitch b {\ncase 300:\n    print(1)\ndefault:\n    print(0)\n}",
        "4:10 R8 case 300 does not fit in u8",
    );
    bad(
        "rune r = 'x'\nswitch r {\ncase 'a':\n    print(1)\n}",
        "3:5 R10 switch over rune needs default | default:",
    );
    bad(
        "rune r = 'x'\nswitch r {\ncase 'a', 'a':\n    print(1)\ndefault:\n    print(0)\n}",
        "4:15 R10 case 'a' is already handled at main.dif:4:10",
    );
    bad(
        "f64 x = 1.5\nswitch x {\ndefault:\n    print(1)\n}",
        "3:12 R10 cannot switch over f64",
    );
}

#[test]
fn results_of_optional_type() {
    // in a `?T!E` function `return none` and `return n` are both fine (R12)
    ok("enum E { Bad }\n\n?int!E f(int n) {\n    if n == 0 {\n        return none\n    }\n    if n == 1 {\n        return err(E.Bad)\n    }\n    if n == 2 {\n        return some(5)\n    }\n    return n\n}\n\nvoid main() {\n    print(f(1))\n}\n");
    ok("enum E { Bad }\n\n?int!E f() {\n    ?int x = none\n    return x\n}\n");
    assert_eq!(
        diags("enum E { Bad }\n\n?int!E f() {\n    return \"x\"\n}\n"),
        ["4:12 R8 expected ?int!E, found str"]
    );
}

#[test]
fn part_2c_programs_are_not_built_yet() {
    let c = dif::compile(
        "main.dif",
        b"void main() {\n    []int xs = [1]\n    print(xs)\n}\n",
    )
    .unwrap();
    assert!(c.starts_with("#error"), "{c}");
    // a program of parts 1 to 2b still compiles to real C
    let c = dif::compile("main.dif", b"void main() {\n    print(1)\n}\n").unwrap();
    assert!(c.contains("int main("), "{c}");
}

#[test]
fn f64_text_form() {
    // the checker prints constants like the runtime does (R29)
    for (src, want) in [
        ("int(1e30)", "1e30 does not fit in int"),
        ("int(0.5)", "0.5 is not an exact int"),
        (
            "int(100000000000000000000.5)",
            "100000000000000000000 does not fit in int",
        ),
        ("int(1.5e-7)", "1.5e-7 is not an exact int"),
        ("int(0.000001)", "0.000001 is not an exact int"),
        ("int(0.1 + 0.2)", "0.30000000000000004 is not an exact int"),
        ("u8(-0.5 * 4 - 1)", "-3 does not fit in u8"),
    ] {
        let d = diags(&in_main(&format!("print({src})")));
        assert!(d.len() == 1 && d[0].contains(want), "{src}: {d:?}");
    }
}

#[test]
fn pop_and_remove_are_changes_like_any_other() {
    bad("[]int xs = [1]\nprint(xs.pop())", "3:11 R6 `xs` is changed but never read afterwards | use xs.last() if the list is not needed afterwards");
    bad("[]int xs = [1]\nprint(xs.remove(0))", "3:11 R6 `xs` is changed but never read afterwards | use xs[0] if the list is not needed afterwards");
    ok(&in_main("[]int xs = [1]\nprint(xs.pop())\nprint(xs)"));
}
