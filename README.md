<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img src="assets/logo-light.svg" width="112" height="112" alt="difficulty: ein C, dessen offene Seite ein Strich schließt">
  </picture>
</p>

<h1 align="center">difficulty</h1>

<p align="center"><b>Schwierig ist nur der Name.</b><br>
Eine kompilierte Sprache, die sich schreibt wie ein angenehmeres C.</p>

difficulty sieht aus wie C: Typen stehen vorn, Blöcke in `{ }`, Programme werden über C11
zu kleinen nativen Binaries. Die Fallen von C fehlen. Es gibt keine Zeiger, sondern Werte, kein `null`, sondern `?T`. Fehler
stehen im Typ, Überlauf hält an, statt still weiterzurechnen. Was ein Programm verändert, steht
mit `mut` am Aufruf. Der Compiler meldet jeden Fehler genau einmal, mit Regel, Stelle und einem
Vorschlag, wie es richtig geht.

Das Logo ist ein C, dessen offene Seite ein Strich schließt. Zusammen ergeben sie ein d.

## Rundgang

### Vertraut auf den ersten Blick

Typ vor dem Namen, keine Semikolons, keine Klammern um Bedingungen, `{…}` im Text.

```dif
const int LIMIT = 10

int square(int n) {
    return n * n
}

void main() {
    int total = 0
    for int i in range(1, LIMIT) {
        if i % 2 == 0 {
            continue
        }
        total += square(i)
    }
    print("Summe der ungeraden Quadrate unter {LIMIT}: {total}")
}
```

### Werte statt Zeiger, Änderungen mit `mut`

Zuweisen kopiert. Eine Funktion ändert ein Argument nur, wenn sie es als `mut` nimmt und der
Aufruf `mut` schreibt. Methoden sind Funktionen an einem `struct`, Klassen gibt es nicht.

```dif
struct Point {
    int x
    int y = 0
}

void shift(mut Point p, int dx) {
    p.x += dx
}

int Point.sum() {
    return self.x + self.y
}

void main() {
    Point a = Point{x: 1}
    Point b = a
    shift(mut b, 5)
    print("{a} {b} {b.sum()}")
}
```

Ausgabe: `Point{x: 1, y: 0} Point{x: 6, y: 0} 6`.

Fehlt das `mut` am Aufruf, sagt der Compiler, was zu schreiben ist:

```dif error R6
struct Point {
    int x
}

void shift(mut Point p, int dx) {
    p.x += dx
}

void main() {
    Point b = Point{x: 1}
    shift(b, 5)
    print(b)
}
```

```
main.dif:11:11: error[R6]: `shift` takes `p` as mut
  hint: shift(mut b, 5)
```

### Kein `null`: was fehlen kann, steht im Typ

`?int` ist eine Zahl oder `none`. An den Wert kommt man nur, indem man den Fall behandelt.

```dif
?int half(int n) {
    if n % 2 != 0 {
        return none
    }
    return n / 2
}

void main() {
    print(half(8))
    print(half(7) ?? -1)
}
```

Ausgabe: `some(4)`, `-1`.

### Fehler stehen im Typ

`int!ParseErr` ist eine Zahl oder ein Fehler. `try` gibt den Fehler an den Aufrufer weiter,
`??` setzt einen Ersatzwert ein, `switch` unterscheidet beide Fälle. Ein Ergebnis, das niemand
ansieht, ist ein Compilerfehler.

```dif
enum ParseErr {
    Empty
    TooBig(int n)
}

int!ParseErr parse_amount(int raw) {
    if raw == 0 {
        return err(ParseErr.Empty)
    }
    if raw > 100 {
        return err(ParseErr.TooBig(raw))
    }
    return raw
}

int!ParseErr total(int a, int b) {
    return try parse_amount(a) + try parse_amount(b)
}

void main() {
    print(total(5, 7))
    print(total(5, 0))
    switch parse_amount(200) {
    case ok(int n):
        print("ok {n}")
    case err(ParseErr e):
        print("Fehler: {e}")
    }
}
```

Ausgabe: `ok(12)`, `err(ParseErr.Empty)`, `Fehler: ParseErr.TooBig(200)`.

### Jeder Fall bedacht

Ein `enum` darf Werte tragen. `switch` muss jede Variante abdecken, sonst übersetzt das
Programm nicht. Durchfallen gibt es nicht.

```dif
enum Shape {
    Square(int side)
    Rect(int w, int h)
    Dot
}

int area(Shape s) {
    switch s {
    case Shape.Square(int side):
        return side * side
    case Shape.Rect(int w, int h):
        return w * h
    case Shape.Dot:
        return 0
    }
}

void main() {
    print(area(Shape.Rect(3, 4)))
}
```

