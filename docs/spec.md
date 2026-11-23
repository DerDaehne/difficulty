# difficulty — Spezifikation

Stand: 2026-11-23 · Version: v0, Teil 1 („Hallo, nativ“) und Teil 2a (Daten) · Begründungen und Fahrplan: [PLAN.md](../PLAN.md)

Diese Spezifikation ist nach den Regeln R1–R31 gegliedert. Ausgearbeitet sind R1–R11, R13–R15, R28 (teilweise) und R29. Alle übrigen stehen mit ihrem vollständigen Wortlaut da und werden mit den nächsten Teilen ausgearbeitet.

## 0. Wie diese Spezifikation zu lesen ist

**Grammatik** steht in EBNF: `a b` Folge, `a | b` Auswahl, `[ a ]` optional, `{ a }` beliebig oft, `"x"` wörtlich, `NL` Zeilenende (siehe §1.3). Jede Produktion steht genau einmal; andere Abschnitte verweisen darauf.

**Kanonische Form.** Wo der Compiler mehrere Schreibweisen annimmt (Feldreihenfolge im Struct-Literal, abschließendes Komma in mehrzeiligen Listen, Trenner der `enum`-Varianten), legt `dif fmt` die eine kanonische Form fest; der Compiler meldet dafür keinen Fehler.

### 0.1 Diagnosen

Ein Verstoß gegen eine Regel trägt deren Nummer: `error[R13]`. Schreibfehler, die keine Regel betreffen, tragen `L` (Lexik) oder `S` (Grammatik):

| Code | Bedeutung |
|---|---|
| `L1` | ungültiges UTF-8 oder ein Zeichen, das außerhalb von Strings und Kommentaren nicht vorkommen darf |
| `L2` | Blockkommentar `/* … */` |
| `L3` | `;` |
| `L4` | Ganzzahl-Literal größer als 2⁶⁴ − 1 |
| `L5` | String reicht über das Zeilenende |
| `L6` | unbekannte Escape-Sequenz, einzelnes `}` oder nicht geschlossenes `{` im String |
| `L7` | `++` oder `--` |
| `L8` | Zahl-Literal in falscher Form: führende Null (`010`), Präfix `0X`/`0B`, `_` direkt nach dem Präfix oder im Exponenten, fehlende oder falsche Ziffern (`0x`, `0b12`, `1e`), Buchstaben direkt nach der Zahl |
| `S1` | unerwartetes Token |
| `S2` | Vergleichskette `a < b < c` |
| `S3` | `&&` und `\|\|` gemischt ohne Klammern |
| `S4` | Ausdruck als Anweisung, der weder Aufruf noch Zuweisung ist |

Jede Meldung nennt Datei, Zeile und Spalte (beide ab 1; Spalten zählen Zeichen, also Unicode-Codepoints), eine Ursache und, wo möglich, einen Hinweis, der den Fehler behebt. `--json` liefert zusätzlich den Byte-Offset. Bei Operatoren zeigt die Position auf den Operator. Meldungen sind englisch (`--lang de` für Deutsch).

**Eine Ursache, eine Meldung.** Erkennt der Compiler eine Schreibweise aus einer anderen Sprache als Ganzes (C-Schleife `for (…;…;…)`, `var x = …`, `elif`, `function f(…)`), meldet er nur die Regel, deren Schreibweise sie ersetzt, mit dem passenden Hinweis; die Zeichen darin (etwa `;`) erzeugen keine weitere Meldung. Sonst wird der erste Fehler einer Anweisung gemeldet.

**Weiterlesen nach einem Fehler.** Der Parser überspringt den Rest der Anweisung bis zum Zeilenende auf derselben Klammerebene und einen dort direkt folgenden Block. Eine Deklaration in fremder Schreibweise (`var x = 5`) gilt danach als Deklaration mit dem abgeleiteten Typ, damit keine Folgefehler entstehen; ebenso ein reserviertes Wort als Name (`bool ok = true`), dessen spätere Benutzungen nicht erneut gemeldet werden.

- Jede Anweisung und jede Deklaration meldet höchstens einen Syntaxfehler. Die Anweisungen eines Blocks darin zählen für sich.
- Lexikfehler werden immer gemeldet. Zerreißt einer den Token-Strom (`L1`, `L5`), entfällt der Syntaxfehler, den er in seiner Anweisung auslöst.
- Fehlt eine `)` oder `]` und geht es in der nächsten Zeile weiter, steht die Meldung an der öffnenden Klammer.
- Beginnt in einem Block eine Funktionsdeklaration (`T name(`), fehlt davor eine `}`: gemeldet wird einmal am innersten offenen Block, die Funktion wird normal gelesen.
- Verschachtelung tiefer als 256 Ebenen (Blöcke und Ausdrücke) ist `S1`, ebenso ein Ausdruck mit mehr als 1024 binären Operatoren und eine Kette von mehr als 256 globalen `const`, die einander verwenden (dort `R4`).

```
fib.dif:6:5: error[R13]: result of `fib(3)` is never used
  hint: use it, or discard it explicitly with `_ = fib(3)`
```

### 0.2 Beispiele

Beispiele gibt es in vier Arten, die `dif test` als Doc-Tests ausführt:

- ` ```dif ` — ein vollständiges, gültiges Modul.
- ` ```dif snippet ` — Anweisungen, die in `void main() { … }` eingebettet werden: Zeile 1 des Ausschnitts ist Zeile 2 der Datei, um 4 Leerzeichen eingerückt. Positionen in Meldungen beziehen sich auf die Datei `main.dif`.
- ` ```dif error R4 ` — ein Modul, das genau diese Diagnose liefern muss (und keine andere). Geprüft wird wie mit `dif check`; ein fehlendes `main` zählt dort nicht.
- ` ```dif panic "message" ` — ein Ausschnitt wie bei `snippet`, der mit genau dieser Meldung abbrechen muss.

### 0.3 Abbruch

Programmierfehler (R14) beenden das Programm sofort mit

```
panic: <message> (<file>:<line>:<column>)
```

auf stderr und Exit-Code 2. Ein Fehler, der aus `void!E main()` herauskommt, endet mit Exit-Code 1.

---

## 1. Lexik

### 1.1 Zeichen

Quelltext ist UTF-8. Ungültiges UTF-8 ist `L1`. Nicht-ASCII-Zeichen sind nur in Strings und Kommentaren erlaubt, sonst `L1`.

```ebnf
letter  = "a" … "z" | "A" … "Z" | "_" ;
digit   = "0" … "9" ;
hexdigit = digit | "a" … "f" | "A" … "F" ;
IDENT   = letter { letter | digit } ;
```

Ein Bezeichner, der ein Schlüsselwort oder ein reserviertes Wort ist (§1.5), ist kein `IDENT`; an der Stelle eines Namens ist er `S1`, Hinweis `choose another name`. Ein Wort mit Buchstaben außerhalb von ASCII (`größe`) ist einmal pro Wort `L1`. `_` allein ist der Platzhalter „verwerfen“ (`_ = f()`), kein Name.

### 1.2 Kommentare

