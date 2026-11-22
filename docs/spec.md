# difficulty — Spezifikation

Stand: 2026-11-22 · Version: v0, Teil 1 („Hallo, nativ“) · Begründungen und Fahrplan: [PLAN.md](../PLAN.md)

Diese Spezifikation ist nach den Regeln R1–R31 gegliedert. Ausgearbeitet sind die Regeln, die der erste Schritt „Hallo, nativ“ braucht; die übrigen stehen mit ihrem Wortlaut da und werden mit den nächsten Schritten ausgearbeitet.

## 0. Wie diese Spezifikation zu lesen ist

**Grammatik** steht in EBNF: `a b` Folge, `a | b` Auswahl, `[ a ]` optional, `{ a }` beliebig oft, `"x"` wörtlich, `NL` Zeilenende (siehe §1.3).

**Fehlercodes.** Ein Verstoß gegen eine Regel trägt deren Nummer: `error[R13]`. Reine Schreibfehler, die keine Regel betreffen, tragen `L` (Lexik, §1) oder `S` (Grammatik, §2) mit einer Nummer. Jede Meldung nennt Datei, Zeile und Spalte, eine Ursache und, wo möglich, einen Hinweis, der den Fehler behebt. Meldungen sind englisch (`dif … --lang de` für Deutsch).

```
fib.dif:4:5: error[R13]: result of `fib(n - 1)` is never used
  hint: use it, or discard it explicitly with `_ =`
```

**Beispiele** gibt es in drei Arten, die `dif test` als Doc-Tests ausführt:

- ` ```dif ` — ein vollständiges, gültiges Modul.
- ` ```dif snippet ` — Anweisungen, die in `void main()` eingebettet werden.
- ` ```dif error R4 ` — ein Modul, das genau diese Diagnose liefern muss.

**Abbruch (Panic).** Programmierfehler (R14) beenden das Programm sofort mit

```
panic: <message> (<file>:<line>:<column>)
```

auf stderr und Exit-Code 2. Ein Fehler, der aus `void!E main()` herauskommt, endet mit Exit-Code 1.

---

## 1. Lexik

### 1.1 Zeichen

Quelltext ist UTF-8. Ungültiges UTF-8 ist `L1`. Nicht-ASCII-Zeichen sind nur in Strings und Kommentaren erlaubt.

```ebnf
letter  = "a" … "z" | "A" … "Z" | "_" ;
digit   = "0" … "9" ;
IDENT   = letter { letter | digit } ;
```

Ein Bezeichner, der ein Schlüsselwort ist, ist kein `IDENT`. `_` allein ist der Platzhalter „verwerfen“ (`_ = f()`), kein Name.

### 1.2 Kommentare

`//` bis Zeilenende. `///` ist ein Doku-Kommentar und gehört zur folgenden Deklaration (`dif doc`, Doc-Tests). `/* … */` gibt es nicht: `L2`, Hinweis `use // for comments`.

### 1.3 Zeilenende und Fortsetzung

Eine Anweisung endet am Zeilenende (R3). Der Lexer erzeugt dafür ein Token `NL`, außer:

1. innerhalb von `( … )` und `[ … ]` — dort spielt das Zeilenende keine Rolle, bis wieder ein `{` geöffnet wird;
2. die Zeile endet mit einem binären Operator, mit `=` oder einem Zuweisungsoperator, oder mit `,`;
3. die nächste Zeile beginnt (nach Leerraum) mit `.`.

Leere Zeilen und Zeilen nur mit Kommentar erzeugen kein `NL`. Mehrere Anweisungen in einer Zeile gibt es nicht (`S1`); `;` ist `L3` mit Hinweis `statements end at the end of the line`.

### 1.4 Literale

