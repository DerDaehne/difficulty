//! Part 2a: struct, enum, methods, mut, switch, if as a value (spec R5–R7, R10,
//! R11, R13, R15, R28). Each case is one mistake with exactly one diagnostic.

use dif::check_source;

const TYPES: &str = "struct Point {
    int x
    int y = 0
}

enum Shape {
    Square(int side)
    Rect(int w, int h)
    Dot,
}

enum Seat { Free, Taken }

void Point.move(mut self, int dx) {
    self.x += dx
}

int Point.sum() {
    return self.x + self.y
}

void shift(mut Point p, int dx) {
    p.x += dx
}
";

/// `TYPES`, then `body` inside `void main()`.
fn prog(body: &str) -> String {
    let lines: Vec<String> = body.lines().map(|l| format!("    {l}")).collect();
    format!("{TYPES}\nvoid main() {{\n{}\n}}\n", lines.join("\n"))
}

fn one(src: &str) -> (&'static str, String) {
    let ds = check_source(src);
    assert_eq!(
        ds.len(),
        1,
        "expected one diagnostic for\n{src}\ngot {ds:#?}"
    );
    (ds[0].code, ds[0].msg.clone())
}

fn clean(src: &str) {
    assert_eq!(check_source(src), vec![], "{src}");
}