`//` bis Zeilenende. `///` ist ein Doku-Kommentar und gehört zur folgenden Deklaration (`dif doc`, Doc-Tests). `/* … */` gibt es nicht: `L2`, Hinweis `use // for comments`. Ein `/* …` ohne `*/` endet mit seiner Zeile; ein `/* … */` über mehrere Zeilen beendet die Zeile wie ein Zeilenumbruch.

### 1.3 Zeilenende und Fortsetzung

Eine Anweisung endet am Zeilenende (R3). Der Lexer erzeugt dafür ein Token `NL`, außer:

1. innerhalb von `( … )` und `[ … ]` — dort spielt das Zeilenende keine Rolle, bis wieder ein `{` geöffnet wird;
2. die Zeile endet mit einem binären Operator, mit `=` oder einem Zuweisungsoperator, oder mit `,`;
3. die nächste Zeile beginnt (nach Leerraum) mit `.`.

Leere Zeilen und Zeilen nur mit Kommentar erzeugen kein `NL`; mehrere `NL` hintereinander gelten wie eines. Zwei Anweisungen in einer Zeile sind `R3`. `;` ist `L3`, Hinweis `statements end at the end of the line`.

### 1.4 Literale

```ebnf
INT      = dec | "0x" hexdigit { hexdigit | "_" } | "0b" ( "0" | "1" ) { "0" | "1" | "_" } ;
dec      = "0" | ( "1" … "9" ) { digit | "_" } ;
FLOAT    = digit { digit | "_" } "." digit { digit | "_" } [ exponent ] | dec exponent ;
exponent = ( "e" | "E" ) [ "+" | "-" ] digit { digit } ;
STRING   = '"' { char | escape | interp } '"' ;
char     = (* jedes Zeichen außer '"', "\", "{", "}" und dem Zeilenende *) ;
escape   = "\" ( "n" | "t" | "r" | "\" | '"' | "{" | "}" | "0" ) | "\u{" hexdigit { hexdigit } "}" ;
interp   = "{" expr [ ":" format ] "}" ;
```

- `_` in Zahlen dient nur der Lesbarkeit: `1_000_000`. Ein Ganzzahl-Literal größer als 2⁶⁴ − 1 ist `L4`; ob ein kleineres an seiner Stelle passt, prüft R8.
- Eine Ganzzahl mit führender Null (`010`) ist `L8`, Hinweis `write 10; octal literals do not exist`. `0` allein und `0.5` sind erlaubt.
- Jede andere Form außerhalb der Grammatik ist ebenfalls `L8` und bleibt ein Token: `0X10`, `0x_ff`, `0x`, `0b102`, `1e`, `1e1_0`, `12ab`.
- Ein String reicht nicht über das Zeilenende hinaus (`L5`). Eine unbekannte Escape-Sequenz ist `L6`. `\{` und `\}` schreiben eine wörtliche Klammer; ein einzelnes `}` ohne `\` ist `L6`.
- `{expr}` setzt den Wert von `expr` als Text ein; `expr` muss `text` sein (R28). Im Ausdruck einer Interpolation stehen keine Strings und keine geschweiften Klammern (`S1`). Formatangaben `{x:.2f}` werden in Teil 2 festgelegt; bis dahin sind sie `S1`.

### 1.5 Wörter

**Schlüsselwörter** (25), nie als Namen nutzbar:

```
const struct enum if else while for in break continue return switch case default
try or mut use pub test extern unsafe void self defer
```

**Reserviert für nach v0:** `spawn parallel yield`.

**Reservierte Literale und Formen**, nie als Namen nutzbar: `true false` (Werte von `bool`), `none some ok err` (Formen von `?T` und `T!E`, R10).

**Kontextwörter** haben nur an einer Stelle eine Bedeutung und sind sonst gewöhnliche Namen: `as` in `use`-Zeilen; `c link pkg source impl load strip release with send` in `use c`- und `extern`-Zeilen; `panics` im Kopf eines `test`.

**Vordefinierte Namen** sind keine Schlüsselwörter, dürfen aber nicht neu deklariert werden (R4): die Typen `int i8 i16 i32 u8 u16 u32 u64 f32 f64 bool str rune` und die eingebauten Funktionen `print range repeat expect assert`.

### 1.6 Operatoren und Satzzeichen

```
+   -   *   /   %   +%  -%  *%  &   |   ^   ~   <<  >>
==  !=  <   <=  >   >=  &&  ||  !   ??
=   +=  -=  *=  /=  %=  &=  |=  ^=  <<= >>=
(   )   [   ]   {   }   ,   .   :
```

`;` (`L3`), `++` und `--` (`L7`, Hinweis `use x += 1`) gibt es nicht.

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

`or` gehört zu einem vorangehenden `try` (`try x or E.V(…)`, Teil 2). Klammern um einen Ausdruck sind immer erlaubt; `dif fmt` entfernt sie um Bedingungen.

```ebnf
expr     = binary ;
binary   = unary { binop unary } ;          (* geordnet nach der Tabelle *)
unary    = { "-" | "!" | "~" | "try" } postfix ;
postfix  = primary { "(" [ args ] ")" | "[" expr "]" | "." IDENT } ;
args     = arg { "," arg } [ "," ] ;
arg      = [ "mut" ] expr ;                          (* nach mut steht ein Ort (place, R3); R6 *)
primary  = INT | FLOAT | STRING | "true" | "false" | IDENT | "self" | "(" expr ")"
         | struct_lit | variant_val | if_expr ;      (* R5, R7, R15 *)
```

Ausdrücke und Argumente werden **von links nach rechts** ausgewertet (R29).

---

## 3. Die Regeln

### R1 — Programm

**Regel.** Ein Programm besteht nur aus Deklarationen: `const`, `struct`, `enum`, Funktionen, `test`, `use`, `extern`. Es startet in `void main()` oder `void!E main()`.

```ebnf
module   = { NL } [ decl { NL { NL } decl } { NL } ] ;
decl     = const_decl | func_decl | struct_decl | enum_decl | method_decl ;   (* test, use, extern folgen *)
```

- Eine Anweisung auf oberster Ebene ist `R1`, Hinweis `move it into a function, e.g. void main() { … }`.
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
main.dif:1:1: error[R1]: statement outside a function
  hint: move it into a function, e.g. void main() { … }
```

### R2 — Jede Deklaration nennt ihren Typ

**Regel.** Jede Deklaration nennt ihren Typ vorn: Variablen, Parameter, Felder, Schleifen- und `case`-Variablen.

```ebnf
type       = "int" | "bool" | "str" | "void" | IDENT ;   (* IDENT: struct oder enum des Moduls; weitere Typen folgen *)
func_decl  = type IDENT "(" [ params ] ")" block ;
params     = param { "," param } [ "," ] ;
param      = [ "mut" ] type IDENT ;                   (* mut: R6 *)
var_decl   = [ "const" ] type IDENT "=" expr ;
```