```ebnf
INT     = dec | "0x" hexdigit { hexdigit | "_" } | "0b" ("0" | "1") { "0" | "1" | "_" } ;
dec     = digit { digit | "_" } ;
FLOAT   = dec "." dec [ exponent ] | dec exponent ;
exponent = ( "e" | "E" ) [ "+" | "-" ] dec ;
STRING  = '"' { char | escape | interp } '"' ;
escape  = "\" ( "n" | "t" | "r" | "\" | '"' | "{" | "}" | "0" ) | "\u{" hexdigit { hexdigit } "}" ;
interp  = "{" expr [ ":" format ] "}" ;
```

- `_` in Zahlen dient nur der Lesbarkeit: `1_000_000`. Ein Literal, das in keinen Ganzzahltyp passt, ist `L4`.
- Ein String reicht nicht über das Zeilenende hinaus (`L5`). Eine unbekannte Escape-Sequenz ist `L6`. `\{` und `\}` schreiben eine wörtliche Klammer.
- `{expr}` setzt den Wert von `expr` als Text ein; `expr` muss `text` sein (R28). Formatangaben `{x:.2f}` werden in Teil 2 festgelegt.
- `true` und `false` sind die Werte von `bool`.

### 1.5 Schlüsselwörter

```
const struct enum if else while for in break continue return switch case default
try mut use pub test extern unsafe void self defer
```

Nach v0 kommen `spawn parallel yield` dazu; sie sind schon jetzt reserviert. Nur in `use c`- und `extern`-Zeilen haben `c link pkg source impl load as strip release with send` eine Bedeutung. Literale und Formen: `true false none some ok err`. Eingebaute Funktionen: `print range repeat expect assert`.

Typnamen (`int`, `bool`, `str`, …) sind keine Schlüsselwörter, sondern vordefinierte Namen, die man nicht neu deklarieren darf (R4).

### 1.6 Operatoren und Satzzeichen

```
+  -  *  /  %  +%  -%  *%  &  |  ^  ~  <<  >>
== != <  <= >  >=  &&  ||  !  ??
=  += -= *= /= %=
(  )  [  ]  {  }  ,  .  :  ;(nur als Fehler L3)
```

`++` und `--` gibt es nicht: `L7`, Hinweis `use x += 1`.

---

## 2. Ausdrücke und Präzedenz

Von schwach nach stark bindend:

| Stufe | Operatoren | Assoziativität |
|---|---|---|
| 1 | `??` | rechts |
| 2 | `\|\|` | links |
| 3 | `&&` | links |
| 4 | `==` `!=` `<` `<=` `>` `>=` | keine |
| 5 | `\|` | links |
| 6 | `^` | links |
| 7 | `&` | links |
| 8 | `<<` `>>` | links |
| 9 | `+` `-` `+%` `-%` | links |
| 10 | `*` `/` `%` `*%` | links |
| 11 | Vorzeichen `-` `!` `~` `try` | rechts |
| 12 | Aufruf `f(…)`, Index `x[i]`, Feld/Methode `x.y` | links |

Drei Korrekturen gegenüber C, jeweils ein Fehler mit Hinweis statt stiller Bedeutung:

- **Bit-Operatoren binden stärker als Vergleiche** (Stufen 5–8 über 4): `x & 1 == 0` heißt `(x & 1) == 0`.
- **Keine Vergleichsketten:** `a < b < c` ist `S2`, Hinweis `write a < b && b < c`.
- **`&&` und `||` nie gemischt ohne Klammern:** `a && b || c` ist `S3`, Hinweis `add parentheses: (a && b) || c`.

`or` gehört zu `try` (`try x or E.V(…)`, Teil 2). Klammern um einen Ausdruck sind immer erlaubt; `dif fmt` entfernt sie um Bedingungen.

```ebnf
expr     = binary ;
binary   = unary { binop unary } ;          (* geordnet nach der Tabelle *)
unary    = { "-" | "!" | "~" | "try" } postfix ;
postfix  = primary { "(" [ args ] ")" | "[" expr "]" | "." IDENT } ;
args     = expr { "," expr } [ "," ] ;
primary  = INT | FLOAT | STRING | "true" | "false" | IDENT | "(" expr ")" ;
```