#[test]
fn valid() {
    clean(&prog(
        "Point a = Point{x: 1}
Point b = Point{
    y: 2,
    x: 3,
}
a.move(2)
shift(mut b, 1,)
Shape s = Shape.Rect(2, 3)
int area = 0
switch s {
case Shape.Square(int side):
    area = side * side
case Shape.Rect(int w, _):
    area = w
case Shape.Dot:
    area = 0
}
str word = if area > 1 { \"groß\" } else if area == 1 { \"eins\" } else { \"null\" }
for int i in range(3) {
    switch i {
    case 0:
        continue
    default:
        print(i)
    }
}
print(\"{a} {b} {a == b} {s} {word} {a.sum()} {Seat.Free}\")",
    ));
    // a method with an effect may stand as a statement although it returns a value
    clean("int noisy() {\n    print(1)\n    return 1\n}\n\nvoid main() {\n    noisy()\n}\n");
    // a value of a struct literal in parentheses is fine in a header
    clean(&prog(
        "Point p = Point{x: 0}\nif p == (Point{x: 0}) {\n    print(p)\n}",
    ));
}

#[test]
fn mistakes() {
    let cases: &[(&str, &str, &str)] = &[
        // R5
        ("Point p = Point{x: 1, z: 2}\nprint(p)", "R4", "`Point` has no field `z`"),
        ("Point p = Point{x: 1, x: 2}\nprint(p)", "R4", "`x` is given twice"),
        ("Point p = Point{}\nprint(p)", "R5", "`Point` needs a value for `x`"),
        ("print(Seat.Fre)", "R4", "`Seat` has no variant `Fre`"),
        ("print(Shape.Rect)", "R8", "`Shape.Rect` needs its values"),
        ("print(Seat.Free(1))", "R8", "`Seat.Free` has no values"),
        ("Shape s = Shape.Dot\nprint(s.side)", "R8", "read with switch"),
        ("print(Point)", "R8", "`Point` is a type"),
        // R6
        ("Point p = Point{x: 1}\nshift(mut p.x, 1)\nprint(p)", "R8", "expected Point, found int"),
        ("shift(mut Point{x: 1}, 1)", "R6", "only a variable, field or element"),
        ("const Point P = Point{x: 1}\nshift(mut P, 1)", "R4", "cannot pass const `P` as mut"),
        ("Point p = Point{x: 1}\nprint(p.sum(mut 1))", "R8", "takes 0 arguments"),
        ("int x = 0\nx = 1\nx = 2\nprint(x)", "R6", "`x` is changed but never read afterwards"),
        ("Point p = Point{x: 1}\nshift(mut p, 1)", "R6", "`p` is changed but never read afterwards"),
        // R7
        ("const Point P = Point{x: 1}\nP.move(1)", "R4", "cannot change const `P`"),
        ("Point p = Point{x: 1}\nprint(p.summ())", "R4", "`Point` has no method `summ`"),
        ("print(self)", "R7", "`self` exists only in methods"),
        // R10
        ("Seat s = Seat.Free\nswitch s {\ncase Free:\n    print(1)\ndefault:\n    print(2)\n}", "R10", "variants are written with their type"),
        ("Seat s = Seat.Free\nswitch s {\ncase Seat.Free:\n    print(1)\ncase Seat.Taken:\n    print(2)\ndefault:\n    print(3)\n}", "R10", "default is never reached"),
        ("int n = 1\nswitch n {\ncase 1:\n    print(1)\n}", "R10", "switch over int needs default"),
        ("int n = 1\nswitch n {\ncase 1:\n    print(1)\ncase 1:\n    print(2)\ndefault:\n    print(3)\n}", "R10", "case 1 is already handled"),
        ("bool b = true\nswitch b {\ndefault:\n    print(1)\n}", "R10", "cannot switch over bool"),
        ("Shape s = Shape.Dot\nswitch s {\ncase Shape.Rect(_, _):\n    print(1)\ndefault:\n    print(2)\n}", "R10", "nothing is bound"),
        ("Shape s = Shape.Dot\nswitch s {\ncase Shape.Rect(int w):\n    print(w)\ndefault:\n    print(2)\n}", "R8", "has the values int w, int h"),
        ("Shape s = Shape.Dot\nswitch s {\ncase Seat.Free:\n    print(1)\ndefault:\n    print(2)\n}", "R8", "the switch is over Shape"),
        ("int n = 1\nswitch n {\ncase 0: print(0)\ndefault:\n    print(1)\n}", "R3", "the statements of a case start on the next line"),
        // R11
        ("for int i in range(3) {\n    switch i {\n    case 0:\n        break\n    default:\n        print(i)\n    }\n}", "R11", "`break` directly inside switch"),
        // R15
        ("int n = 1\nint m = if n > 0 { 1 }\nprint(m)", "R15", "if as a value needs else"),
        ("int n = 1\nint m = if n > 0 { 1 } else { \"a\" }\nprint(m)", "R8", "the branches of if give int and str"),
        ("int n = 1\nint m = if n > 0 {\n    n = 2\n} else {\n    3\n}\nprint(m)", "R15", "this branch of if gives no value"),
        // R3
        ("Point p = Point{x: 0}\nif p == Point{x: 0} {\n    print(p)\n}", "S1", "a struct literal in a header needs parentheses"),
    ];
    for (body, code, msg) in cases {
        let (c, m) = one(&prog(body));
        assert_eq!(c, *code, "{body}: {m}");
        assert!(m.contains(msg), "{body}: {m}");
    }
}

#[test]
fn declarations() {
    let cases: &[(&str, &str, &str)] = &[
        (
            "struct P { int x }\n",
            "R3",
            "fields of a struct stand one per line",
        ),
        (
            "enum Tree {\n    Leaf\n    Node(Pair p)\n}\n\nstruct Pair {\n    Tree l\n    Pair r\n}\n",
            "R5",
            "`Pair` contains itself",
        ),
        (
            "struct A {\n    B b\n}\n\nstruct B {\n    A a\n}\n",
            "R5",
            "`A` contains itself",
        ),
        (
            "void int.twice() {\n}\n",
            "R7",
            "is not a struct or enum of this module",
        ),
        (
            "struct P {\n    int x\n}\n\nint P.x() {\n    return 1\n}\n",
            "R4",
            "`x` is already declared",
        ),
        (
            "int noisy() {\n    print(1)\n    return 1\n}\n\nstruct P {\n    int x = noisy()\n}\n",
            "R5",
            "a field default uses only constants",
        ),
        (
            "int sum(mut int a, int b) {\n    return a + b\n}\n",
            "R6",
            "`a` is never changed; remove mut",
        ),
        (
            "void reset(int n) {\n    n = 0\n}\n",
            "R6",
            "the change to `n` is lost",
        ),
        (
            "struct P {\n    int x\n}\n\nint P.get(mut self) {\n    return self.x\n}\n",
            "R6",
            "`self` is never changed; remove mut",
        ),
        (
            "struct P {\n    int x\n}\n\nvoid P.move(int dx) {\n    self.x += dx\n}\n",
            "R6",
            "the change to `self.x` is lost: `self` is a copy",
        ),
        (
            "struct P {\n    int x\n}\n\nvoid P.move(int a, mut self) {\n    self.x += a\n}\n",
            "R7",
            "`mut self` is only the first parameter of a method",
        ),
        (
            "void f(mut self) {\n}\n",
            "R7",
            "`mut self` is only the first parameter of a method",
        ),
        // a change to a part of self that is overwritten before anyone reads it
        (
            "struct P {\n    int x\n}\n\nvoid P.r(mut self) {\n    self.x = 0\n    self = P{x: 1}\n}\n",
            "R6",
            "`self.x` is changed but never read afterwards",
        ),
    ];
    for (src, code, msg) in cases {
        let (c, m) = one(src);
        assert_eq!(c, *code, "{src}: {m}");
        assert!(m.contains(msg), "{src}: {m}");
    }
    // a method that changes self needs mut self; the call needs a variable (R7)
    let src = "struct C {\n    int n\n}\n\nvoid C.inc(mut self) {\n    self.n += 1\n}\n\nvoid C.twice(mut self) {\n    self.inc()\n    self.inc()\n}\n\nC fresh() {\n    return C{n: 0}\n}\n\nvoid main() {\n    fresh().twice()\n}\n";
    let (c, m) = one(src);
    assert_eq!(
        (c, m.as_str()),
        ("R7", "`C.twice` changes self and needs a variable")
    );
}

#[test]
fn method_on_void_call_says_no_value() {
    let (code, msg) = one(&prog("Point p = Point{x: 1, y: 2}\np.move(1).move(2)"));
    assert_eq!(code, "R8");
    assert!(msg.contains("returns nothing"), "{msg}");
}

/// An enum may contain itself in a variant payload, directly, in turns with another enum,
/// or through a struct field inside a payload (R5); a cycle of struct fields alone may not.
#[test]
fn recursive_enums_pass_r5() {
    let ok = |src: &str| assert!(check_source(src).is_empty(), "{src}");
    ok("enum Tree {\n    Leaf\n    Node(Tree l, Tree r)\n}\n");
    ok("enum A {\n    X(B b)\n    Y\n}\n\nenum B {\n    P(A a)\n}\n");
    ok("struct S {\n    E e\n}\n\nenum E {\n    A(S s)\n    B\n}\n");
    ok("enum E {\n    A([2]E xs)\n    B(?E e)\n    C\n}\n");
    let (c, m) = one("struct S {\n    E e\n    S s\n}\n\nenum E {\n    A\n}\n");
    assert_eq!((c, m.as_str()), ("R5", "`S` contains itself"));
}