- `void` steht nur als Rückgabetyp.
- Eine Variable wird bei der Deklaration immer initialisiert; `int n` ohne `=` ist `R2`, Hinweis `give it a value: int n = 0`.
- Schreibweisen aus anderen Sprachen: `var x = 1`, `let x = 1`, `auto x = 1` sind `R2`, Hinweis mit dem abgeleiteten Typ (`int x = 1`, per `dif fix`). `def`, `fn`, `function`, `func` vor einer Funktion sind `R2`, Hinweis `the return type comes first: int add(int a, int b)`.
- `name: T` in einer Parameterliste und `-> T` nach ihr sind `R2`, Hinweis mit der ganzen Kopfzeile in Typ-vorn-Schreibweise (`int add(int a, int b) {`). `void main(void)` ist `S1`, Hinweis `void main() {`.
- Schleifenvariablen ohne Typ (`for i in range(10)`) sind `R2`, Hinweis `for int i in range(10)`.
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
block       = "{" { NL } [ stmt { NL { NL } stmt } { NL } ] "}" ;
stmt        = var_decl | assign | if_stmt | while_stmt | for_stmt | switch_stmt
            | "break" | "continue" | return_stmt | call_stmt | discard ;
if_stmt     = "if" expr block { "else" "if" expr block } [ "else" block ] ;
assign      = place assign_op expr ;
assign_op   = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ;
place       = ( IDENT | "self" ) { "." IDENT | "[" expr "]" } ;   (* self: R7 *)
return_stmt = "return" [ expr ] ;
call_stmt   = postfix ;                      (* muss mit einem Aufruf enden *)
discard     = "_" "=" expr ;
```

- Jeder Block steht in `{ }`, auch bei einer einzigen Anweisung: `if x > 0 print(x)` ist `R3`, Hinweis `put the body in braces: if x > 0 { … }`.
- Das `{` eines Blocks steht in derselben Zeile wie seine Kopfzeile; `}` und `else` stehen in einer Zeile (`} else {`).
- Zwei Anweisungen in einer Zeile sind `R3`, Hinweis `put each statement on its own line`. Folgt einer Anweisung ein Token, das keine Anweisung beginnen kann (`)`, `=`), ist das `S1`.
- Die Kopfzeile endet am ersten `{` auf Klammerebene 0. Darum ist `if hall.is_full() { … }` immer eine Bedingung mit Rumpf, nie ein Aufruf mit Block-Argument (R26). Ein Struct-Literal oder ein `if`-Ausdruck in einer Kopfzeile steht deshalb in Klammern: `if p == (Point{x: 0, y: 0}) {`. Ein Struct-Literal ohne Klammern dort ist `S1` am Typnamen, Meldung `a struct literal in a header needs parentheses`, Hinweis mit der geklammerten Kopfzeile.
- Klammern um die Bedingung sind erlaubt (`if (x > 0) {` ist ein Klammerausdruck); `dif fmt` entfernt sie.
- `elif` (Python) und `elsif` (Ruby, Perl) sind `R3`, Hinweis `use else if`, auch direkt nach `}`; der Zweig wird danach wie `else if` gelesen.
- `=` in einer Bedingung (`if x = 1 {`) ist `S1`, Hinweis `if x == 1 {`.
- Eine Anweisung, die weder Aufruf noch Zuweisung noch Deklaration ist (z. B. `x + 1` allein), ist `S4`.

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
main.dif:3:13: error[R3]: the body of `if` must start on the same line
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
- Funktionen, Konstanten, `struct`- und `enum`-Typen und vordefinierte Namen (§1.5) leben im Modul; eine lokale Variable oder ein Parameter mit demselben Namen ist `R4`. Felder und Methoden leben in ihrem Typ, Varianten in ihrem `enum`; ein Name kommt dort jeweils nur einmal vor (`R4`).
- Die Benutzung eines Namens, der an dieser Stelle nicht gilt oder nirgends deklariert ist, ist `R4`, mit dem ähnlichsten gültigen Namen als Hinweis (`did you mean total?`). Eine Zuweisung an einen unbekannten Namen ist dagegen `R2` (fehlende Deklaration).
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
main.dif:4:13: error[R4]: `n` is already declared at main.dif:2:9
  hint: names are not redeclared inside their block; choose another name
```

### R5 — Werte und Griffe

**Regel.** Zuweisung und Rückgabe kopieren den Wert; die Kopie ist unabhängig, intern aber billig. Ein Griff (Kanal, Task, `shared`, Datei, Bibliothek, C-Objekt) ist ein Wert, der auf etwas außerhalb zeigt: kopiert wird der Griff, nicht das Ding. Parameter ohne `mut` sind lokale Kopien, wie in C.

```ebnf
struct_decl = "struct" IDENT "{" NL { NL } { field NL { NL } } "}" ;
field       = type IDENT [ "=" expr ] ;
enum_decl   = "enum" IDENT "{" { NL } variant { ( "," | NL ) { NL } variant } [ "," ] { NL } "}" ;
variant     = IDENT [ "(" type IDENT { "," type IDENT } ")" ] ;
struct_lit  = IDENT "{" { NL } [ init { "," { NL } init } [ "," ] { NL } ] "}" ;
init        = IDENT ":" expr ;
variant_val = postfix ;      (* E.V oder E.V(args): Typname eines enum vor dem Punkt *)
```

`A.b` ist eine Variante, wenn `A` ein `enum`-Typ ist; sonst ist `.b` ein Feld oder eine Methode des Werts vor dem Punkt.

Teil 2a legt fest (Griffe, Listen, Maps und das Speichermodell folgen in Teil 2c):

- `int`, `bool`, `str`, `struct` und `enum` sind Werte. `Point b = a` kopiert; danach ändert `b.x = 9` nur `b`. Ein Parameter ohne `mut` ist eine lokale Kopie und darf im Rumpf geändert werden, wenn die Änderung dort auch gelesen wird (R6).
- Ein `struct` hat benannte Felder, eines pro Zeile; `struct P { int x }` in einer Zeile ist `R3`, Hinweis `put each field on its own line`.
- Ein Feld darf einen Vorgabewert haben. Er wird bei jedem Literal neu ausgewertet, das das Feld weglässt. Erlaubt ist ein Ausdruck ohne Variablen und ohne `self`: Literale, `const`, Struct-Literale, Varianten, Operatoren und Aufrufe von Funktionen ohne Wirkung (R13); sonst `R5`, Meldung `a field default uses only constants and functions without effect`.
- Ein Literal `Point{x: 1, y: 2}` nennt jedes Feld mit Namen, in beliebiger Reihenfolge (kanonisch ist die Deklarationsreihenfolge, §0), ausgewertet von links nach rechts; danach die Vorgabewerte in Deklarationsreihenfolge. Ein Feld ohne Vorgabe, das fehlt, ist `R5`, Meldung `` `Point` needs a value for `y` ``, Hinweis mit dem ergänzten Literal; eingesetzt wird `0`, `false` oder `""`, bei anderen Typen `…`. Fehlen mehrere, nennt die Meldung sie in Deklarationsreihenfolge (`` `y`, `z` ``). Ein unbekanntes oder doppelt genanntes Feld ist `R4`.
- Ein Literal darf über mehrere Zeilen gehen: Zeilenumbrüche sind nach `{`, nach `,` und vor `}` erlaubt, ein abschließendes Komma ebenso.
- Felder liest und ändert man mit `p.x` und `p.x = 1`.
- Ein `enum` zählt seine Varianten auf, getrennt durch `,` oder Zeilenumbruch. Eine Variante kann benannte Werte tragen (`NoSuchSeat(int row, int seat)`). Man schreibt Varianten immer mit ihrem Typ: `Seat.Free`, `BookErr.NoSuchSeat(3, 5)` mit den Werten in Reihenfolge. An die Werte kommt man nur über `switch` (R10).
- Ein Typ, der sich über Felder oder Varianten-Werte selbst enthält (`struct Node { Node next }`, `enum Tree { Leaf, Node(Tree l, Tree r) }`, auch über mehrere Typen hinweg), ist `R5` an seinem Namen, Meldung `` `Tree` contains itself ``, Hinweis `a value has a fixed size; keep linked values in a list`. Listen kommen in Teil 2c.
- Namen von `struct` und `enum` leben im Modul wie Funktionen (R4). Feld- und Methodennamen eines Typs sind verschieden voneinander (R4).

```dif
struct Point {
    int x
    int y = 0
}

enum Seat { Free, Taken }

void main() {
    Point a = Point{x: 1}
    Point b = a
    b.x = 10
    Seat s = Seat.Taken
    print("{a} {b} {a == b} {s}")
}
```

Ausgabe: `Point{x: 1, y: 0} Point{x: 10, y: 0} false Seat.Taken`.

```dif error R5
struct Point {
    int x
    int y
}

void main() {
    Point p = Point{x: 1}
    print(p)
}
```

```
main.dif:7:15: error[R5]: `Point` needs a value for `y`
  hint: Point{x: 1, y: 0}
```

### R6 — `mut`

**Regel.** Soll eine Funktion ein Argument ändern, steht `mut` in der Signatur und am Aufruf; die Änderung landet beim Aufrufer. Eine Änderung, die danach nie gelesen wird, ist ein Fehler (sie träfe nur eine Kopie). `mut` nimmt Orte: Variablen, Felder und Elemente. Ein Ort geht höchstens einmal veränderbar in einen Aufruf: als `self`, als `mut`-Argument oder über ein Funktionsliteral, das ihn ändert; überlappen zwei Orte, ist das ein Fehler, bei Elementen mit Laufzeit-Index ein Abbruch. Liest derselbe Aufruf einen Ort zusätzlich, bekommt der lesende Teil einen Schnappschuss.

**Grammatik:** `param` (R2), `arg` (§2), `place` (R3).

Teil 2a legt fest:

- **Orte und Wurzeln.** Ein Ort ist eine Variable, ein Parameter oder `self` (seine *Wurzel*), gefolgt von Feldern: `p`, `p.x`, `scene.player.pos`. Was einen Ort ändert, ändert seine Wurzel.
- **Ändern und Lesen.** Ändern heißt: einem Ort zuweisen; mit `+=` usw. rechnen; ihn mit `mut` an einen Parameter übergeben, den die aufgerufene Funktion ändert; auf ihm eine Methode aufrufen, die `self` ändert. Die letzten drei lesen den Ort zugleich. Lesen heißt jede andere Benutzung der Wurzel oder eines Teils davon.
- **Signatur und Aufruf.** Ein Parameter `mut T p` ändert den Ort, den der Aufrufer mit `mut` übergibt: `shift(mut b, 5)`, `shift(mut scene.player, 5)`. Ein Ort mit `const`-Wurzel geht nie mit `mut` hinein (`R4`, `` cannot pass const `k` as mut ``). Meldungen, jeweils `R6` am Argument:
  - `mut` fehlt: `` `shift` takes `p` as mut ``, Hinweis: der Aufruf samt `mut`;
  - `mut` zu viel: `` `show` takes `p` without mut ``, Hinweis: der Aufruf ohne `mut`;
  - kein Ort nach `mut` (`mut f()`, `mut 1`): `only a variable, field or element can be passed as mut`.
- **Überflüssiges `mut`.** Ändert eine Funktion einen `mut`-Parameter nie (kein Ort mit dieser Wurzel wird geändert), ist das `R6` am `mut` des Parameters, Meldung `` `p` is never changed; remove mut ``, Hinweis: die Signatur ohne `mut`. Aufrufe, die dort `mut` übergeben, werden nicht zusätzlich gemeldet und zählen nicht als Änderung.
- **Verlorene Änderung.** Eine Änderung ist verloren, wenn auf keinem Weg danach ihre Wurzel gelesen wird, bevor die Wurzel ganz neu zugewiesen wird (`v = …`; eine Teilzuweisung `v.x = …` beendet nichts) oder die Funktion endet. Am Ende einer Funktion gelten `mut`-Parameter und `self` als gelesen, weil die Änderung beim Aufrufer landet. In einer Schleife zählt der nächste Durchlauf als „danach“; ein `n += 1` im Rumpf liest dort den Wert des vorigen Durchlaufs und gilt darum als gelesen. Eine verlorene Änderung ist `R6` an ihrem Ort:
  - an einem Parameter ohne `mut`: `` the change to `p` is lost: `p` is a copy ``, Hinweis `take it as mut: ` und die Signatur samt `mut`;
  - sonst: `` `x` is changed but never read afterwards ``, Hinweis `use the value, or remove the change`.
  
  Gemeldet wird pro Wurzel die erste verlorene Änderung. Der Anfangswert einer Deklaration ist keine Änderung: `int x = 0` gefolgt von `x = 1; x = 2; print(x)` meldet `x = 1`.
- **Keine Aliase.** In einem Aufruf geht ein Ort höchstens einmal veränderbar hinein: als `self` einer Methode, die `self` ändert, oder als `mut`-Argument. Zwei solche Orte dürfen sich nicht überlappen (`a` und `a.x`, oder zweimal `a.x`), sonst `R6` am zweiten, Meldung `` `p` goes into this call changeable twice ``, Hinweis `pass it changeable only once`. Liest der Aufruf denselben Ort zusätzlich als Argument ohne `mut`, bekommt dieses Argument den Wert von vor dem Aufruf. Ein Aufruf im Argument eines anderen ist ein eigener Aufruf: `f(mut a, g(mut a))` ist erlaubt, `g` ist fertig, bevor `f` beginnt.

```dif
struct Point {
    int x
    int y
}

void shift(mut Point p, int dx) {
    p.x += dx
}

int gcd(int a, int b) {
    while b != 0 {
        int t = b
        b = a % b
        a = t
    }
    return a
}

void main() {
    Point b = Point{x: 1, y: 2}
    shift(mut b, 5)
    print(b)
    print(gcd(48, 18))
}
```

Ausgabe: `Point{x: 6, y: 2}`, `6`.

```dif error R6
struct Point {
    int x
    int y
}

void shift(mut Point p, int dx) {
    p.x += dx
}

void main() {
    Point b = Point{x: 1, y: 2}
    shift(b, 5)
    print(b)
}
```

```
main.dif:12:11: error[R6]: `shift` takes `p` as mut
  hint: shift(mut b, 5)
```

```dif error R6
struct Point {
    int x
    int y
}

void nudge(Point p) {
    p.x += 1
}
```

```
main.dif:7:5: error[R6]: the change to `p` is lost: `p` is a copy
  hint: take it as mut: void nudge(mut Point p)
```

```dif error R6
struct Point {
    int x
    int y
}

void swap_x(mut Point a, mut Point b) {
    int t = a.x
    a.x = b.x
    b.x = t
}

void main() {
    Point p = Point{x: 1, y: 2}
    swap_x(mut p, mut p)
    print(p)
}
```

```
main.dif:14:19: error[R6]: `p` goes into this call changeable twice
  hint: pass it changeable only once
```

```dif error R6
struct Point {
    int x
    int y
}

int sum(mut Point p) {
    return p.x + p.y
}

void main() {
    Point p = Point{x: 1, y: 2}
    print(sum(mut p))
}
```

```
main.dif:6:9: error[R6]: `p` is never changed; remove mut
  hint: int sum(Point p)
```

### R7 — Methoden

**Regel.** Eine Methode ist eine Funktion an einem Typ: `R T.name(params)`. In ihr ist `self` immer der Ort, an dem sie aufgerufen wird, und darf geändert werden. Methoden, die `self` nicht ändern, gehen auch auf Ausdrücken.

```ebnf
method_decl = type IDENT "." IDENT "(" [ params ] ")" block ;   (* R Typ.name(…) *)
```

Ein Methodenaufruf `x.name(…)` ist ein `postfix` aus §2.

Teil 2a legt fest:

- Methoden gibt es für die `struct`- und `enum`-Typen des Moduls; für andere Typen ist `R7`. Der Name steht in der Form `Typ.name`, darum findet `grep "Point.scale"` die Definition. `self` steht nie in der Parameterliste.
- In der Methode ist `self` der Ort, auf dem sie aufgerufen wird, vom Typ `Typ`. Sie darf ihn lesen und ändern (`self.x *= k`, `self = Point{x: 0, y: 0}`). `self` außerhalb einer Methode ist `R7`.
- Der Compiler leitet ab, ob eine Methode `self` ändert: sie ändert (R6) einen Ort mit der Wurzel `self`, also etwa `self.x = 1`, `shift(mut self.pos, 1)` oder `self.inner.bump()` mit einer Methode `bump`, die `self` ändert. Die Ableitung geht über alle Aufrufe, auch rekursive.
- Eine Methode, die `self` ändert, braucht vor dem Punkt einen Ort (R6), dessen Wurzel keine `const`-Variable ist; auch Teilorte wie `scene.player.move(1)`. Vor einem anderen Ausdruck ist sie `R7` am Aufruf, Meldung `` `Counter.inc` changes self and needs a variable ``, Hinweis `store the value in a variable first`. Am Aufruf steht kein `mut`: der Methodenaufruf zeigt schon, dass der Ort vor dem Punkt gemeint ist. Für R6 zählt der Aufruf als Lesen und Ändern dieses Ortes.
- Eine Methode, die `self` nicht ändert, geht auf jedem Ausdruck: `fresh().total()`, `a.moved(1).moved(2)`.
- Ein unbekannter Methodenname ist `R4`, mit dem ähnlichsten Namen als Hinweis.

```dif
struct Point {
    int x
    int y
}

Point Point.moved(int dx) {
    Point q = self
    q.x += dx
    return q
}

void Point.scale(int k) {
    self.x *= k
    self.y *= k
}

void shift(mut Point p, int dx) {
    p.x += dx
}

void main() {
    Point a = Point{x: 1, y: 2}
    Point b = a
    b.x = 10
    shift(mut b, 5)
    b.scale(2)
    Point c = b.moved(1)
    print("{a} {b} {c}")
}
```

Ausgabe: `Point{x: 1, y: 2} Point{x: 30, y: 4} Point{x: 31, y: 4}`.

```dif error R7
struct Counter {
    int n
}

void Counter.inc() {
    self.n += 1
}

Counter fresh() {
    return Counter{n: 0}
}

void main() {
    fresh().inc()
}
```

```
main.dif:14:5: error[R7]: `Counter.inc` changes self and needs a variable
  hint: store the value in a variable first
```

### R8 — Keine Typmischung

**Regel.** Typen mischen sich nie von selbst. Operatoren verlangen gleiche Typen; `T(x)` wandelt exakt um oder bricht ab; absichtlich runden mit `round()`/`trunc()`; Text baut man mit `"{x}"`. Ausdrücke nur aus Literalen rechnen im Typ ihrer Stelle; ohne Stelle ist ein Literal ohne Punkt `int`, mit Punkt `f64`; Float-Literale werden auf den nächsten Wert gerundet. „Steht beim Kompilieren fest“ heißt: nur Literale und `const`.

**Grammatik:** die Ausdrücke aus §2; die Regel prüft ihre Typen.

Teil 1 legt fest:

| Operator | Operanden | Ergebnis |
|---|---|---|
| `+ - * / %`, `+% -% *%` | `int, int` | `int` |
| `& \| ^ << >>` | `int, int` | `int` |
| `~`, Vorzeichen `-` | `int` | `int` |
| `+` (Verkettung) | `str, str` | `str` |
| `== !=` | gleicher Typ (`int`, `bool`, `str`) | `bool` |
| `< <= > >=` | `int, int` oder `str, str` | `bool` |

- Jede andere Kombination ist `R8`, z. B. `"41" + 1` mit Hinweis `build text with interpolation: "{a}{b}"`. Die Operanden von `&&`, `||` und `!` prüft R9.
- `+` verbindet zwei `str`. Werte anderer Typen kommen nur über `"{x}"` in einen Text; das meint die Regel mit „Text baut man mit `"{x}"`“.
- **Stelle** eines Ausdrucks ist der Typ, den sein Ziel verlangt: die deklarierte Variable, der Parameter, der Rückgabetyp, der andere Operand eines Vergleichs. Ohne Stelle (`print(7 / 2)`) ist ein Literal `int`.
- **Ausdrücke nur aus Literalen und `const`** (deren Wert selbst so feststeht) rechnet der Compiler exakt aus, ohne Überlauf zwischendurch, mit der Division des Typs ihrer Stelle (`int k = 7 / 2` ist `3`). Erst das Ergebnis muss in den Typ passen, sonst `R8` (`int x = 9223372036854775808`, Hinweis `the value does not fit in int`). Darum ist `-9223372036854775808` ein gültiges `int`. Bit-Operatoren und Shifts rechnen dabei wie zur Laufzeit auf 64 Bit (R29), `1 << 63` ist also `-9223372036854775808`.
- Ein Literal, dessen Art nicht zur Stelle passt (`bool b = 1`), ist `R8`.
- **Rückgabe auf jedem Weg.** Eine Funktion mit Rückgabetyp (nicht `void`) endet auf jedem Weg mit `return` und einem Wert; sonst `R8` an ihrem Namen, Meldung `` `f` does not return a value on every path ``, Hinweis `end it with return, e.g. return 0` (mit `0`, `false`, `""` oder `…` je nach Typ). Ein Block *endet so*, wenn seine letzte Anweisung `return` ist, ein `if` mit `else`, dessen Zweige alle so enden, ein vollständiger `switch` (R10), dessen Zweige alle so enden, oder `while true` ohne `break` für diese Schleife. `return` ohne Wert in einer solchen Funktion und `return` mit Wert in einer `void`-Funktion sind `R8`.

```dif snippet
int a = 7 / 2
str s = "a ist {a}"
print(s + "!")
```

```dif error R8
void main() {
    print("41" + 1)
}
```

```
main.dif:2:16: error[R8]: cannot apply `+` to str and int
  hint: build text with interpolation: "{a}{b}"
```

### R9 — Bedingungen sind `bool`

**Regel.** Bedingungen sind `bool`, nichts anderes.

```ebnf
cond = expr ;      (* in if_stmt, while_stmt und als Operand von &&, ||, ! *)
```

Bedingungen stehen in `if`, `else if`, `while` und als Operanden von `&&`, `||`, `!`. Ein anderer Typ ist `R9`; es gibt kein „wahr-ish“ für Zahlen oder Texte. `&&`, `||` und `!` liefern `bool`; `&&` und `||` werten ihren rechten Operanden nur aus, wenn er das Ergebnis noch ändern kann.

```dif snippet
int n = 3
while n > 0 && n != 7 {
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

**Regel.** `switch` deckt jeden möglichen Wert ab: alle Varianten oder `default`. Jeder `case` ist ein eigener Block und endet von selbst. Varianten eines `enum` schreibt man mit ihrem Typ; `T!E` und `?T` sind keine `enum`s, sie haben die eingebauten Formen `ok`, `err`, `some`, `none`.

```ebnf
switch_stmt = "switch" expr "{" NL { NL } { case } [ default ] "}" ;
case        = "case" pattern { "," pattern } ":" NL { stmt NL { NL } } ;
default     = "default" ":" NL { stmt NL { NL } } ;
pattern     = [ "-" ] INT | STRING | IDENT | IDENT "." IDENT [ "(" binding { "," binding } ")" ] ;
binding     = type IDENT | "_" ;
```

Teil 2a legt fest:

- `switch` geht über `int`, `str` und `enum`. Über einem anderen Typ ist es `R10`, Meldung `` cannot switch over bool `` (bzw. den Typ), für `bool` mit Hinweis `use if`.
- Nach `{` und nach jedem `case …:` und `default:` folgt ein Zeilenumbruch. Eine Anweisung in derselben Zeile (`case 0: return 1`, `switch s { … }` in einer Zeile) ist `R3`, Meldung `the statements of a case start on the next line`.
- Ein `case` nennt einen oder mehrere Werte. Seine Anweisungen stehen in den folgenden Zeilen bis zum nächsten `case`, `default` oder `}`; sie bilden einen eigenen Block. Es gibt kein Durchfallen; ein `case` ohne Anweisungen tut nichts.
- **Werte über `int` und `str`:** Literale oder `const`-Namen dieses Typs. Ein Wert anderen Typs ist `R8`, ein unbekannter Name `R4`.
- **Werte über einem `enum E`:** nur Varianten, immer qualifiziert (`E.V`). Eine unqualifizierte Variante (`case Free:`) ist `R10`, Meldung `variants are written with their type`, Hinweis `case Seat.Free:`. Ein Literal, ein `const` oder eine Variante eines anderen `enum` ist `R8`.
- **Bindungen.** `case Shape.Rect:` passt auf jede `Rect`, gleich welche Werte sie trägt. Wer Werte braucht, bindet sie in Klammern, für jeden Wert der Variante einen Eintrag in Reihenfolge: Typ und Name, oder `_` zum Überspringen: `case Shape.Rect(int w, _):`. Falsche Anzahl oder falscher Typ ist `R8`, Meldung `` `Shape.Rect` has the values int w, int h ``. Klammern nur aus `_` sind `R10`, Hinweis `case Shape.Rect:`. Bindungen gibt es nur, wenn der `case` genau einen Wert nennt (`R10`). Gebundene Namen gelten im Block des `case` und folgen R4.
- Über einem `enum` ist jede Variante behandelt oder es steht ein `default`; sonst `R10` am `switch`, Meldung `switch does not handle ` mit den fehlenden Varianten in Deklarationsreihenfolge, getrennt durch `, ` (`switch does not handle Seat.Taken, Seat.Reserved`), Hinweis `add case Seat.Taken: …, or default:` mit der ersten fehlenden. Über `int` und `str` ist `default` Pflicht (`R10`). Ein `default`, der nie erreicht wird, weil alle Varianten behandelt sind, ist `R10`.
- `default` steht als letzter Zweig. Ein Wert, der schon in einem früheren `case` steht, ist `R10` mit der Position des ersten.
- `break` steht nie direkt in einem `switch` (R11); `continue` und `return` dürfen dort stehen.
- Endet jeder Zweig eines vollständigen `switch` mit `return`, endet auch der `switch` dort (für „`return` auf jedem Weg“).

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

str classify(int n) {
    switch n {
    case 0:
        return "null"
    case 1, 2, 3, 4, 5, 6, 7, 8, 9:
        return "einstellig"
    default:
        return if n < 0 { "negativ" } else { "groß" }
    }
}

void main() {
    print(area(Shape.Rect(3, 4)))
    print(classify(7))
    print(Shape.Square(2))
}
```

Ausgabe: `12`, `einstellig`, `Shape.Square(2)`.

```dif error R10
enum Seat { Free, Taken }

str symbol(Seat s) {
    switch s {
    case Seat.Free:
        return "."
    }
}
```

```
main.dif:4:5: error[R10]: switch does not handle Seat.Taken
  hint: add case Seat.Taken: …, or default:
```

Ein unvollständiger `switch` meldet nur `R10`, nicht zusätzlich das fehlende `return`.

### R11 — Schleifen

**Regel.** Schleifen: `for T x in …` (über `range(n)`, `range(a, b)`, Listen, `io.lines()`, Kanäle), `for int i, T x in xs` (Index und Element), `for K k, V v in m` (Schlüssel und Wert) und `while cond`. `break` und `continue` gehören zur innersten Schleife; `break` steht nie direkt in einem `switch`, weil es dort in C und Go den `switch` verließe.

```ebnf
while_stmt = "while" expr block ;
for_stmt   = "for" type IDENT [ "," type IDENT ] "in" expr block ;
```

Teil 1 legt fest:

- `range(n)` liefert die Ganzzahlen `0 … n-1`, `range(a, b)` die Ganzzahlen `a … b-1`; ist das Ende nicht größer als der Anfang, läuft die Schleife nicht. Die Grenzen werden einmal vor der Schleife ausgewertet. In Teil 1 steht `range` nur als Quelle einer `for`-Schleife.
- Die Schleifenvariable ist eine neue Variable im Schleifenblock (R4); ändert der Rumpf sie, wirkt das nur bis zum nächsten Durchlauf.
- `break` verlässt die innerste Schleife, `continue` beginnt ihren nächsten Durchlauf. Außerhalb einer Schleife sind beide `R11`.
- `break` direkt in einem `switch` (nicht in einer Schleife darin) ist `R11`, Meldung `` `break` directly inside switch ``, Hinweis `leave the loop after the switch, or return`.
- Die C-Schleife `for (…; …; …)` ist `R11`, Hinweis mit der passenden `range`-Form.

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
main.dif:2:5: error[R11]: C-style for loop
  hint: for int i in range(3)
```

### R12 — Scheitern steht im Typ

**Regel.** Kann eine Funktion scheitern, steht es im Rückgabetyp: `T!E` heißt Wert oder `err(e)`, `?T` heißt Wert oder `none`. Erfolg gibt man einfach zurück. `try x` reicht einen Fehler gleichen Typs weiter; `try x or E.V(…)` übersetzt ihn vorher in einen eigenen.

*Ausarbeitung in Teil 2 (M2).*

### R13 — Jeder Aufruf hat eine Wirkung

**Regel.** Ein Ergebnis vom Typ `T!E` oder `?T` wird immer verwendet: `try`, `??`, `switch` oder bewusst `_ =`. Jeder andere Aufruf braucht eine Wirkung: sein Ergebnis wird verwendet, oder er ändert etwas (`mut`, `self`), macht Ein-/Ausgabe, ruft C oder startet einen Task. Ein Aufruf ohne Wirkung ist ein Fehler.

**Grammatik:** `call_stmt` und `discard` aus R3.

Teil 1 und Teil 2a legen fest:

- Ein Aufruf als Anweisung, dessen Ergebnis nicht `void` ist, ist `R13`, Hinweis `use it, or discard it explicitly with _ = …` — außer die Funktion hat eine Wirkung (ab Teil 2a).
- **Wirkung (Teil 2a).** Eine Funktion oder Methode hat eine Wirkung, wenn sie einen `mut`-Parameter oder `self` ändert, `print` aufruft oder eine Funktion mit Wirkung aufruft; der Compiler leitet das über alle Aufrufe hinweg ab (auch rekursiv). Ein Aufruf mit Wirkung darf als Anweisung stehen, auch wenn er einen Wert liefert. Ergebnisse vom Typ `T!E` und `?T` werden trotzdem immer verwendet (Teil 2b).
- `_ = f()` verwirft ein Ergebnis bewusst.
- Ein Aufruf einer `void`-Funktion als Anweisung ist erlaubt.

```dif
int square(int n) {
    return n * n
}

void main() {
    int a = square(3)
    _ = square(4)
    print(a)
}
```

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

### R14 — Abbruch bei Programmierfehlern

**Regel.** Programmierfehler brechen mit Meldung und Position ab: Index außerhalb, Überlauf, Division durch 0, ungenaue Umwandlung, `assert`. Immer, auch im Release.

**Grammatik:** keine eigene; die Regel gilt zur Laufzeit für die Operatoren aus §2.

Teil 1 legt fest (alle für `int`, siehe R29):

| Fall | Meldung |
|---|---|
| Überlauf bei `+ - *` und Vorzeichen `-` | `integer overflow in +` (bzw. `-`, `*`) |
| Division oder `%` durch 0 | `division by zero` |
| `INT_MIN / -1` | `integer overflow in /` |
| Shift mit Weite < 0 oder ≥ 64 | `shift by <n> out of range 0..63` |

- Die Position ist die des Operators. Der Abbruch läuft ohne `defer` (R23) und endet mit Exit-Code 2.
- `+% -% *%` rechnen bewusst modulo 2⁶⁴ und brechen nie ab.
- Steht der Abbruch schon beim Kompilieren fest (nur Literale und `const`, z. B. `1 / 0`, `1 << 64`), meldet ihn der Compiler als `error[R14]`. Ein Überlauf ist dort kein Abbruch: Konstanten rechnen exakt, und ein Ergebnis, das nicht in `int` passt (auch `INT_MIN / -1`), ist `R8` (siehe R8).

```dif snippet
int big = 9223372036854775807
print(big +% 1)
```

Ausgabe: `-9223372036854775808`.

```dif panic "integer overflow in +"
int big = 9223372036854775807
print(big + 1)
```

```
panic: integer overflow in + (main.dif:3:15)
```

### R15 — `if` als Ausdruck

**Regel.** `if` mit `else` liefert einen Wert, wenn jeder Zweig mit einem Ausdruck endet.

```ebnf
if_expr     = "if" expr value_block { "else" "if" expr value_block } "else" value_block ;
value_block = "{" { NL } { stmt NL { NL } } expr { NL } "}" ;
```

Teil 2a legt fest:

- `if` als Ausdruck steht überall, wo ein Ausdruck stehen kann, außer am Anfang einer Anweisung (dort ist es die `if`-Anweisung aus R3) und ungeklammert in einer Kopfzeile (R3).
- Jeder Zweig ist ein Block, dessen letzte Zeile ein Ausdruck ist; davor dürfen Anweisungen stehen. Der Wert des Zweigs ist dieser Ausdruck; ein `if`-Ausdruck in der letzten Zeile ist erlaubt.
- `else` ist Pflicht: `R15` am `if`, Meldung `if as a value needs else`, Hinweis mit `else { … }`. Endet ein Zweig nicht mit einem Ausdruck mit Wert (sondern mit einer Anweisung oder einem `void`-Aufruf), ist das `R15`, Meldung `this branch of if gives no value`. Alle Zweige haben denselben Typ, sonst `R8`, Meldung `` the branches of if give int and str ``; Literale bekommen den Typ ihrer Stelle (R8).
- Der Ausdruck endet mit dem `}` seines letzten Zweigs: `if c { 1 } else { 2 } + 3` addiert 3 zum gewählten Wert.
- Ausgewertet wird nur der gewählte Zweig.

```dif snippet
int n = -3
str sign = if n < 0 { "negativ" } else if n == 0 { "null" } else { "positiv" }
int abs = if n < 0 {
    int m = -n
    m
} else {
    n
}
print("{sign} {abs}")
```

Ausgabe: `negativ 3`.

```dif error R15
void main() {
    int n = 3
    str s = if n > 0 { "positiv" }
    print(s)
}
```

```
main.dif:3:13: error[R15]: if as a value needs else
  hint: if n > 0 { "positiv" } else { … }
```

### R16 — Module

**Regel.** Eine Datei ist ein Modul. `use name` lädt `name.dif` neben der Datei, `use std.x` die Standardbibliothek, `use c "x.h"` einen C-Header; `as` gibt jedem Modul einen anderen Namen; zyklische `use` sind ein Fehler. `pub` vor Typen, Funktionen, Feldern und Methoden macht sie sichtbar. Funktionen ohne `self` gehören in ein Modul, nicht an einen Typ (`c.to_str(p)`, `chan.new[int](64)`).

*Ausarbeitung in Teil 2/3.*

### R17 — C, Pointer, `unsafe`

**Regel.** Pointer gibt es nur in `unsafe { }` und in C-Deklarationen; dort gelten C-Regeln, ein roher Pointer `*T` darf wie in C NULL sein (`p.is_null()`). Strings und Griffe, die C zurückgibt, sind `?T`; Parameter an C sind nicht optional, außer eine `extern`-Deklaration erlaubt `?`. Pointer auf opake C-Typen sind Griffe; nennt der Import einen Freigeber (`release T with f`), werden sie gezählt und `f` läuft genau einmal. Strings an C gelten nur für die Dauer des Aufrufs, ein NUL-Byte darin bricht ab. Nicht genannte Felder eines C-structs sind 0.

*Ausarbeitung mit M4 (`extern`) und v0.1 (`use c`).*

### R18 — Tests

**Regel.** Tests stehen neben dem Code: `test "name" { }`, geprüft mit `expect(cond)`. `dif test` führt sie aus, auch die Codebeispiele in `///`-Kommentaren.

*Ausarbeitung in Teil 2 (M2).*

### R19 — Tasks *(nach v0)*

**Regel.** `spawn f(args)` startet `f` parallel und liefert `task[T]`; `t.wait()` holt das Ergebnis. Argumente gehen ohne `mut` hinein. Ein Task lebt nur in der Funktion, die ihn gestartet hat: verlässt sie ihren Rumpf, werden ihre noch laufenden Tasks erst abgebrochen, dann abgewartet. Ein Abbruch im Task kommt beim Warten an. Nach `t.cancel()` kehrt jeder blockierende std-Aufruf im Task mit Fehler zurück. `main` läuft immer im Hauptthread des Betriebssystems, Tasks nie.

*Ausarbeitung nach v0; `spawn` ist reserviert.*

### R20 — Kanäle *(nach v0)*

**Regel.** Tasks tauschen Werte nur über Kanäle aus: `chan[T]`, `ch.send(v)`, `for T x in ch` (endet nach `ch.close()`). Ein Kanal ist ein Griff: Kopien zeigen auf denselben Kanal. Gesendete Werte gehören danach dem Empfänger. Griffe zählen atomar; ein C-Griff darf nur in einen Task, wenn sein Import `send T` sagt.

*Ausarbeitung nach v0.*

### R21 — Parallele Schleife *(nach v0)*

**Regel.** `parallel for T x in xs { … yield v }` verteilt die Durchläufe auf alle Kerne und liefert die `yield`-Werte als Liste in Eingabereihenfolge. Der Rumpf liest Variablen von außen nur.

*Ausarbeitung nach v0; `parallel` und `yield` sind reserviert.*

### R22 — Geteilte Werte *(nach v0)*

**Regel.** `share(x)` friert einen Wert ein und liefert `shared[T]`: nur lesbar, und ohne Kopie an beliebig viele Tasks übergebbar. Ändern heißt, eine neue Version zu bauen und erneut zu teilen.

*Ausarbeitung nach v0.*

### R23 — `defer`

**Regel.** `defer X` führt X am Ende des umgebenden Blocks aus, als stünde es dort, auf jedem Weg hinaus (Ende, `return`, `try`, `break`); Argumente werden erst dann ausgewertet. Mehrere `defer` laufen in umgekehrter Reihenfolge. Ein Ergebnis vom Typ `T!E` braucht `_ =`. Ein Abbruch (R14) beendet das Programm ohne `defer`.

*Ausarbeitung mit M4.*

### R24 — C-Callbacks mit Kontext

**Regel.** Erwartet C ein Paar aus Funktion und `void *user`, übergibt man eine dif-Funktion und, wenn sie Zustand braucht, eine Variable; die Funktion bekommt die Variable als ersten Parameter, mit `mut`, wenn sie sie ändert (R6). Keine Funktion: `none`. Der Kontext gilt, solange der Aufruf läuft, in den die Variable mit `mut` hineinging; ein Callback außerhalb dieses Aufrufs oder aus einem anderen Thread bricht mit Meldung ab.

*Ausarbeitung mit v0.1.*

### R25 — Benannte Parameter

**Regel.** Parameter mit Default stehen hinten und werden beim Aufruf beim Namen genannt (`name: wert`), alle anderen der Reihe nach.

*Ausarbeitung mit M3.*

### R26 — Funktionswerte und Blöcke

**Regel.** Ein Funktionswert hat den Typ `R(T1, T2)` und wird geschrieben wie eine Funktion ohne Namen: `R(T1 a, T2 b) { … }`. Ein Literal steht nur direkt als Argument eines Aufrufs (nie in einer Variable, einem struct, einer Liste, in `spawn` oder als Typargument) und lebt nur während dieses Aufrufs: es darf ringsum lesen und ändern. Ein Literal ohne Parameter darf als Block `{ … }` direkt hinter der schließenden Klammer stehen; `return` und `try` darin beenden den Block. Benannte Funktionen und Varianten-Konstruktoren haben keine Umgebung und sind gewöhnliche Werte, die man speichern darf. Ändert ein Funktionswert einen Parameter, steht `mut` in seinem Typ.

*Ausarbeitung mit M3.*

### R27 — Generics

**Regel.** Typparameter stehen in eckigen Klammern hinter dem Namen und werden beim Aufruf meist abgeleitet; jede Instanz wird eigener Code. Eine Einschränkung steht vor dem Typparameter wie ein Typ vor dem Namen: `equal`, `ordered`, `hashable`, `text`.

*Ausarbeitung mit M3.*

### R28 — Fähigkeiten

**Regel.** Jeder Typ ohne Funktionswert-Felder ist `equal` (`==`) und `text` (Ausgabe, `"{x}"`); `ordered` (`<`) sind Zahlen und `str`; `hashable` ist alles, was `equal` ist. Griffe vergleichen ihre Identität, Maps sind gleich, wenn ihre Einträge gleich sind.

Teil 1 legt die Textform fest: `int` dezimal mit `-` für negative Zahlen, `bool` als `true`/`false`, `str` unverändert. `str` vergleicht `<` bytweise (UTF-8).

Teil 2a ergänzt:

- Ein `struct` ist `equal`, wenn alle Felder es sind; `==` vergleicht Feld für Feld. Ein `enum` vergleicht Variante und Werte. `<` gibt es für beide nicht (`R8`).
- Die Textform sieht aus wie das Literal: `Point{x: 1, y: 2}` (Felder in Deklarationsreihenfolge), `Seat.Taken`, `BookErr.NoSuchSeat(3, 5)`. Ein `str` darin steht in Anführungszeichen mit Escapes (`Person{name: "Ada"}`); allein ausgegeben bleibt ein `str` unverändert.

### R29 — Zahlen

**Regel.** Zahlen: Ganzzahl-`/` rundet Richtung 0, `%` hat das Vorzeichen des Dividenden; Floats folgen IEEE 754 (`inf`, `NaN`, `NaN == NaN` ist false); Ganzzahl-Division durch 0, `INT_MIN / -1` und Shifts ab der Bitbreite brechen ab. Ausdrücke und Argumente werden von links nach rechts ausgewertet.

**Grammatik:** `INT` aus §1.4 und die Operatoren aus §2.

Teil 1 legt den Ganzzahl-Teil fest:

- `int` ist immer 64 Bit mit Zweierkomplement, Bereich `-9223372036854775808 … 9223372036854775807`.
- `a / b` rundet Richtung 0, `a % b` hat das Vorzeichen von `a`, es gilt `(a / b) * b + a % b == a`. `INT_MIN % -1` ist `0`.
- `<<` und `>>` verschieben um `0 … 63` Stellen; `>>` ist arithmetisch (füllt mit dem Vorzeichen). `<<` rechnet auf den 64 Bits: was über Bit 63 hinausgeschoben wird, geht verloren, ohne Abbruch, beim Kompilieren wie zur Laufzeit. Andere Weiten brechen ab (R14).
- `+% -% *%` rechnen modulo 2⁶⁴.
- Auswertungsreihenfolge: Operanden von links nach rechts, Argumente von links nach rechts, bei einer Zuweisung erst der Ort, dann der Wert.

```dif snippet
print(-7 / 2)
print(-7 % 3)
print(1 << 62)
```

Ausgabe: `-3`, `-1`, `4611686018427387904`.

```dif panic "shift by 64 out of range 0..63"
int w = 64
print(1 << w)
```

```
panic: shift by 64 out of range 0..63 (main.dif:3:13)
```

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