Ausdrücke und Argumente werden **von links nach rechts** ausgewertet (R29).

---

## 3. Die Regeln

### R1 — Programm

**Regel.** Ein Programm besteht nur aus Deklarationen: `const`, `struct`, `enum`, Funktionen, `test`, `use`, `extern`. Es startet in `void main()` oder `void!E main()`.

```ebnf
module   = { NL } { decl { NL } } ;
decl     = const_decl | func_decl ;         (* Teil 1; struct, enum, test, use, extern folgen *)
```

- Eine Anweisung auf oberster Ebene ist `R1`, Hinweis `move it into a function, e.g. void main()`.
- Ein ausführbares Programm hat genau eine Funktion `void main()` ohne Parameter; fehlt sie, ist das `R1` (bei `dif run`/`dif build`, nicht bei `dif check` einer Bibliothek).
- Die Reihenfolge der Deklarationen spielt keine Rolle: eine Funktion darf eine weiter unten deklarierte aufrufen.

```dif
void main() {
    print("Hallo, Welt")
}
```

```dif error R1
print("Hallo")
```

```
hello.dif:1:1: error[R1]: statement outside a function
  hint: move it into a function, e.g. void main() { … }
```

### R2 — Jede Deklaration nennt ihren Typ

**Regel.** Jede Deklaration nennt ihren Typ vorn: Variablen, Parameter, Felder, Schleifen- und `case`-Variablen.

```ebnf
type       = "int" | "bool" | "str" | "void" ;   (* Teil 1; weitere Typen folgen *)
func_decl  = type IDENT "(" [ params ] ")" block ;
params     = param { "," param } [ "," ] ;
param      = type IDENT ;
var_decl   = [ "const" ] type IDENT "=" expr ;
```

- `void` steht nur als Rückgabetyp.
- Eine Variable wird bei der Deklaration immer initialisiert; `int n` ohne `=` ist `R2`, Hinweis `give it a value: int n = 0`.
- Schreibweisen aus anderen Sprachen: `var x = 1`, `let x = 1`, `auto x = 1` sind `R2`, Hinweis mit dem abgeleiteten Typ (`int x = 1`, per `dif fix`). `def`, `fn`, `function`, `func` vor einer Funktion sind `R2`, Hinweis `the return type comes first: int add(int a, int b)`.
- Eine Zuweisung an einen unbekannten Namen ist `R2`, Hinweis `declare it with a type: int x = …`.

```dif
int add(int a, int b) {
    return a + b
}

void main() {
    int x = add(2, 3)
    print(x)
}
```

```dif error R2
void main() {
    var x = 5
    print(x)
}
```

```
main.dif:2:5: error[R2]: declarations name their type
  hint: int x = 5
```

### R3 — Zeilen, Blöcke, Kopfzeilen

**Regel.** Anweisungen enden am Zeilenende. Blöcke stehen in `{}`, Bedingungen ohne Klammern. Die Kopfzeile von `if`, `while`, `for` und `switch` endet am ersten `{`.

```ebnf
block      = "{" { NL } [ stmt { NL { NL } stmt } { NL } ] "}" ;
stmt       = var_decl | assign | if_stmt | while_stmt | for_stmt
           | "break" | "continue" | return_stmt | call_stmt | discard ;
if_stmt    = "if" expr block { "else" "if" expr block } [ "else" block ] ;
assign     = place assign_op expr ;
assign_op  = "=" | "+=" | "-=" | "*=" | "/=" | "%=" ;
place      = IDENT { "." IDENT | "[" expr "]" } ;
return_stmt = "return" [ expr ] ;
call_stmt  = postfix ;                      (* muss ein Aufruf sein *)
discard    = "_" "=" expr ;
```