### Rechnen ohne Überraschung

Ein Überlauf hält das Programm an und nennt die Stelle. Wer modulo 2⁶⁴ rechnen will, schreibt
es hin: `+%`, `-%`, `*%`. Ausgewertet wird immer von links nach rechts.

```dif panic "integer overflow in +"
int big = 9223372036854775807
print(big +% 1)
print(big + 1)
```

```
-9223372036854775808
panic: integer overflow in + (main.dif:4:15)
```

### Der Compiler erklärt

Eine Meldung nennt die Regel (hier R13), die Stelle und eine Reparatur. Gewohnheiten aus
anderen Sprachen (`var x = 5`, `i++`, `elif`) bekommen einen Hinweis auf die Schreibweise von
difficulty. Mit `dif check --json` kommen die Meldungen maschinenlesbar.

```dif error R13
int square(int n) {
    return n * n
}

void main() {
    square(3)
}
```

```
main.dif:6:5: error[R13]: result of `square(3)` is never used
  hint: use it, or discard it explicitly with `_ = square(3)`
```

### Tests gehören zur Sprache

`test`-Blöcke stehen neben dem Code. `dif test` startet jeden Test in einem eigenen Prozess
und zeigt bei einem Fehlschlag die Werte der Teilausdrücke.

```dif test
int add(int a, int b) {
    return a + b
}

test "addition" {
    expect(add(2, 3) == 5)
}

test "wrong sum" {
    int a = 2
    expect(a + 2 == 5)
}
```

```
ok   main.dif:5 test "addition"
FAIL main.dif:9 test "wrong sum"
    main.dif:11:5: expect failed: a + 2 == 5
        a + 2 = 4
test result: 1 passed, 1 failed
```

### Klein und nativ

Ein Programm wird über C11 mit gcc oder clang übersetzt. Es braucht keine VM und keinen Garbage
Collector, „Hallo, Welt“ ist gestrippt 14 KB groß (dynamisch gegen die libc gelinkt). Speicher verwaltet difficulty über Werte,
Referenzzählung und Copy-on-Write ([Messungen](docs/measurements.md)).

```sh
dif check main.dif          # Syntax, Namen, Typen
dif run main.dif            # übersetzen und starten
dif build main.dif -o prog  # natives Programm
dif test                    # alle test-Blöcke unter .
```

## Stand

Früh, aber benutzbar. Gebaut sind die Teile 1 bis 2c der [Spezifikation](docs/spec.md):
`int`, `bool`, `str`, `struct`, `enum`, Methoden, `mut`, `switch`, `if` als Ausdruck, `?T`,
`T!E`, `try`, `??`, Tests, dazu Listen, Arrays und Maps als Werte (Copy-on-Write), Zahlen
von `i8` bis `u64` und `f64`, Text mit Unicode sowie Dateien und Standardeingabe. Als Nächstes
folgen Funktionsliterale mit `filter`/`map`/`sort_by` (Teil 2d), danach Generics und das direkte
Einbinden von C-Bibliotheken. Der Fahrplan steht in [PLAN.md](PLAN.md).

## Entwickeln

Der Compiler `dif` ist in Rust geschrieben und hat keine Abhängigkeiten.

```sh
nix develop          # rustc, cargo, clippy, rustfmt, gcc
cargo test
cargo run -- run examples/fib.dif
cargo run -- test examples/ledger.dif
```

## Editoren

Syntax-Hervorhebung, Einrückung (4 Leerzeichen) und `//`-Kommentare für `.dif`-Dateien.

Vim/Neovim: `editors/vim` in den Runtimepath legen, zum Beispiel

```sh
mkdir -p ~/.vim && cp -r editors/vim/* ~/.vim/        # Neovim: ~/.config/nvim/
```

Emacs: in der Konfiguration

```elisp
(add-to-list (quote load-path) "/pfad/zu/difficulty/editors/emacs")
(require (quote dif-mode))
```

## Für Werkzeuge und KI-Agenten

Weil es für jede Sache eine Schreibweise gibt und jede Meldung einen Reparaturvorschlag hat,
eignet sich difficulty auch für Werkzeuge. [llms.txt](llms.txt) beschreibt die Sprache auf
wenigen Seiten. `eval/` misst, wie gut Sprachmodelle damit Aufgaben lösen
([eval/README.md](eval/README.md)).

Lizenz: [MIT](LICENSE).