- Jeder Block steht in `{ }`, auch bei einer einzigen Anweisung. `if x > 0 print(x)` ist `S1`.
- Die Kopfzeile endet am ersten `{` auf Klammerebene 0. Darum ist `if hall.is_full() { … }` immer eine Bedingung mit Rumpf, nie ein Aufruf mit Block-Argument (R26).
- Klammern um die Bedingung sind erlaubt (`if (x > 0) {` ist ein Klammerausdruck); `dif fmt` entfernt sie.
- `elif` und `else if` ohne Leerzeichen-Form: `elif` ist `S1`, Hinweis `use else if`.
- Eine Anweisung, die kein Aufruf, keine Zuweisung und keine Deklaration ist (z. B. `x + 1` allein), ist `S4`.

```dif snippet
int n = 3
if n > 0 {
    n -= 1
} else if n == 0 {
    print("null")
} else {
    print("negativ")
}
```

```dif error R3
void main() {
    int n = 3
    if n > 0
    {
        n -= 1
    }
}
```

```
main.dif:3:13: error[R3]: the block of `if` starts on the same line
  hint: if n > 0 {
```

### R4 — Variablen, `const`, Gültigkeit

**Regel.** Variablen sind veränderlich, `const` nicht. Global gibt es nur `const`. Ein Name gilt bis zum Ende seines Blocks und wird darin nicht neu deklariert.

```ebnf
const_decl = "const" type IDENT "=" expr ;          (* oberste Ebene und in Blöcken *)
var_decl   = [ "const" ] type IDENT "=" expr ;      (* nur in Blöcken, siehe R2 *)
```

- Auf oberster Ebene ist nur `const` erlaubt; `int x = 1` dort ist `R4`, Hinweis `make it const, or move it into a function`.
- Eine Zuweisung an eine `const`-Variable ist `R4`.
- Ein Name gilt von seiner Deklaration bis zum Ende des Blocks, in dem er steht, einschließlich innerer Blöcke. Ihn darin noch einmal zu deklarieren — auch in einem inneren Block, auch als Parameter- oder Schleifenvariable — ist `R4` (kein Verdecken).
- Funktionen, Konstanten und vordefinierte Namen (`int`, `print`, …) leben im Modul; eine lokale Variable mit demselben Namen ist `R4`.
- Eine globale `const` braucht einen Wert, der beim Kompilieren feststeht: Literale, andere `const` und Operatoren darauf.

```dif
const int LIMIT = 100

void main() {
    int total = 0
    for int i in range(10) {
        total += i
    }
    print(total)
}
```

```dif error R4
void main() {
    int n = 1
    if n > 0 {
        int n = 2
        print(n)
    }
}
```

```
main.dif:4:13: error[R4]: `n` is already declared in this function (main.dif:2:9)
  hint: choose another name
```

### R5 — Werte und Griffe

**Regel.** Zuweisung und Rückgabe kopieren den Wert; die Kopie ist unabhängig, intern aber billig. Ein Griff (Kanal, Task, `shared`, Datei, Bibliothek, C-Objekt) ist ein Wert, der auf etwas außerhalb zeigt: kopiert wird der Griff, nicht das Ding. Parameter ohne `mut` sind lokale Kopien, wie in C.

*Teil 1: Für `int`, `bool` und `str` gilt die Regel ohne Sonderfall — Parameter sind lokale Kopien und dürfen im Rumpf geändert werden. Ausarbeitung mit Speichermodell in Teil 2 (M2).*

### R6 — `mut`

**Regel.** Soll eine Funktion ein Argument ändern, steht `mut` in der Signatur und am Aufruf; die Änderung landet beim Aufrufer. Eine Änderung, die danach nie gelesen wird, ist ein Fehler. `mut` nimmt Orte: Variablen, Felder und Elemente. Ein Ort geht höchstens einmal veränderbar in einen Aufruf; überlappende Orte sind ein Fehler bzw. ein Abbruch; ein zusätzlich gelesener Ort wird als Schnappschuss übergeben.

*Ausarbeitung in Teil 2 (M2).*

### R7 — Methoden

**Regel.** Eine Methode ist eine Funktion an einem Typ: `R T.name(params)`. In ihr ist `self` immer der Ort, an dem sie aufgerufen wird, und darf geändert werden. Methoden, die `self` nicht ändern, gehen auch auf Ausdrücken.

*Ausarbeitung in Teil 2 (M2).*

### R8 — Keine Typmischung

**Regel.** Typen mischen sich nie von selbst. Operatoren verlangen gleiche Typen; `T(x)` wandelt exakt um oder bricht ab; absichtlich runden mit `round()`/`trunc()`; Text baut man mit `"{x}"`. Ausdrücke nur aus Literalen rechnen im Typ ihrer Stelle; ohne Stelle ist ein Literal ohne Punkt `int`, mit Punkt `f64`; Float-Literale werden auf den nächsten Wert gerundet. „Steht beim Kompilieren fest“ heißt: nur Literale und `const`.

**Grammatik:** die Ausdrücke aus §2; die Regel prüft ihre Typen.

Teil 1 legt fest:

| Operator | Operanden | Ergebnis |
|---|---|---|
| `+ - * / %`, `+% -% *%` | `int, int` | `int` |
| `& \| ^ << >>` | `int, int` | `int` |
| `~`, Vorzeichen `-` | `int` | `int` |
| `+` | `str, str` | `str` |
| `== !=` | gleicher Typ (`int`, `bool`, `str`) | `bool` |
| `< <= > >=` | `int, int` oder `str, str` | `bool` |
| `&& \|\|`, `!` | `bool` | `bool` |

- Jede andere Kombination ist `R8`, z. B. `"41" + 1` mit Hinweis `use "{a}{b}" to build text`.
- **Stelle** eines Ausdrucks ist der Typ, den sein Ziel verlangt: die deklarierte Variable, der Parameter, der Rückgabetyp, der andere Operand eines Vergleichs. Ein Ausdruck nur aus Literalen wird im Typ seiner Stelle gerechnet (`int k = 7 / 2` ist `3`); ohne Stelle (`print(7 / 2)`) ist er `int`.
- Ein Literal, das an seiner Stelle nicht passt (`bool b = 1`), ist `R8`.

```dif snippet
int a = 7 / 2
str s = "a ist {a}"
print(s)
```

```dif error R8
void main() {
    print("41" + 1)
}
```

```
main.dif:2:11: error[R8]: cannot apply `+` to str and int
  hint: build text with interpolation: "{a}{b}"
```

### R9 — Bedingungen sind `bool`

**Regel.** Bedingungen sind `bool`, nichts anderes.

```ebnf
cond = expr ;      (* in if_stmt, while_stmt und als Operand von &&, ||, ! *)
```

Bedingungen stehen in `if`, `else if`, `while` und als Operanden von `&&`, `||`, `!`. Ein anderer Typ ist `R9`; es gibt kein „wahr-ish“ für Zahlen oder Texte.

```dif snippet
int n = 3
while n > 0 {
    n -= 1
}
```

```dif error R9
void main() {
    int n = 3
    while n {
        n -= 1
    }
}
```

```
main.dif:3:11: error[R9]: condition is int, not bool
  hint: compare explicitly: while n != 0
```

### R10 — `switch`

**Regel.** `switch` deckt jeden möglichen Wert ab: alle Varianten oder `default`. Jeder `case` ist ein eigener Block und endet von selbst. Varianten eines `enum` schreibt man mit ihrem Typ; `T!E` und `?T` haben die eingebauten Formen `ok`, `err`, `some`, `none`.

*Ausarbeitung in Teil 2 (M2).*

### R11 — Schleifen

**Regel.** Schleifen: `for T x in …` (über `range(n)`, `range(a, b)`, Listen, `io.lines()`, Kanäle), `for int i, T x in xs`, `for K k, V v in m` und `while cond`. `break` und `continue` gehören zur innersten Schleife; `break` steht nie direkt in einem `switch`.

```ebnf
while_stmt = "while" expr block ;
for_stmt   = "for" type IDENT [ "," type IDENT ] "in" expr block ;
```

Teil 1 legt fest:

- `range(n)` liefert die Ganzzahlen `0 … n-1`, `range(a, b)` die Ganzzahlen `a … b-1`; ist das Ende nicht größer als der Anfang, läuft die Schleife nicht. Grenzen werden einmal vor der Schleife ausgewertet.
- Die Schleifenvariable ist eine neue Variable im Schleifenblock (R4); ändert der Rumpf sie, wirkt das nur bis zum nächsten Durchlauf.
- `break` verlässt, `continue` beendet den Durchlauf der innersten Schleife. Außerhalb einer Schleife sind beide `R11`.
- Die C-Schleife `for (int i = 0; i < n; i++)` ist `R11`, Hinweis `for int i in range(n)`.

```dif snippet
for int i in range(1, 4) {
    if i == 2 {
        continue
    }
    print(i)
}
```

```dif error R11
void main() {
    for (int i = 0; i < 3; i += 1) {
        print(i)
    }
}
```

```
main.dif:2:9: error[R11]: C-style for loop
  hint: for int i in range(3)
```

### R12 — Scheitern steht im Typ

**Regel.** Kann eine Funktion scheitern, steht es im Rückgabetyp: `T!E` heißt Wert oder `err(e)`, `?T` heißt Wert oder `none`. Erfolg gibt man einfach zurück. `try x` reicht einen Fehler gleichen Typs weiter; `try x or E.V(…)` übersetzt ihn vorher.

*Ausarbeitung in Teil 2 (M2).*

### R13 — Jeder Aufruf hat eine Wirkung

**Regel.** Ein Ergebnis vom Typ `T!E` oder `?T` wird immer verwendet: `try`, `??`, `switch` oder bewusst `_ =`. Jeder andere Aufruf braucht eine Wirkung: sein Ergebnis wird verwendet, oder er ändert etwas (`mut`, `self`), macht Ein-/Ausgabe, ruft C oder startet einen Task. Ein Aufruf ohne Wirkung ist ein Fehler.

**Grammatik:** `call_stmt` und `discard` aus R3.

Teil 1 legt fest:

- Ein Aufruf als Anweisung, dessen Ergebnis nicht `void` ist, ist `R13`, Hinweis `use it, or discard it explicitly with _ =`.
- `_ = f()` verwirft ein Ergebnis bewusst.
- Ein Aufruf einer `void`-Funktion als Anweisung ist erlaubt. (Die Prüfung „ohne Wirkung“ für `void`-Funktionen folgt mit der Wirkungsanalyse in Teil 2.)

```dif error R13
int fib(int n) {
    return n
}

void main() {
    fib(3)
}
```

```
main.dif:6:5: error[R13]: result of `fib(3)` is never used
  hint: use it, or discard it explicitly with `_ =`
```

### R14 — Abbruch bei Programmierfehlern

**Regel.** Programmierfehler brechen mit Meldung und Position ab: Index außerhalb, Überlauf, Division durch 0, ungenaue Umwandlung, `assert`. Immer, auch im Release.

**Grammatik:** keine eigene; die Regel gilt zur Laufzeit für die Operatoren aus §2.

Teil 1 legt fest (alle für `int`, siehe R29):

| Fall | Meldung |
|---|---|
| Überlauf bei `+ - *` und Vorzeichen `-` | `integer overflow in +` (bzw. `-`, `*`) |
| Division oder `%` durch 0 | `division by zero` |
| `INT_MIN / -1`, `INT_MIN % -1` | `integer overflow in /` (bzw. `%`) |
| Shift mit Weite < 0 oder ≥ 64 | `shift by <n> out of range 0..63` |

Die Position ist die des Operators im Quelltext. Der Abbruch läuft ohne `defer` (R23) und endet mit Exit-Code 2. `+% -% *%` rechnen bewusst modulo 2⁶⁴ und brechen nie ab.

```dif snippet
int big = 9223372036854775807
print(big +% 1)
```

Ausgabe: `-9223372036854775808`. Ohne `%`:

```
panic: integer overflow in + (main.dif:3:15)
```

### R15 — `if` als Ausdruck

**Regel.** `if` mit `else` liefert einen Wert, wenn jeder Zweig mit einem Ausdruck endet.

*Ausarbeitung in Teil 2 (M2).*

### R16 — Module

**Regel.** Eine Datei ist ein Modul. `use name` lädt `name.dif` neben der Datei, `use std.x` die Standardbibliothek, `use c "x.h"` einen C-Header; `as` gibt jedem Modul einen anderen Namen; zyklische `use` sind ein Fehler. `pub` vor Typen, Funktionen, Feldern und Methoden macht sie sichtbar. Funktionen ohne `self` gehören in ein Modul, nicht an einen Typ.

*Ausarbeitung in Teil 2/3.*

### R17 — C, Pointer, `unsafe`

**Regel.** Pointer gibt es nur in `unsafe { }` und in C-Deklarationen; dort gelten C-Regeln, ein roher Pointer `*T` darf wie in C NULL sein. Strings und Griffe, die C zurückgibt, sind `?T`; Parameter an C sind nicht optional, außer `extern` erlaubt `?`. Pointer auf opake C-Typen sind Griffe; mit Freigeber (`release T with f`) gezählt, `f` läuft genau einmal. Strings an C gelten nur für die Dauer des Aufrufs, ein NUL-Byte darin bricht ab. Nicht genannte Felder eines C-structs sind 0.

*Ausarbeitung mit M4 (`extern`) und v0.1 (`use c`).*

### R18 — Tests

**Regel.** Tests stehen neben dem Code: `test "name" { }`, geprüft mit `expect(cond)`. `dif test` führt sie aus, auch die Codebeispiele in `///`-Kommentaren.

*Ausarbeitung in Teil 2 (M2).*

### R19–R22 — Nebenläufigkeit *(nach v0)*

Wortlaut in PLAN.md §3.2. Die Schlüsselwörter `spawn`, `parallel`, `yield` sind reserviert.

### R23 — `defer`

**Regel.** `defer X` führt X am Ende des umgebenden Blocks aus, als stünde es dort, auf jedem Weg hinaus (Ende, `return`, `try`, `break`); Argumente werden erst dann ausgewertet. Mehrere `defer` laufen in umgekehrter Reihenfolge. Ein Ergebnis vom Typ `T!E` braucht `_ =`. Ein Abbruch (R14) beendet das Programm ohne `defer`.

*Ausarbeitung mit M4.*

### R24 — C-Callbacks mit Kontext

**Regel.** Erwartet C ein Paar aus Funktion und `void *user`, übergibt man eine dif-Funktion und, wenn sie Zustand braucht, eine Variable; die Funktion bekommt sie als ersten Parameter, mit `mut`, wenn sie sie ändert. Keine Funktion: `none`. Der Kontext gilt, solange der Aufruf läuft, in den die Variable mit `mut` hineinging; ein Callback außerhalb oder aus einem anderen Thread bricht ab.

*Ausarbeitung mit v0.1.*

### R25 — Benannte Parameter

**Regel.** Parameter mit Default stehen hinten und werden beim Aufruf beim Namen genannt (`name: wert`), alle anderen der Reihe nach.

*Ausarbeitung mit M3.*

### R26 — Funktionswerte und Blöcke

**Regel.** Ein Funktionswert hat den Typ `R(T1, T2)` und wird geschrieben wie eine Funktion ohne Namen: `R(T1 a, T2 b) { … }`. Ein Literal steht nur direkt als Argument eines Aufrufs und lebt nur während dieses Aufrufs; es darf ringsum lesen und ändern. Ein Literal ohne Parameter darf als Block `{ … }` direkt hinter der schließenden Klammer stehen; `return` und `try` darin beenden den Block. Benannte Funktionen und Varianten-Konstruktoren sind gewöhnliche Werte. Ändert ein Funktionswert einen Parameter, steht `mut` in seinem Typ.

*Ausarbeitung mit M3.*

### R27 — Generics

**Regel.** Typparameter stehen in eckigen Klammern hinter dem Namen und werden beim Aufruf meist abgeleitet; jede Instanz wird eigener Code. Eine Einschränkung steht vor dem Typparameter: `equal`, `ordered`, `hashable`, `text`.

*Ausarbeitung mit M3.*

### R28 — Fähigkeiten

**Regel.** Jeder Typ ohne Funktionswert-Felder ist `equal` (`==`) und `text` (Ausgabe, `"{x}"`); `ordered` (`<`) sind Zahlen und `str`; `hashable` ist alles, was `equal` ist. Griffe vergleichen ihre Identität, Maps sind gleich, wenn ihre Einträge gleich sind.

Teil 1 legt die Textform fest: `int` dezimal mit `-` für negative Zahlen, `bool` als `true`/`false`, `str` unverändert.

### R29 — Zahlen

**Regel.** Ganzzahl-`/` rundet Richtung 0, `%` hat das Vorzeichen des Dividenden; Floats folgen IEEE 754; Ganzzahl-Division durch 0, `INT_MIN / -1` und Shifts ab der Bitbreite brechen ab. Ausdrücke und Argumente werden von links nach rechts ausgewertet.

**Grammatik:** `INT` aus §1.4 und die Operatoren aus §2.

Teil 1 legt den Ganzzahl-Teil fest:

- `int` ist immer 64 Bit mit Zweierkomplement, Bereich `-9223372036854775808 … 9223372036854775807`.
- `a / b` rundet Richtung 0, `a % b` hat das Vorzeichen von `a`, es gilt `(a / b) * b + a % b == a`.
- `<<` und `>>` verschieben um `0 … 63` Stellen; `>>` ist arithmetisch (füllt mit dem Vorzeichen). Andere Weiten brechen ab (R14).
- `+% -% *%` rechnen modulo 2⁶⁴.
- Auswertungsreihenfolge: Operanden von links nach rechts, Argumente von links nach rechts, bei einer Zuweisung erst der Ort, dann der Wert.

```dif snippet
print(-7 / 2)
print(-7 % 3)
print(1 << 62)
```

Ausgabe: `-3`, `-1`, `4611686018427387904`.

Verstoß zur Laufzeit (R14):

```dif snippet
int w = 64
print(1 << w)
```

```
panic: shift by 64 out of range 0..63 (main.dif:3:13)
```

Ein Literal, das in `int` nicht passt (`9223372036854775808`), ist schon beim Kompilieren `L4`.

### R30 — Maps

**Regel.** `map[K]V` schreibt man `{k: v, …}`; `m[k]` liefert `?V`, `m[k] = v` setzt, die Reihenfolge ist die der Einfügung.

*Ausarbeitung in Teil 2 (M2).*

### R31 — Erlaubte Umwandlungen

**Regel.** Ein Wert vom Typ `T` passt dort, wo `?T` oder `T!E` erwartet wird; `ok` und `some` gibt es nur in Mustern. Ein `str` passt dort, wo C `c.str` erwartet, eine benannte Funktion an einen C-Funktionszeiger. Sonst wird nie umgewandelt (R8).

*Ausarbeitung in Teil 2 (M2).*

---

## 4. Eingebaute Funktionen (Teil 1)

| Funktion | Bedeutung |
|---|---|
| `void print(text T x)` | schreibt `"{x}"` und einen Zeilenumbruch auf stdout |
| `range(int n)`, `range(int a, int b)` | Ganzzahlen `0 … n-1` bzw. `a … b-1`; in Teil 1 nur als Quelle einer `for`-Schleife |

`print` ist ein Aufruf mit Ein-/Ausgabe und erfüllt damit R13. `repeat`, `expect` und `assert` folgen in Teil 2.
