# difficulty — Spezifikation

Stand: 2026-11-25 · Version: v0, Teil 1 („Hallo, nativ“), Teil 2a (Daten), Teil 2b (Fehler und Tests), Teil 2c (Sammlungen, Zahlen, Text, Ein- und Ausgabe) und Teil 2d (Funktionsliterale, Block-Argument) · Begründungen und Fahrplan: [PLAN.md](../PLAN.md)

Diese Spezifikation ist nach den Regeln R1–R31 gegliedert. Ausgearbeitet sind R1–R15, R18, R26, R28–R31 und von R16 die Zeilen `use std.…`. Alle übrigen stehen mit ihrem vollständigen Wortlaut da und werden mit den nächsten Teilen ausgearbeitet.

## 0. Wie diese Spezifikation zu lesen ist

**Grammatik** steht in EBNF: `a b` Folge, `a | b` Auswahl, `[ a ]` optional, `{ a }` beliebig oft, `"x"` wörtlich, `NL` Zeilenende (siehe §1.3). Jede Produktion steht genau einmal; andere Abschnitte verweisen darauf.

**Kanonische Form.** Wo der Compiler mehrere Schreibweisen annimmt (Feldreihenfolge im Struct-Literal, überflüssige Klammern um einen Elementtyp wie in `[](?int)`, abschließendes Komma in mehrzeiligen Listen, Trenner der `enum`-Varianten, `x` statt `some(x)` an einer `?T`-Stelle, die ein Ziel ist, R12; `or F.V` statt `or err(E e): F.V(e)` hinter `try`, R12), legt `dif fmt` die eine kanonische Form fest; der Compiler meldet dafür keinen Fehler. Ein Block aus genau einer Anweisung ohne eigenen Block (`if`, `else`, `while`, `for`, Rumpf eines Funktionsliterals oder Blocks, R26) steht einzeilig, wenn die Zeile dann höchstens 100 Zeichen lang ist (`if c { return e }`, `} else { x = 1 }`, `for int i in xs { total += i }`), sonst mehrzeilig; alles andere steht mehrzeilig. Die Fälle von `switch` sind keine Blöcke und bleiben davon unberührt. Der Compiler nimmt beide Formen an.

### 0.1 Diagnosen

Ein Verstoß gegen eine Regel trägt deren Nummer: `error[R13]`. Schreibfehler, die keine Regel betreffen, tragen `L` (Lexik) oder `S` (Grammatik):

| Code | Bedeutung |
|---|---|
| `L1` | ungültiges UTF-8 oder ein Zeichen, das außerhalb von Strings und Kommentaren nicht vorkommen darf |
| `L2` | Blockkommentar `/* … */` |
| `L3` | `;` |
| `L4` | Ganzzahl-Literal größer als 2⁶⁴ − 1 |
| `L5` | String reicht über das Zeilenende |
| `L6` | unbekannte Escape-Sequenz, `\u{…}` ohne gültigen Unicode-Codepoint, einzelnes `}` oder nicht geschlossenes `{` im String |
| `L7` | `++` oder `--` |
| `L8` | Zahl-Literal in falscher Form: führende Null (`010`), Präfix `0X`/`0B`, `_` direkt nach dem Präfix oder im Exponenten, fehlende oder falsche Ziffern (`0x`, `0b12`, `1e`), Buchstaben direkt nach der Zahl |
| `L9` | rune-Literal in falscher Form: leer (`''`), mehr als ein Zeichen (`'ab'`) oder nicht geschlossen |
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

Beispiele gibt es in sechs Arten, die `dif test` als Doc-Tests ausführt:

- ` ```dif ` — ein vollständiges, gültiges Modul.
- ` ```dif snippet ` — Anweisungen, die in `void main() { … }` eingebettet werden: Zeile 1 des Ausschnitts ist Zeile 2 der Datei, um 4 Leerzeichen eingerückt. Positionen in Meldungen beziehen sich auf die Datei `main.dif`.
- ` ```dif error R4 ` — ein Modul, das genau diese Diagnose liefern muss (und keine andere). Geprüft wird wie mit `dif check`; ein fehlendes `main` zählt dort nicht.
- ` ```dif panic "message" ` — ein Ausschnitt wie bei `snippet`, der mit genau dieser Meldung abbrechen muss.
- ` ```dif fails "text" ` — ein Modul, dessen `void!E main()` mit einem Fehler endet, dessen Textform (R28) genau `text` ist: Exit-Code 1, stderr `error: text`.
- ` ```dif test ` — ein Modul mit `test`-Blöcken (R18); `dif test main.dif` muss genau die Ausgabe des folgenden einfachen Blocks liefern.

### 0.3 Abbruch

Programmierfehler (R14) beenden das Programm sofort mit

```
panic: <message> (<file>:<line>:<column>)
```

auf stderr und Exit-Code 2. Ein Fehler, der aus `void!E main()` herauskommt, endet mit Exit-Code 1 (Format in R12).

### 0.4 Exit-Codes von `dif`

| Code | `dif check` | `dif run`, `dif build`, `dif test` |
|---|---|---|
| 0 | keine Diagnose | gebaut, bei `dif run` gelaufen: der Exit-Code des Programms; bei `dif test` alle Tests bestehen |
| 1 | Diagnosen | Diagnosen (die Datei lässt sich nicht übersetzen); bei `dif test` scheitert ein Test (R18); bei `dif run` der Exit-Code des Programms, 1 bei einem Fehler aus `main` |
| 2 | falscher Aufruf, Datei nicht lesbar | falscher Aufruf, Datei nicht lesbar, kein C-Compiler gefunden (`$CC`, sonst `cc`, `gcc`, `clang`); bei `dif test` Diagnosen (R18); bei `dif run` ein Abbruch des Programms (R14) oder ein Signal |
| 3 | – | **interner Fehler:** der C-Compiler lehnt das C ab, das `dif` erzeugt hat. Das ist ein Fehler von `dif`, nicht des Programms: stderr nennt die Meldung des C-Compilers, es entsteht kein Programm und kein Test läuft |

`dif run` gibt den Exit-Code des Programms weiter, auch den von `os.exit(n)` (§4.6); wer den Code 3 des Compilers von einem Code 3 des Programms trennen will, nimmt `dif build` und startet das Programm selbst. Das erzeugte C braucht die Mathematikbibliothek (`round`, `floor`, `sqrt`, `fmod`); `dif` übergibt dem C-Compiler `-lm`.

---

## 1. Lexik

### 1.1 Zeichen

Quelltext ist UTF-8. Ungültiges UTF-8 ist `L1`. Nicht-ASCII-Zeichen sind nur in Strings, rune-Literalen und Kommentaren erlaubt, sonst `L1`.

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
escape   = "\" ( "n" | "t" | "r" | "\" | '"' | "'" | "{" | "}" | "0" ) | "\u{" hexdigit { hexdigit } "}" ;
interp   = "{" expr [ ":" format ] "}" ;
format   = "g" | "." digit { digit } "f" ;
RUNE     = "'" ( rchar | escape ) "'" ;
rchar    = (* jedes Zeichen außer "'", "\" und dem Zeilenende *) ;
```

- `_` in Zahlen dient nur der Lesbarkeit: `1_000_000`. Ein Ganzzahl-Literal größer als 2⁶⁴ − 1 ist `L4`; ob ein kleineres an seiner Stelle passt, prüft R8.
- Eine Ganzzahl mit führender Null (`010`) ist `L8`, Hinweis `write 10; octal literals do not exist`. `0` allein und `0.5` sind erlaubt.
- Jede andere Form außerhalb der Grammatik ist ebenfalls `L8` und bleibt ein Token: `0X10`, `0x_ff`, `0x`, `0b102`, `1e`, `1e1_0`, `12ab`.
- Ein String reicht nicht über das Zeilenende hinaus (`L5`). Eine unbekannte Escape-Sequenz ist `L6`. `\{` und `\}` schreiben eine wörtliche Klammer; ein einzelnes `}` ohne `\` ist `L6`.
- `{expr}` setzt den Wert von `expr` als Text ein; `expr` muss `text` sein (R28). Im Ausdruck einer Interpolation stehen keine Strings und keine geschweiften Klammern (`S1`).

Teil 2c ergänzt:

- **Zahl-Literale.** `INT` und `FLOAT` stehen ohne Typ da; welchen Typ sie haben und ob sie passen, legt R8 fest. Ein Literal mit Punkt oder Exponenten ist ein Float-Literal, eines ohne beides ein Ganzzahl-Literal. Eine Methode auf einem Zahl-Literal steht in Klammern, `(5).abs()`; `5.abs()` ist `L8`, Hinweis `put the number in parentheses: (5).abs()`.
- **rune-Literal.** `'a'` ist ein einzelner Unicode-Codepoint vom Typ `rune`; `'\n'`, `'\''`, `'\u{1F600}'` und `'é'` sind es ebenso. Die Escapes sind dieselben wie im String; `'"'` und `'{'` brauchen keinen. Ein Literal, das leer ist (`''`), mehr als einen Codepoint enthält (`'ab'`, auch ein Buchstabe mit angehängtem Akzent) oder vor dem Zeilenende nicht schließt, ist `L9` an dem öffnenden `'`. Meldung bei leerem oder zu langem Inhalt: `` a rune literal holds exactly one character ``, Hinweis `use a string for more than one character: "ab"` (bei `''`: `write a character between the quotes: 'a'`); bei fehlendem Ende `` rune literal is not closed ``, Hinweis `add ' at the end`. `\u{…}` mit einem Wert über 10FFFF oder einem Surrogat (D800 … DFFF) ist `L6`, Meldung `` not a Unicode character ``, im String wie im rune-Literal.
- **Formatangabe.** `{x:g}` und `{x:.2f}` formatieren einen `f64` (R29); `.` nennt höchstens 20 Nachkommastellen (`S1`, Meldung `` precision at most 20 ``). Jede andere Angabe (`{x:q}`, `{x:.f}`, `{x:5}`) ist `S1` am ersten Zeichen nach `:`, Meldung `` unknown format `q` ``, Hinweis `formats are g and .Nf, e.g. {x:.2f}`. Eine Formatangabe an einem Wert, der kein `f64` ist, ist `R8` (R29).

```dif snippet
rune a = 'a'
print(a)
print(int('\n'))
print(int('\u{1F600}'))
print('\'')
```

Ausgabe: `a`, `10`, `128512`, `'`.

```dif error L9
void main() {
    rune r = 'ab'
    print(r)
}
```

```
main.dif:2:14: error[L9]: a rune literal holds exactly one character
  hint: use a string for more than one character: "ab"
```

### 1.5 Wörter

**Schlüsselwörter** (25), nie als Namen nutzbar:

```
const struct enum if else while for in break continue return switch case default
try or mut use pub test extern unsafe void self defer
```

**Reserviert für nach v0:** `spawn parallel yield f32`. `f32` als Typ ist `S1`, Meldung `` `f32` is not available in v0 ``, Hinweis `use f64`.

**Reservierte Literale und Formen**, nie als Namen nutzbar: `true false` (Werte von `bool`), `none some ok err` (Formen von `?T` und `T!E`, R10).

**Kontextwörter** haben nur an einer Stelle eine Bedeutung und sind sonst gewöhnliche Namen: `as` in `use`-Zeilen; `c link pkg source impl load strip release with send` in `use c`- und `extern`-Zeilen; `panics` im Kopf eines `test`.

**Vordefinierte Namen** sind keine Schlüsselwörter, dürfen aber nicht neu deklariert werden (R4): die Typen `int i8 i16 i32 u8 u16 u32 u64 f64 bool str rune map` und die eingebauten Funktionen `print range repeat expect assert`. Die Namen `io` und `os` gelten, sobald ein `use std.io` bzw. `use std.os` sie einführt (R16).

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

`or` gehört zu einem vorangehenden `try` (`try x or E.V(…)`, R12). Klammern um einen Ausdruck sind immer erlaubt; `dif fmt` entfernt sie um Bedingungen.

```ebnf
expr     = binary ;
binary   = unary { binop unary } ;          (* geordnet nach der Tabelle *)
unary    = { "-" | "!" | "~" } ( try_expr | postfix ) ;
try_expr = "try" postfix [ try_or ] ;          (* R12 *)
suffix   = "(" [ args ] ")" | "[" expr "]" | "." IDENT ;
postfix  = primary { suffix } | primary { suffix } "(" [ args ] ")" block ;   (* zweite Form: Block-Argument, R26 *)
args     = arg { "," arg } [ "," ] ;
arg      = [ "mut" ] expr | func_lit ;               (* nach mut steht ein Ort (place, R3); R6; func_lit: R26 *)
primary  = INT | FLOAT | STRING | RUNE | "true" | "false" | IDENT | "self" | "(" expr ")"
         | struct_lit | variant_val | if_expr | "none" | err_val | some_val
         | list_lit | map_lit ;   (* R5, R7, R15, R12, R30 *)
```

Ausdrücke und Argumente werden **von links nach rechts** ausgewertet (R29).

---

## 3. Die Regeln

### R1 — Programm

**Regel.** Ein Programm besteht nur aus Deklarationen: `const`, `struct`, `enum`, Funktionen, `test`, `use`, `extern`. Es startet in `main`, die `void main()` oder `void!E main()` heißt (R12; `E` ist jeder Typ).

```ebnf
module   = { NL } [ decl { NL { NL } decl } { NL } ] ;
decl     = use_decl | const_decl | func_decl | struct_decl | enum_decl | method_decl | test_decl ;   (* extern folgt; use_decl: R16 *)
```

- Eine Anweisung auf oberster Ebene ist `R1`, Hinweis `move it into a function, e.g. void main() { … }`.
- Ein ausführbares Programm hat genau eine Funktion `main` ohne Parameter, `void main()` oder `void!E main()`; fehlt sie, ist das `R1` (bei `dif run`/`dif build`, nicht bei `dif check` einer Bibliothek).
- Die Reihenfolge der Deklarationen spielt keine Rolle, eine Funktion darf eine weiter unten deklarierte aufrufen; ausgenommen sind die `use`-Zeilen: sie stehen am Anfang der Datei vor allen anderen Deklarationen (R16: ein späteres `use` ist `R16`).

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
type       = [ "?" ] base_type [ "!" base_type ] ;         (* ?T und T!E: R12 *)
base_type  = prim_type | list_type | array_type | map_type | named_type ;
prim_type  = "int" | "i8" | "i16" | "i32" | "u8" | "u16" | "u32" | "u64" | "f64" | "bool" | "str" | "rune" | "void" ;
named_type = IDENT [ "." IDENT ] ;                      (* struct oder enum; mit Modul: io.IoErr (R16) *)
list_type  = "[" "]" elem_type ;                        (* R5 *)
array_type = "[" ( INT | IDENT ) "]" elem_type ;        (* R5; IDENT: eine const int *)
map_type   = "map" "[" elem_type "]" elem_type ;        (* R30 *)
elem_type  = [ "?" ] base_type | "(" type ")" ;         (* []?int: Liste von ?int; ?[]int: ?Liste; [](int!E): Liste von Ergebnissen *)
                                                        (* Klammern sind nur nötig, wenn der Elementtyp ein `!E` enthält; sonst sind sie überflüssig (§0) *)
func_decl  = type IDENT "(" [ params ] ")" block ;
params     = param { "," param } [ "," ] ;
param      = [ "mut" ] type IDENT | func_type IDENT ;   (* mut: R6; func_type: R26 *)
var_decl   = [ "const" ] type IDENT "=" expr ;
```

- `void` und `void!E` stehen nur als Rückgabetyp.
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
call_stmt   = try_expr | postfix ;         (* postfix muss mit einem Aufruf enden; try_expr: R12 *)
discard     = "_" "=" expr ;
```

- Jeder Block steht in `{ }`, auch bei einer einzigen Anweisung: `if x > 0 print(x)` ist `R3`, Hinweis `put the body in braces: if x > 0 { … }`.
- Ein Block aus einer einzigen Anweisung darf in einer Zeile stehen, `if x < 0 { return -1 }`; `dif fmt` setzt diese Form nach der Regel in §0 (Länge höchstens 100 Zeichen). Beides wird angenommen.
- Das `{` eines Blocks steht in derselben Zeile wie seine Kopfzeile; `}` und `else` stehen in einer Zeile (`} else {`).
- Zwei Anweisungen in einer Zeile sind `R3`, Hinweis `put each statement on its own line`. Folgt einer Anweisung ein Token, das keine Anweisung beginnen kann (`)`, `=`), ist das `S1`.
- Die Kopfzeile endet am ersten `{` auf Klammerebene 0. Darum ist `if hall.is_full() { … }` immer eine Bedingung mit Rumpf, nie ein Aufruf mit Block-Argument (R26). Ein Struct-Literal oder ein `if`-Ausdruck in einer Kopfzeile steht deshalb in Klammern: `if p == (Point{x: 0, y: 0}) {`. Ein Struct-Literal ohne Klammern dort ist `S1` am Typnamen, Meldung `a struct literal in a header needs parentheses`, Hinweis mit der geklammerten Kopfzeile. Ein Map-Literal (R30) steht dort ebenso in Klammern; ungeklammert ist es `S1` an seinem `{`, Meldung `a map literal in a header needs parentheses`. Ein Listen-Literal braucht keine Klammern: `for int x in [1, 2, 3] {`. Ein Funktionsliteral steht in den Klammern seines Aufrufs und beendet die Kopfzeile nicht; ein Block-Argument im Kopf braucht Klammern um den Aufruf (R26).
- Klammern um die Bedingung sind erlaubt (`if (x > 0) {` ist ein Klammerausdruck); `dif fmt` entfernt nur Klammern, die nichts bewirken, nicht die, ohne die ein `{` in der Kopfzeile den Kopf beenden würde (um ein Struct- oder Map-Literal, um einen Aufruf mit Block-Argument, R26).
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
print(n)
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

**Regel.** Zuweisung und Rückgabe kopieren den Wert; die Kopie ist unabhängig, intern aber billig. Ein **Griff** ist ein Wert, dessen Kopie dasselbe Objekt bezeichnet (Kanal, Task, `shared`, Datei, C-Objekt und die Handle-Typen der Standardbibliothek wie `db.Conn`, `html.Page`, `xml.Writer`, `ui.Builder`): kopiert wird der Griff, nicht das Ding. Seine Methoden dürfen hinter den Griff schreiben, ohne den Wert zu ändern: kein `mut self`, kein `mut` am Aufruf, kein `R6`. Griffe entstehen nur in der Standardbibliothek und aus C (opake Typen); eigene Typen sind in v0 nie Griffe. Parameter ohne `mut` sind lokale Kopien, wie in C.

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

Teil 2a legt fest (Griffe, Listen, Arrays, Maps und das Speichermodell: „R5, Teil 2c“ unten):

- `int`, `bool`, `str`, `struct` und `enum` sind Werte. `Point b = a` kopiert; danach ändert `b.x = 9` nur `b`. Ein Parameter ohne `mut` ist eine lokale Kopie und darf im Rumpf geändert werden, wenn die Änderung dort auch gelesen wird (R6).
- Ein `struct` hat benannte Felder, eines pro Zeile; `struct P { int x }` in einer Zeile ist `R3`, Hinweis `put each field on its own line`.
- Ein Feld darf einen Vorgabewert haben. Er wird bei jedem Literal neu ausgewertet, das das Feld weglässt. Erlaubt ist ein Ausdruck ohne Variablen und ohne `self`: Literale, `const`, Struct-Literale, Varianten, Operatoren und Aufrufe von Funktionen ohne Wirkung (R13); sonst `R5`, Meldung `a field default uses only constants and functions without effect`.
- Ein Literal `Point{x: 1, y: 2}` nennt jedes Feld mit Namen, in beliebiger Reihenfolge (kanonisch ist die Deklarationsreihenfolge, §0), ausgewertet von links nach rechts; danach die Vorgabewerte in Deklarationsreihenfolge. Ein Feld ohne Vorgabe, das fehlt, ist `R5`, Meldung `` `Point` needs a value for `y` ``, Hinweis mit dem ergänzten Literal; eingesetzt wird `0`, `false` oder `""`, bei anderen Typen `…`. Fehlen mehrere, nennt die Meldung sie in Deklarationsreihenfolge (`` `y`, `z` ``). Ein unbekanntes oder doppelt genanntes Feld ist `R4`.
- Ein Literal darf über mehrere Zeilen gehen: Zeilenumbrüche sind nach `{`, nach `,` und vor `}` erlaubt, ein abschließendes Komma ebenso.
- Felder liest und ändert man mit `p.x` und `p.x = 1`.
- Ein `enum` zählt seine Varianten auf, getrennt durch `,` oder Zeilenumbruch. Eine Variante kann benannte Werte tragen (`NoSuchSeat(int row, int seat)`). Man schreibt Varianten immer mit ihrem Typ: `Seat.Free`, `BookErr.NoSuchSeat(3, 5)` mit den Werten in Reihenfolge. An die Werte kommt man nur über `switch` (R10).
- Ein Typ, der sich über Felder oder Varianten-Werte selbst enthält (`struct Node { Node next }`, `enum Tree { Leaf, Node(Tree l, Tree r) }`, auch über mehrere Typen hinweg), ist `R5` an seinem Namen, Meldung `` `Tree` contains itself ``, Hinweis `a value has a fixed size; keep linked values in a list`. Über eine Liste oder Map (Teil 2c) enthält sich ein Typ nicht selbst, über ein Array schon.
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

### R5, Teil 2c — Speicher, Listen und Arrays

**Regel.** R5 gilt für alle Typen. Teil 2c legt fest, wie sich `str`, `[]T`, `[N]T` und `map[K]V` als Werte verhalten und wie Listen und Arrays aussehen (Maps: R30).

```ebnf
list_lit = "[" [ expr { "," expr } [ "," ] ] "]" ;
```

**Grammatik:** `list_type`, `array_type`, `elem_type` (R2), `list_lit` hier, Index `x[i]` (§2, `place` in R3).

- **Werte.** `str`, `[]T`, `[N]T` und `map[K]V` sind Werte wie `int`: eine Zuweisung, ein Argument ohne `mut`, ein Rückgabewert und ein Feld in einem Struct-Literal sind unabhängige Kopien, auch tief (Liste von Listen, Struct mit einer Liste im Feld). Ein Schreiben in die Kopie ändert nie das Original und umgekehrt: nach `ys = xs; xs[0] = 1` hat `ys` den alten Wert, nach `Hall b = a; b.seats.push(x)` hat `a.seats` den alten Inhalt. Ein Text ist unveränderlich; `s += "x"` und `s = s + "x"` bauen einen neuen Wert.
- **Intern geteilt.** `str`, Listen und Maps sind referenzgezählt mit Copy-on-Write: eine Kopie teilt den Speicher mit dem Original, bis eine der beiden geschrieben wird. Ein Schreiben an einem Wert, den nichts teilt, geschieht an Ort und Stelle, `push` ist amortisiert O(1); an einem geteilten Wert kopiert die erste Änderung vorher den Speicher, O(n). Zuweisung, Argument und Rückgabe kosten O(1), außer bei `[N]T` und Structs, die inline liegen und kopiert werden. Kein Programm kann sehen, ob ein Wert geteilt ist; der Speicher wird frei, sobald der letzte Wert verschwindet, und weil es im sicheren Code keine Referenzen gibt, gibt es keine Zyklen. Ist der Speicher erschöpft, bricht das Programm ab (R14).
- **Griffe** (Definition oben, Regel) sind Werte, die auf etwas außerhalb zeigen. Das Copy-on-Write gilt für sie nicht, und `==` vergleicht ihre Identität (R28). Eine Liste von Griffen kopiert die Griffe. Die Standardbibliothek von Teil 2c (§4) hat keinen Griff; sie kommen mit ihren Modulen.
- **Listen `[]T`** wachsen und schrumpfen. Das Literal `[1, 2, 3]` hat den Elementtyp seiner Stelle (R8): `[]u8 b = [1, 2]` ist eine Liste von `u8`; ohne Stelle bekommen alle Elemente denselben Typ, und der hängt nicht von der Reihenfolge ab: der Typ des ersten Elements, das kein Literal ist; bestehen alle aus Literalen, ist er `f64`, wenn eines davon ein Float-Literal ist, sonst `int` (`[x, 1]` und `[1, x]` mit `x` vom Typ `u8` sind beide `[]u8`; `print([1, 2])` ist `[]int`; `[1, 2.5]` und `[2.5, 1]` sind beide `[]f64`, denn ein Ganzzahl-Literal passt an die Stelle eines `f64` (R8, Teil 2c)). Danach wird jedes Element nach R8 an diesen Typ gebunden; eines, das nicht passt, ist `R8` am Element (`[x, 2.5]` mit `x` vom Typ `int`: Meldung `` expected int, found f64 ``, Hinweis `int(2.5)`; ein `str` darin: `` expected int, found str ``). Bei Schlüsseln und Werten einer Map gilt dieselbe Regel (R30). Die leere Liste `[]` hat keinen Typ von sich aus: ohne Stelle (`print([])`, `_ = []`) ist sie `R8`, Meldung `` `[]` needs a place of type []T ``, ohne Hinweis. Mit Stelle (`[]int xs = []`, Argument, Feldvorgabe, `return []`) gilt der Typ der Stelle. `==` vergleicht Länge und Elemente (R28).
- **Arrays `[N]T`** haben genau `N` Elemente und liegen inline; `N` ist ein Ganzzahl-Literal oder eine `const int` mit Wert ab 1, sonst `R5`, Meldung `` the length of an array is a constant of at least 1 ``. Das Literal `[a, b, c]` an der Stelle eines `[3]int` muss genau `N` Elemente haben, sonst `R8`, Meldung `` [3]int needs 3 elements, found 2 ``, Hinweis `use []int for a list that can have any length`, am `[`. Eine Liste wird nie von selbst zum Array (R31); `xs.slice(0, 3)` ist wieder eine Liste. Arrays verschachteln (`[3][3]int`).
- **Index.** `x[i]` liest ein Element einer Liste oder eines Arrays; `i` ist ein `int`, sonst `R8`, Meldung `` expected int, found u8 ``, Hinweis `int(i)`. Das Ergebnis ist eine Kopie. `x[i] = v`, `x[i] += v` und `x[i].f = v` schreiben in das Element (Ort, R6); das Element muss existieren, `xs[xs.len()] = v` hängt nicht an (`push`). Der Index ist immer geprüft: außerhalb von `0 … len-1` bricht das Programm ab (R14) an der `[`. Bei einem Array mit konstantem Index steht der Abbruch schon beim Kompilieren fest und ist `error[R14]`. Ein `str` hat keinen Index: `s[0]` ist `R8`, Meldung `` cannot index str ``, Hinweis `use s.slice(a, b) or s.runes()`.
- **Schleife über einen Schnappschuss.** `for T x in xs` läuft über den Wert, den `xs` beim Eintritt hatte (R11); Änderungen an `xs` im Rumpf sind sicher, wirken aber nicht auf diese Schleife. Das kostet nichts, bis der Rumpf `xs` schreibt: dann kopiert dieses erste Schreiben einmal.
- **Selbstbezug.** Über `[]T` und `map` darf ein Typ sich selbst enthalten (ein `struct Tree` mit dem Feld `[]Tree kids`, Beispiel unten): die Liste ist ein eigener Speicher. Über ein Array nicht (R5, Teil 2a).

```dif snippet
[]int xs = [1, 2, 3]
[]int ys = xs
xs[0] = 10
xs.push(4)
print(xs)
print(ys)
for int x in xs {
    if x == 10 {
        xs.push(99)
    }
}
print(xs.len())
```

Ausgabe: `[10, 2, 3, 4]`, `[1, 2, 3]`, `5`.

```dif snippet
u8 x = 7
print([1, 2])
print([1, 2.5])
print([2.5, 1])
print([x, 1])
```

Ausgabe: `[1, 2]`, `[1, 2.5]`, `[2.5, 1]`, `[7, 1]`.

```dif error R8
void main() {
    int x = 1
    print([x, 2.5])
}
```

```
main.dif:3:15: error[R8]: expected int, found f64
  hint: int(2.5)
```

```dif
struct Hall {
    []int taken = []
    [3]int rows = [0, 0, 0]
}

void book(mut Hall h, int i) {
    h.taken.push(i)
    h.rows[i] += 1
}

void main() {
    Hall a = Hall{}
    Hall b = a
    book(mut b, 2)
    print(a)
    print(b)
}
```

Ausgabe: `Hall{taken: [], rows: [0, 0, 0]}`, `Hall{taken: [2], rows: [0, 0, 1]}`.

```dif
struct Tree {
    int value
    []Tree kids = []
}

int Tree.total() {
    int sum = self.value
    for Tree k in self.kids {
        sum += k.total()
    }
    return sum
}

void main() {
    Tree leaf = Tree{value: 2}
    Tree root = Tree{value: 1, kids: [leaf, leaf]}
    root.kids[0].value = 10
    print(root.total())
    print(leaf.value)
}
```

Ausgabe: `13`, `2`.

```dif snippet
[3]int a = [1, 2, 3]
[3]int b = a
b[0] = 9
print(a)
print(b)
print(b.len())
[2][2]int grid = [[1, 2], [3, 4]]
grid[1][0] = 7
print(grid)
```

Ausgabe: `[1, 2, 3]`, `[9, 2, 3]`, `3`, `[[1, 2], [7, 4]]`.

```dif panic "index 3 out of bounds for length 3"
[]int xs = [1, 2, 3]
int i = 3
print(xs[i])
```

```
panic: index 3 out of bounds for length 3 (main.dif:4:13)
```

```dif error R14
void main() {
    [3]int a = [1, 2, 3]
    print(a[3])
}
```

```
main.dif:3:12: error[R14]: index 3 out of bounds for length 3
```

```dif error R5
struct Node {
    [2]Node kids
}
```

```
main.dif:1:8: error[R5]: `Node` contains itself
  hint: a value has a fixed size; keep linked values in a list
```

```dif error R8
void main() {
    [3]int a = [1, 2]
    print(a)
}
```

```
main.dif:2:16: error[R8]: [3]int needs 3 elements, found 2
  hint: use []int for a list that can have any length
```

```dif error R8
void main() {
    print([])
}
```

```
main.dif:2:11: error[R8]: `[]` needs a place of type []T
```

### R6 — `mut`

**Regel.** Soll eine Funktion ein Argument ändern, steht `mut` in der Signatur und am Aufruf; die Änderung landet beim Aufrufer. Eine Änderung, die danach nie gelesen wird, ist ein Fehler (sie träfe nur eine Kopie). `mut` nimmt Orte: Variablen, Felder und Elemente. Ein Ort geht höchstens einmal veränderbar in einen Aufruf: als `self` oder als `mut`-Argument; überlappen zwei Orte, ist das ein Fehler, bei Elementen mit Laufzeit-Index ein Abbruch. Liest derselbe Aufruf einen Ort zusätzlich, bekommt der lesende Teil einen Schnappschuss.

**Grammatik:** `param` (R2), `arg` (§2), `place` (R3).

Teil 2a legt fest:

- **Orte und Wurzeln.** Ein Ort ist eine Variable, ein Parameter oder `self` (seine *Wurzel*), gefolgt von Feldern: `p`, `p.x`, `scene.player.pos`. Was einen Ort ändert, ändert seine Wurzel.
- **Ändern und Lesen.** Ändern heißt: einem Ort zuweisen; mit `+=` usw. rechnen; ihn mit `mut` an einen Parameter übergeben, den die aufgerufene Funktion ändert; auf ihm eine Methode aufrufen, die `self` ändert. Die letzten drei lesen den Ort zugleich. Lesen heißt jede andere Benutzung der Wurzel oder eines Teils davon.
- **Signatur und Aufruf.** Ein Parameter `mut T p` ändert den Ort, den der Aufrufer mit `mut` übergibt: `shift(mut b, 5)`, `shift(mut scene.player, 5)`. Ein Ort mit `const`-Wurzel geht nie mit `mut` hinein (`R4`, `` cannot pass const `k` as mut ``). Meldungen, jeweils `R6` am Argument:
  - `mut` fehlt: `` `shift` takes `p` as mut ``, Hinweis: der Aufruf samt `mut`;
  - `mut` zu viel: `` `show` takes `p` without mut ``, Hinweis: der Aufruf ohne `mut`;
  - kein Ort nach `mut` (`mut f()`, `mut 1`): `only a variable, field or element can be passed as mut`.
- **Überflüssiges `mut`.** Ändert eine Funktion einen `mut`-Parameter nie (kein Ort mit dieser Wurzel wird geändert), ist das `R6` am `mut` des Parameters, Meldung `` `p` is never changed; remove mut ``, Hinweis: die Signatur ohne `mut`. Aufrufe, die dort `mut` übergeben, werden nicht zusätzlich gemeldet und zählen nicht als Änderung.
- **Verlorene Änderung.** Orte sind Pfade aus Wurzel und Feldern (`p`, `p.x`, `p.inner.y`); ein Index gehört zum indizierten Ort (`xs[i]` ist `xs`, `p.items[i].f` ist `p.items`), weil er erst zur Laufzeit feststeht. Eine Änderung eines Ortes ist verloren, wenn auf keinem Weg danach dieser Ort, ein Teil davon (`p.x.z`) oder ein Ganzes, das ihn enthält (`p`, etwa als Argument, in `print(p)`, `return p` oder `q = p`), gelesen wird, bevor der Ort ganz neu zugewiesen wird (`p.x = …` beendet `p.x` und alles darunter, `v = …` alles unter `v`; ein Index beendet nichts) oder die Funktion endet. Ein Lesen von `p.y` rettet also eine Änderung von `p.x` nicht; ein Methodenaufruf `p.show()` liest `p` ganz. Aufrufe mit `mut p.x` ändern wie bisher den Ort der Wurzel `p`. Am Ende einer Funktion gelten `mut`-Parameter und `self` als gelesen, weil die Änderung beim Aufrufer landet. In einer Schleife zählt der nächste Durchlauf als „danach“; ein `n += 1` im Rumpf liest dort den Wert des vorigen Durchlaufs und gilt darum als gelesen. Eine verlorene Änderung ist `R6` an ihrem Ort; die Meldung nennt den Pfad:
  - an einem Parameter ohne `mut`: `` the change to `p.x` is lost: `p` is a copy `` (bei einer ganzen Änderung `` the change to `p` is lost … ``), Hinweis `take it as mut: ` und die Signatur samt `mut`;
  - sonst: `` `x.y` is changed but never read afterwards ``, Hinweis `use the value, or remove the change`.
  
  Gemeldet wird je Ort (Wurzel mit Feldpfad) die erste verlorene Änderung. Der Anfangswert einer Deklaration ist keine Änderung: `int x = 0` gefolgt von `x = 1; x = 2; print(x)` meldet `x = 1`.
- **Keine Aliase.** In einem Aufruf geht ein Ort höchstens einmal veränderbar hinein: als `self` einer Methode, die `self` ändert, oder als `mut`-Argument. Zwei solche Orte dürfen sich nicht überlappen (`a` und `a.x`, oder zweimal `a.x`), sonst `R6` am zweiten, Meldung `` `p` goes into this call changeable twice ``, Hinweis `pass it changeable only once`. Liest der Aufruf denselben Ort zusätzlich als Argument ohne `mut`, bekommt dieses Argument den Wert von vor dem Aufruf. Ein Aufruf im Argument eines anderen ist ein eigener Aufruf: `f(mut a, g(mut a))` ist erlaubt, `g` ist fertig, bevor `f` beginnt.

Teil 2c ergänzt Elemente:

- Ein Ort ist auch ein Element einer Liste oder eines Arrays, `xs[i]`, `grid[1][0]`, `scene.items[i].pos`; seine Wurzel ist die Variable am Anfang. `m[k]` einer Map ist kein Ort (R30). Wer `xs[i]` ändert, ändert `xs`. `normalize(mut tasks[i])` und `tasks[i].done = true` sind gültig. Der Index wird vor dem Aufruf ausgewertet und geprüft (R14).
- **Überlappung.** `xs` und `xs[i]` überlappen immer, `xs[0]` und `xs[0]` ebenso, `xs[0]` und `xs[1]` nie; `R6` wie oben (`` `xs[0]` goes into this call changeable twice ``); sind beide Orte Elemente derselben Liste, lautet der Hinweis `to swap two elements use xs.swap(i, j)` (mit dem Namen des Orts statt `xs`). Zwei Elemente derselben Wurzel, von denen ein Index erst zur Laufzeit feststeht (`swap(mut xs[i], mut xs[j])`), prüft das Programm beim Aufruf; gleiche Indizes brechen mit `overlapping mut arguments: index <n>` ab (R14), `<n>` ist der gemeinsame Index. Wer zwei Elemente vertauschen will, nimmt `xs.swap(i, j)` (§4.4); Abbrüche tragen keinen Hinweis.

Teil 2d ergänzt Funktionsliterale und Blöcke (R26):

- **Ein Literal ändert nichts ringsum.** Es nimmt beim Auswerten eine Kopie jedes Namens der Umgebung, den es benutzt, und liest nur diese Kopien (R26). Darum ist ein Literal nie ein Weg in einen Aufruf: „Keine Aliase“ gilt unverändert und kennt keine Literale, ein Schnappschuss-Sonderfall entfällt. Ein Literal sieht immer den Wert von vor dem Aufruf: `xs.sort_by(…)` mit einem Literal, das `xs` liest, liest die Liste vor dem Sortieren.
- **Änderung an einem eingefangenen Namen.** Weist ein Literal oder Block einem Namen der Umgebung oder einem Teil davon zu, rechnet es mit `+=` usw. oder ruft es darauf eine Methode, die `self` ändert, ist das immer `R6` an der Änderung: Meldung `` the change to `x` is lost: `x` is a copy ``, Hinweis `a function literal sees a copy; return the value or use a for loop`. `x` ist die Wurzel (`p.x = 1` meldet `p`); gemeldet wird pro Wurzel die erste Änderung. Das gilt für jeden Namen der Umgebung: Variablen, Parameter mit und ohne `mut`, `self` und die Namen äußerer Literale. Eine `const`-Wurzel bleibt `R4`. Ein Griff (R5) ist keine Ausnahme der Regel, sondern fällt nicht darunter: seine Kopie ist derselbe Griff, und eine Methode, die hinter den Griff schreibt, ändert den Namen nicht.
- **Verlorene Änderung.** Was ein Literal liest, gilt am Aufruf als gelesen: eine Änderung vor dem Aufruf, die nur das Literal liest, ist nicht verloren. Parameter und Variablen des Literals selbst folgen den Regeln einer Funktion (siehe oben): die Änderung an einem Parameter ohne `mut` meldet `` the change to `p` is lost: `p` is a copy `` ohne Hinweis, weil sein Typ vom Aufruf kommt.

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
    print(p.y)
}
```

Das Lesen von `p.y` liest die Änderung an `p.x` nicht.

```
main.dif:7:5: error[R6]: the change to `p.x` is lost: `p` is a copy
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

- Methoden gibt es für die `struct`- und `enum`-Typen des Moduls und, eingebaut, für `str`, `rune`, Zahlen, Listen, Arrays und Maps (§4); eigene Methoden an anderen Typen sind `R7`. Der Name steht in der Form `Typ.name`, darum findet `grep "Point.scale"` die Definition. `self` steht nie in der Parameterliste.
- In der Methode ist `self` der Ort, auf dem sie aufgerufen wird, vom Typ `Typ`. Sie darf ihn lesen und ändern (`self.x *= k`, `self = Point{x: 0, y: 0}`). `self` außerhalb einer Methode ist `R7`.
- Der Compiler leitet ab, ob eine Methode `self` ändert: sie ändert (R6) einen Ort mit der Wurzel `self`, also etwa `self.x = 1`, `shift(mut self.pos, 1)` oder `self.inner.bump()` mit einer Methode `bump`, die `self` ändert. Die Ableitung geht über alle Aufrufe, auch rekursive.
- Eine Methode, die `self` ändert, braucht vor dem Punkt einen Ort (R6), dessen Wurzel keine `const`-Variable ist; auch Teilorte wie `scene.player.move(1)`. Vor einem anderen Ausdruck ist sie `R7` am Aufruf, Meldung `` `Counter.inc` changes self and needs a variable ``, Hinweis `store the value in a variable first`. Am Aufruf steht kein `mut`: der Methodenaufruf zeigt schon, dass der Ort vor dem Punkt gemeint ist. Für R6 zählt der Aufruf als Lesen und Ändern dieses Ortes.
- Eine Methode, die `self` nicht ändert, geht auf jedem Ausdruck: `fresh().total()`, `a.moved(1).moved(2)`.
- Eingebaute Methoden (§4) folgen denselben Regeln; die Tabellen dort nennen, welche `self` ändern. In der Meldung steht der Typ: `` `[]int.push` changes self and needs a variable ``.
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

### R8, Teil 2c — Zahlenbreiten, `f64` und `rune`

**Regel.** R8 gilt für alle Zahlentypen: sie mischen sich nie von selbst, auch `int` und `i32` nicht.

**Grammatik:** `prim_type` (R2), `INT`, `FLOAT`, `RUNE` (§1.4).

Teil 2c legt fest:

| Typ | Bits | Werte |
|---|---|---|
| `int` | 64 | `-9223372036854775808 … 9223372036854775807` |
| `i8` `i16` `i32` | 8 16 32 | `-128 … 127`, `-32768 … 32767`, `-2147483648 … 2147483647` |
| `u8` `u16` `u32` `u64` | 8 16 32 64 | `0 … 255`, `0 … 65535`, `0 … 4294967295`, `0 … 18446744073709551615` |
| `f64` | 64 | IEEE 754 binary64 (R29) |
| `rune` | 32 | ein Unicode-Codepoint `0 … 10FFFF` ohne Surrogate (D800 … DFFF) |

Ein *Ganzzahltyp* ist `int`, `i8 … u64`; *Zahlentyp* ist ein Ganzzahltyp oder `f64`. Die Tabelle von Teil 1 gilt für jeden Ganzzahltyp, ergänzt so:

| Operator | Operanden | Ergebnis |
|---|---|---|
| `+ - * / %` | zwei gleiche Ganzzahltypen, oder zwei `f64` (`%` ist dort `fmod`) | derselbe Typ |
| `+% -% *%`, `& \| ^ << >>` | zwei gleiche Ganzzahltypen (bei `<<`/`>>` hat auch die Weite diesen Typ) | derselbe Typ |
| `~` | Ganzzahltyp | derselbe Typ |
| Vorzeichen `-` | `int`, `i8`, `i16`, `i32`, `f64` | derselbe Typ |
| `== !=` | zwei gleiche Typen, die `equal` sind (R28) | `bool` |
| `< <= > >=` | zwei gleiche Typen aus Zahlentypen, `rune`, `str` | `bool` |

- **Typ eines Literals.** Ein Literal nimmt den Typ seiner Stelle (Teil 1). Bei einem binären Operator ist die Stelle des einen Operanden der Typ des anderen: `u8 y = x + 100` rechnet mit `100` als `u8`. Ohne Stelle ist ein Ganzzahl-Literal `int`, ein Float-Literal `f64`.
- **Art.** Ein Ganzzahl-Literal passt an die Stelle eines jeden Zahlentyps (`f64 x = 3`, `u8 b = 255`); ein Float-Literal nur an die eines `f64`, auch wenn sein Wert ganz ist (`int n = 2.0` ist `R8`, Meldung `` expected int, found f64 ``, Hinweis `int(2.0)` bzw. die Zahl ohne Punkt). `rune` hat nur `RUNE`-Literale; `rune r = 97` ist `R8`, Hinweis `rune(97)` bzw. `'a'`.
- **Passt der Wert?** Ein Ganzzahl-Ausdruck aus Literalen und `const`, dessen Ergebnis nicht in den Typ der Stelle passt, ist `R8` am Ausdruck, Meldung `` expected u8, found 256 ``, Hinweis `the value does not fit in u8`. Das gilt auch für negative Werte an unsigned-Stellen (`u8 x = -1`). Ausdrücke aus Ganzzahl-Literalen rechnet der Compiler exakt (Teil 1).
- **`f64`-Literale** werden einzeln auf den nächsten `f64` gerundet. Ein Literal, das sich nicht darstellen lässt, ist `R8` am Literal, Meldung `` expected f64, found 1e400 ``, Hinweis `the value does not fit in f64`: das gilt, wenn es dabei `inf` würde (`1e400`) und wenn ein Literal ungleich 0 dabei 0 würde (`1e-400`, `0.0` ist erlaubt); es gibt keinen stillen Überlauf und keinen stillen Unterlauf. **Ausdrücke** aus `f64`-Werten rechnen dagegen Operation für Operation in IEEE 754, auch beim Kompilieren, und liefern genau das Ergebnis der Laufzeit: `0.1 + 0.2` ist `0.30000000000000004`, `1.0 / 0.0` ist `inf` und `1e300 * 1e300` ist `inf`, kein Fehler (R29).
- **Keine Mischung.** Zwei verschiedene Zahlentypen an einem Operator sind `R8` am Operator, Meldung `` cannot apply `+` to int and i32 ``, Hinweis: der rechte Operand in den Typ des linken umgewandelt (`a + int(b)`). Ebenso eine Zahl an der Stelle eines anderen Zahlentyps (`f64 x = n`, Argument, `return`): `` expected f64, found int ``, Hinweis `f64(n)`. Es gibt keine Erweiterung `u8` → `int` und keine von `int` → `f64`.
- **Nicht erlaubt.** `-x` an `u8 … u64` ist `R8`, Meldung `` cannot apply `-` to u8 ``, Hinweis `use 0 -% x for wrapping, or a signed type`. `& | ^ << >> ~ +% -% *%` an `f64` sind `R8` mit derselben Meldung; Hinweis `use ints for bit operations`. `< <= > >=` an `bool`, Structs und Listen sind `R8`.
- **`rune`** hat keine Arithmetik und keine Verknüpfung mit Zahlen: `r + 1` ist `R8`, Meldung `` cannot apply `+` to rune and int ``, Hinweis `rune(int(r) + 1)`; `str + rune` ebenso, Hinweis `build text with interpolation: "{s}{r}"`. `==`, `!=` und `<` … vergleichen Codepoints. Umwandlungen R31, Methoden §4.2.
- **`switch`** geht auch über jeden Ganzzahltyp und über `rune` (R10); die `case`-Werte sind Literale oder `const` dieses Typs.

```dif error R8
void main() {
    int a = 1
    i32 b = 2
    print(a + b)
}
```

```
main.dif:4:13: error[R8]: cannot apply `+` to int and i32
  hint: a + int(b)
```

```dif error R8
void main() {
    u8 x = 256
    print(x)
}
```

```
main.dif:2:12: error[R8]: expected u8, found 256
  hint: the value does not fit in u8
```

```dif error R8
void main() {
    rune r = 'a'
    rune next = r + 1
    print(next)
}
```

```
main.dif:3:19: error[R8]: cannot apply `+` to rune and int
  hint: rune(int(r) + 1)
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
pattern     = [ "-" ] INT | STRING | RUNE | IDENT | IDENT "." IDENT { "." IDENT } [ "(" binding { "," binding } ")" ]   (* E.V, mit Modul io.E.V *)
            | result_pat ;
result_pat  = "ok" [ "(" binding ")" ] | "err" [ "(" binding ")" ] | "some" [ "(" binding ")" ] | "none" ;
binding     = type IDENT | "_" ;
```

Teil 2a legt fest:

- `switch` geht über `int` (und die anderen Ganzzahltypen, Teil 2c), `rune` (Teil 2c), `str`, `enum` und (Teil 2b) `T!E` und `?T`. Über einem anderen Typ (`f64`, Listen, Maps, Structs) ist es `R10`, Meldung `` cannot switch over bool `` (bzw. den Typ), für `bool` mit Hinweis `use if`.
- Nach `{` und nach jedem `case …:` und `default:` folgt ein Zeilenumbruch. Eine Anweisung in derselben Zeile (`case 0: return 1`, `switch s { … }` in einer Zeile) ist `R3`, Meldung `the statements of a case start on the next line`.
- Ein `case` nennt einen oder mehrere Werte. Seine Anweisungen stehen in den folgenden Zeilen bis zum nächsten `case`, `default` oder `}`; sie bilden einen eigenen Block. Es gibt kein Durchfallen; ein `case` ohne Anweisungen tut nichts.
- **Werte über Ganzzahltypen, `rune` und `str`:** Literale oder `const`-Namen dieses Typs (bei `rune` `'a'`). Ein Wert anderen Typs ist `R8`, ein unbekannter Name `R4`.
- **Werte über einem `enum E`:** nur Varianten, immer qualifiziert (`E.V`). Eine unqualifizierte Variante (`case Free:`) ist `R10`, Meldung `variants are written with their type`, Hinweis `case Seat.Free:`. Ein Literal, ein `const` oder eine Variante eines anderen `enum` ist `R8`.
- **Bindungen.** `case Shape.Rect:` passt auf jede `Rect`, gleich welche Werte sie trägt. Wer Werte braucht, bindet sie in Klammern, für jeden Wert der Variante einen Eintrag in Reihenfolge: Typ und Name, oder `_` zum Überspringen: `case Shape.Rect(int w, _):`. Falsche Anzahl oder falscher Typ ist `R8`, Meldung `` `Shape.Rect` has the values int w, int h ``. Klammern nur aus `_` sind `R10`, Hinweis `case Shape.Rect:`. Bindungen gibt es nur, wenn der `case` genau einen Wert nennt (`R10`). Gebundene Namen gelten im Block des `case` und folgen R4.
- Über einem `enum` ist jede Variante behandelt oder es steht ein `default`; sonst `R10` am `switch`, Meldung `switch does not handle ` mit den fehlenden Varianten in Deklarationsreihenfolge, getrennt durch `, ` (`switch does not handle Seat.Taken, Seat.Reserved`), Hinweis `add case Seat.Taken: …, or default:` mit der ersten fehlenden. Über Zahlen, `rune` und `str` ist `default` Pflicht (`R10`). Ein `default`, der nie erreicht wird, weil alle Varianten behandelt sind, ist `R10`.
- `default` steht als letzter Zweig. Ein Wert, der schon in einem früheren `case` steht, ist `R10` mit der Position des ersten.
- `break` steht nie direkt in einem `switch` (R11); `continue` und `return` dürfen dort stehen.
- Endet jeder Zweig eines vollständigen `switch` mit `return`, endet auch der `switch` dort (für „`return` auf jedem Weg“).

Teil 2b ergänzt `switch` über `T!E` und `?T` (Beispiele bei R12):

- `switch` geht auch über `T!E` (Muster `ok`, `err`) und `?T` (Muster `some`, `none`). Ein Muster der falschen Art (`case some:` über `T!E`, `case ok:` über `int`) ist `R8`, Meldung `` `some` needs a switch over ?T, found int!AgeErr ``.
- Mit Klammern bindet das Muster den Wert: `case ok(int n):`, `case err(AgeErr e):`, `case some(int x):`; Typ und Name wie bei Varianten, `_` überspringt. Ohne Klammern passt es auf jeden Wert (`case err:`). Ein falscher Typ ist `R8`, Meldung `` `ok` holds an int `` (bzw. `err` holds an AgeErr, `some` holds an int), Hinweis mit dem richtigen Muster (`case ok(int s):`), an dem Typ im Muster. `ok` bei `void!E` hat keinen Wert, `none` nie: Klammern dort sind `R10`, Meldung `` `ok` has no value in void!E `` (bzw. `` `none` has no value ``), Hinweis `case ok:` (bzw. `case none:`).
- Vollständig ist ein `switch` über `T!E`, wenn `ok` und `err` behandelt sind, über `?T`, wenn `some` und `none` behandelt sind, sonst steht ein `default`. Sonst `R10` am `switch`, Meldung `switch does not handle none` (bzw. `err`; fehlen beide: `ok, err` bzw. `some, none`), Hinweis `add case none: …, or default:`. Doppelte Muster und ein überflüssiges `default` sind `R10` wie bei `enum`.

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

Teil 2c legt fest:

- **Quellen.** `for T x in xs` läuft über die Elemente einer Liste oder eines Arrays in Reihenfolge; `for int i, T x in xs` liefert zusätzlich den Index `i` ab 0 (sein Typ ist `int`); `for K k, V v in m` läuft über die Einträge einer Map in der Reihenfolge der Einfügung (R30); `for str line in try io.lines()` über die Zeilen von stdin, eine Folge von `str!io.IoErr` (§4.6, „Folgen von Ergebnissen“ unten). Eine Quelle anderen Typs ist `R11` an der Quelle, Meldung `` cannot loop over int ``, Hinweis bei `int`: `for int i in range(n)`, bei `str`: `for rune r in s.runes()`, sonst keiner. Ein `str` ist keine Quelle.
- **Variablen.** Die Typen der Schleifenvariablen sind die der Elemente (bei `m` Schlüssel und Wert); ein anderer ist `R8`, Meldung `` expected str, found int `` an der Variablen. Die Form passt zur Quelle: eine Map mit einer Variablen ist `R11`, Meldung `` a loop over a map needs key and value ``, Hinweis `for str k, int v in counts` (mit den Typen und dem Namen der Quelle); `for int i, T x in m` ist `R8` wie oben. Die Variablen sind Kopien (R5); ihre Änderung wirkt nur bis zum nächsten Durchlauf.
- **Schnappschuss.** Die Quelle wird einmal vor der Schleife ausgewertet und als Wert festgehalten (R5, Teil 2c). Schreibt der Rumpf in `xs` oder `m`, sieht diese Schleife davon nichts; `for int x in xs { xs.push(x) }` endet. **Ausgenommen ist `io.lines()`:** stdin ist ein Strom, kein Wert; die Schleife liest bei jedem Durchlauf die nächste Zeile (§4.6). Der Index von `for int i, T x in …` zählt auch dort die Elemente ab 0; `for int i, str line in try io.lines()` ist erlaubt.
- `io.lines()` und `range` stehen nur als Quelle einer `for`-Schleife. Beispiele bei R30.
- **Folgen von Ergebnissen.** Hat eine Quelle Elemente vom Typ `T!E` oder `?T` (eine Liste oder ein Array `[](int!E)`, `io.lines()`, später weitere), entpackt `for T x in try xs` jedes Element wie `try` (R12): `x` ist das `T`; ist ein Element ein Fehler, kehrt die Funktion sofort mit ihm zurück. `try` steht in der Schleifenform wie überall in `expr`, die Grammatik `for_stmt` bleibt; `or` ist erlaubt (`for int n in try rs or E.V(…)`). Ohne `try` ist die Schleifenvariable ein `T!E` (`for int!E r in rs`); `for int n in rs` mit solchen Elementen ist `R8`, Meldung `` expected int, found int!ParseErr `` an der Variablen, Hinweis `for int n in try rs`. Beispiele und Fehler in „R11, Teil 2c“ unten, Regeln für den Fehlertyp in R12.


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

### R11, Teil 2c — Schleifen über Ergebnisse

```dif
enum ParseErr { Bad(str s) }

int!ParseErr parse(str s) {
    switch s.parse_int() {
    case some(int n):
        return n
    case none:
        return err(ParseErr.Bad(s))
    }
}

int!ParseErr total([]str parts) {
    [](int!ParseErr) rs = []
    for str p in parts {
        rs.push(parse(p))
    }
    int sum = 0
    for int n in try rs {
        sum += n
    }
    return sum
}

void main() {
    print(total(["1", "2", "3"]))
    print(total(["1", "x", "3"]))
}
```

Ausgabe: `ok(6)`, `err(ParseErr.Bad("x"))`.

```dif error R8
enum ParseErr { Bad }

void main() {
    [](int!ParseErr) rs = []
    for int n in rs {
        print(n)
    }
}
```

```
main.dif:5:13: error[R8]: expected int, found int!ParseErr
  hint: for int n in try rs
```

### R12 — Scheitern steht im Typ

**Regel.** Kann eine Funktion scheitern, steht es im Rückgabetyp: `T!E` heißt Wert oder `err(e)`, `?T` heißt Wert oder `none`. Erfolg gibt man einfach zurück. `try x` reicht einen Fehler gleichen Typs weiter; `try x or E.V(…)` übersetzt ihn vorher in einen eigenen, `try x or err(C c): E.V(c)` bindet dabei die Ursache, `try x or E.V` ist die Kurzform dafür.

```ebnf
err_val  = "err" "(" expr ")" ;
some_val = "some" "(" expr ")" ;
try_or   = "or" ( "err" "(" type IDENT ")" ":" postfix | postfix ) ;   (* Bindung der Ursache | Wert oder F.V *)
```

**Grammatik:** `type` (R2), `try_expr`, `none` und `??` (§2), `err_val`, `some_val` und `try_or` hier, die Muster `ok`, `err`, `some`, `none` in R10.

Teil 2b legt fest:

- **Typen.** `?T` ist ein `T` oder `none`; `T!E` ist ein `T` oder ein Fehler `E`; `void!E` ist „nichts oder ein Fehler“. `E` ist ein beliebiger Typ (`enum`, `struct`, `str`, …) ohne Sonderregel; ausgegeben wird er in der Textform von R28. In Typen bindet `!E` am schwächsten: `?int!E` ist `(?int)!E`. Weiter verschachtelt (`??int`, `int!A!B`) ist `S1`, Hinweis `a type has at most one ? and one !E`; ein `?` vor einem `?` ist also nie erlaubt, auch nicht in Klammern (`?(?int)`). Die Grenze gilt für jeden `type` einzeln: der Elementtyp einer Liste, eines Arrays oder einer Map ist ein eigener Typ (R2, Teil 2c), darum sind `?[]?int` (optionale Liste optionaler Zahlen, kein verschachteltes Optional) und `[](int!E)!F` gültig. Klammern um einen Elementtyp sind nur für ein `!E` nötig (`[](int!E)`, denn `[]int!E` ist „Liste oder Fehler“); `[](?int)` ist dasselbe wie `[]?int`, und `dif fmt` entfernt die Klammern (§0). `?void` ist `R12`, Meldung `?void has no value that could be missing`, Hinweis `use void!E`. `void!E` steht wie `void` nur als Rückgabetyp; sonst darf `?T` und `T!E` überall stehen, wo ein Typ steht (Variablen, Parameter, Felder).
- **Erfolg.** Ein `T` passt dort, wo `?T` oder `T!E` erwartet wird (R31): bei `return`, Initialisierung, Zuweisung und Argument; als Operand von `==` nicht (R28). Eine `void!E`-Funktion endet mit `return` oder am Ende ihres Rumpfs und ist damit erfolgreich; jede andere braucht auf jedem Weg ein `return` mit Wert (R8).
- **`none`** ist der leere Wert von `?T`, **`some(x)`** ein `?T` mit dem Wert `x`, **`err(e)`** der Fehlerwert von `T!E`; `e` hat den Typ `E` (sonst `R8`). `some(x)` hat den Typ `?T`, wobei `T` der Typ von `x` ist; `x` selbst ist kein `?U` (`some(x)` mit `x` vom Typ `?int` ist `R12`, Meldung `` `some` needs a plain value, found ?int ``, Hinweis `use x directly`). `none` und `err(e)` nehmen ihren Typ von ihrer **Stelle** (R8; bei `==` und `!=` ist der andere Operand die Stelle, `r == err(E.X)` und `x == none` sind also gültig).
- **Falsche Stelle.** Ein Wert, der an seiner Stelle nicht passt, ist `R8` wie jeder andere: `int n = none` mit der Meldung `` expected int, found none `` (bei `err(…)` ebenso `found err(…)`, bei `some(3)` `` found ?int ``), Hinweis bei einer Deklaration `declare it as ?int n` (bei `err(e)` `int!AgeErr n`, mit dem Typ von `e`). Nur wenn gar keine Stelle da ist (`none == none`, `print(none)`), ist es `R12`, Meldung `` `none` needs a place of type ?T `` bzw. `` `err(…)` needs a place of type T!E ``, ohne Hinweis.
- **Kanonisch** ist an einer `?T`-Stelle `x` statt `some(x)` (R31). `dif fmt` macht `some(x)` zu `x`, wenn die Stelle ein Ziel ist: Deklaration, Zuweisung, Argument, `return`. Nie als Operand von `==` und `!=`: dort bleibt `x == some(3)` stehen (`x == 3` wäre `R8`). Der Compiler meldet `some(x)` an einem Ziel nicht.
- **`ok(x)`** gibt es nur als Muster in `switch`; als Ausdruck ist es `R12`, Meldung `` `ok` is only a pattern in switch ``, Hinweis `return the value directly: return 1`.
- `void` steht auch in `int!void` nur als Rückgabetyp (R2); `?T!E` mit `?T` als Wert ist erlaubt.
- **`try x`** hat den Typ `T`, wenn `x` ein `T!E` oder `?T` ist, und gilt nur in Funktionen und `test`-Blöcken (R18). Ist `x` ein Fehler oder `none`, kehrt die Funktion sofort mit genau diesem Fehler bzw. `none` zurück; sonst rechnet der Ausdruck mit dem Wert weiter. `try` bindet wie ein Vorzeichen (§2): `try f(a) + try f(b)` ist `(try f(a)) + (try f(b))`. Als Anweisung steht `try f()` für den Aufruf `f()` (R3); der Wert `T` zählt für R13 wie das Ergebnis eines Aufrufs.

  | `x` | Funktion gibt | `try x` | `try x or e` |
  |---|---|---|---|
  | `T!E` | `U!E` | gibt `E` weiter | `e` hat Typ `E` |
  | `T!E` | `U!F` | `R12` | `e` hat Typ `F` |
  | `?T` | `?U` | gibt `none` weiter | `R12` |
  | `?T` | `U!F` | `R12` | `e` hat Typ `F` |
  | `T!E`, `?T` | `?U` bei `T!E`, sonst ohne Fehler (`int`, `void`, …) | `R12` | `R12` |

- Die Meldungen von `try`, jeweils an `try` bzw. `or`:
  - Funktion kann nicht scheitern: `` `try` needs a function that returns T!E or ?T ``, Hinweis `change the return type to int!E, or use ?? or switch` (mit dem `T` der Funktion, `?int` bei `?T`; ohne Hinweis außerhalb einer Funktion).
  - falsche Form oder falscher Fehlertyp: `` `try` gives AgeErr, but the function returns int!FormErr `` (statt `AgeErr` steht `none` bei `?T`), Hinweis `translate it with try … or FormErr.V(…)`; bei einer Funktion `?U` stattdessen `handle the error with ?? or switch`.
  - `x` kann nicht scheitern: `` `try` needs a result of type T!E or ?T, found int ``, Hinweis `remove try`.
  - `or` in einer Funktion, die kein `U!F` liefert: `` `or` needs a function that returns U!F ``, Hinweis `remove the or part, or return int!F`.
  - `or` ohne `try` ist `S1`, Meldung `` unexpected `or` ``, Hinweis `or belongs to try: try f() or E.V(…)`.
- **`try x or e`:** `e` wird nur ausgewertet, wenn `x` scheitert; es ersetzt den ursprünglichen Fehler (er ist danach nicht mehr erreichbar). Für `?T` ersetzt es `none`. Hat `e` nicht den Fehlertyp der Funktion, ist es `R8`. `or` gehört zum unmittelbar davorstehenden `try` und umfasst genau einen `postfix`-Ausdruck: `try f() or E.V(1) + 2` ist `(try f() or E.V(1)) + 2`.
- **`try x or err(E e): w`** bindet den Fehler von `x`. `E` ist der Fehlertyp von `x`, ausgeschrieben (R2); ein anderer Typ ist `R8` am Typ, Meldung `` `err` holds a ParseErr `` (mit dem richtigen Typ), Hinweis `or err(ParseErr s): …` (mit dem Namen aus dem Quelltext). `e` ist nur in `w` sichtbar (R4) und ein gewöhnlicher Name, der dort nicht schon vergeben sein darf. `w` hat den Fehlertyp der Funktion (sonst `R8` wie bei `or e`) und wird nur im Fehlerfall ausgewertet; was `w` aus `e` braucht, wird kopiert, `e` selbst wird danach wie der ursprüngliche Fehler freigegeben. Bei einem `?T` gibt es keinen Fehlerwert: `err(…):` ist dort `R12` am Typ, Meldung `` `try` on ?int has no error to bind ``, Hinweis `remove err(…):`; ohne Bindung gilt dort die Tabelle. Ob hinter `or` die Bindung oder ein Wert `err(x)` steht, entscheidet das `:` hinter der schließenden Klammer.
- **Kurzform `or F.V`.** Der Ausdruck hinter `or` hat eine von zwei Formen, nach seinem Typ: ein Wert vom Fehlertyp `F` der Funktion (wie bisher), oder ein Varianten-Konstruktor `F.V` ohne Klammern (R26), dessen **genau eine** Nutzlast den Fehlertyp von `x` hat: `try io.read_file(path) or AppErr.Io` ist `try io.read_file(path) or err(io.IoErr e): AppErr.Io(e)`, die Ursache wandert unverändert in die Variante. Hat `V` eine andere Nutzlast, ist es `R8` am Konstruktor, Meldung `` `AppErr.Msg` holds str, but the cause is ParseErr ``; hat `V` mehrere, `` `AppErr.Two` has 2 values, but the cause is one ``; Hinweis jeweils `write it out: or err(ParseErr e): AppErr.Msg(…)`. Eine Variante ohne Nutzlast ist ein gewöhnlicher Wert. Für `?T` gibt es die Kurzform nicht (`F.V` mit Nutzlast ist dort `R8`, Meldung `` `F.V` needs its values: F.V(…) ``).
- **Kanonische Form (§0).** `dif fmt` schreibt `or err(E e): F.V(e)` (genau diese Form: `e` als einziges Argument) als `or F.V`; der Compiler nimmt beide an und meldet für keine einen Fehler. Alles, was mehr tut (ein anderes Argument, ein Ausdruck um `e`, mehrere Nutzlasten), bleibt die Langform.
- **`a ?? b`** ist `a`, wenn `a` einen Wert hat, sonst `b`; `a` ist ein `?T` oder `T!E`, `b` und das Ergebnis sind `T`. `b` wird nur ausgewertet, wenn es gebraucht wird. `??` bindet am schwächsten und ist rechtsassoziativ: `a ?? b ?? c` ist `a ?? (b ?? c)`. Ist `a` weder `?T` noch `T!E`, ist es `R12` am `??`, Meldung `` `??` needs a ?T or T!E on the left, found int ``, Hinweis `remove the ?? and its default`. Ein `b` anderen Typs ist `R8`.
- **`try` an einer Schleife.** `for T x in try xs` (R11) wendet `try` auf jedes Element einer Folge von Ergebnissen an (Liste oder Array `[](T!E)` oder `[](?T)`, `io.lines()`): `x` hat den Typ `T`, ein Element, das ein Fehler oder `none` ist, beendet die Schleife und kehrt aus der Funktion zurück, genau wie `try` an einem Einzelwert. Für Fehlertyp der Funktion, `or` und die Tabelle oben gilt jeweils das Element als `x`; alle Meldungen von `try` stehen unverändert an dem `try` vor der Quelle. Eine Quelle, die keine Folge von `T!E` oder `?T` ist, ist `R12`, Meldung `` `try` needs a list of T!E or ?T, found []int ``, Hinweis `remove try`. Eine Schleife mit `try` darf nur in einer Funktion oder einem `test`-Block stehen (wie `try`).
- **`void!E main()`** ist neben `void main()` erlaubt; `E` ist jeder Typ; jede andere Form von `main` ist `R1`, Meldung `` `main` must return void or void!E ``. Kommt ein Fehler aus `main` heraus, schreibt das Programm eine Zeile `error: ` und die Textform des Fehlers (R28; ein `str` unverändert, wie bei `print`) auf stderr und endet mit Exit-Code 1; was vorher auf stdout stand, bleibt erhalten. Einen Fehler gibt `main` an die Umgebung, ein Panic (R14) endet mit Exit-Code 2.
- `switch` über `T!E` und `?T` steht in R10, Verwendungspflicht in R13, Gleichheit und Textform in R28.

```dif
enum AgeErr {
    Negative
    TooOld(int n)
}

enum FormErr {
    BadAge(int n)
}

int!AgeErr check_age(int n) {
    if n < 0 {
        return err(AgeErr.Negative)
    }
    if n > 150 {
        return err(AgeErr.TooOld(n))
    }
    return n
}

?int half(int n) {
    if n % 2 != 0 {
        return none
    }
    return n / 2
}

int!AgeErr double_age(int n) {
    return try check_age(n) * 2
}

int!FormErr form_age(int n) {
    int a = try check_age(n) or FormErr.BadAge(n)
    return a
}

void main() {
    print(check_age(30) ?? 0)
    print(check_age(-1) ?? 0)
    print(half(7) ?? half(8) ?? -1)
    print(half(8))
    print(half(7))
    print(check_age(-1))
    switch double_age(200) {
    case ok(int n):
        print("ok {n}")
    case err(AgeErr e):
        print("error {e}")
    }
    switch form_age(200) {
    case ok(int a):
        print("age {a}")
    case err(FormErr e):
        print("form error {e}")
    }
}
```

Ausgabe: `30`, `0`, `4`, `some(4)`, `none`, `err(AgeErr.Negative)`, `error AgeErr.TooOld(200)`, `form error FormErr.BadAge(200)`.

Die Ursache binden und die Kurzform (`or err(E e): F.V(e)` und `or F.V` sind dasselbe, `dif fmt` schreibt die Kurzform):

```dif
use std.io

enum ParseErr {
    Empty
    NotANumber(str text)
}

enum AppErr {
    BadAmount(ParseErr cause)
    Io(io.IoErr cause)
}

int!ParseErr parse_amount(str raw) {
    if raw.is_empty() {
        return err(ParseErr.Empty)
    }
    int n = raw.parse_int() ?? -1
    if n < 0 {
        return err(ParseErr.NotANumber(raw))
    }
    return n
}

int!AppErr order(str raw) {
    int n = try parse_amount(raw) or err(ParseErr e): AppErr.BadAmount(e)
    return n * 10
}

str!AppErr load(str path) {
    str text = try io.read_file(path) or AppErr.Io
    return text
}

void main() {
    print(order("12"))
    print(order(""))
    print(order("abc"))
    print(load("no/such/file.txt"))
}
```

Ausgabe: `ok(120)`, `err(AppErr.BadAmount(ParseErr.Empty))`, `err(AppErr.BadAmount(ParseErr.NotANumber("abc")))`, `err(AppErr.Io(IoErr.NotFound("no/such/file.txt")))`.

Ein Fehler aus `main`:

```dif fails "HallErr.NoSeat(7)"
enum HallErr {
    NoSeat(int n)
}

void!HallErr book(int n) {
    if n > 5 {
        return err(HallErr.NoSeat(n))
    }
    print("booked {n}")
}

void!HallErr main() {
    try book(3)
    try book(7)
    print("not reached")
}
```

Ausgabe: `booked 3` auf stdout; auf stderr `error: HallErr.NoSeat(7)`; Exit-Code 1.

```dif error R12
enum E { Bad }

int!E parse(int n) {
    return n
}

int twice(int n) {
    return try parse(n) * 2
}
```

```
main.dif:8:12: error[R12]: `try` needs a function that returns T!E or ?T
  hint: change the return type to int!E, or use ?? or switch
```

```dif error R12
enum AgeErr { Negative }
enum FormErr { BadAge }

int!AgeErr check(int n) {
    return n
}

int!FormErr form(int n) {
    return try check(n)
}
```

```
main.dif:9:12: error[R12]: `try` gives AgeErr, but the function returns int!FormErr
  hint: translate it with try … or FormErr.V(…)
```

```dif error R8
enum AppErr {
    Msg(str text)
}

enum ParseErr {
    Empty
}

int!ParseErr parse(str raw) {
    return 1
}

int!AppErr order(str raw) {
    return try parse(raw) or AppErr.Msg
}
```

```
main.dif:14:30: error[R8]: `AppErr.Msg` holds str, but the cause is ParseErr
  hint: write it out: or err(ParseErr e): AppErr.Msg(…)
```

```dif error R8
enum AppErr {
    Msg(str text)
}

enum ParseErr {
    Empty
}

int!ParseErr parse(str raw) {
    return 1
}

int!AppErr order(str raw) {
    return try parse(raw) or err(str s): AppErr.Msg(s)
}
```

```
main.dif:14:34: error[R8]: `err` holds a ParseErr
  hint: or err(ParseErr s): …
```

```dif error R12
enum AppErr {
    Msg(str text)
}

?int find(str raw) {
    return none
}

int!AppErr order(str raw) {
    return try find(raw) or err(str s): AppErr.Msg(s)
}
```

```
main.dif:10:33: error[R12]: `try` on ?int has no error to bind
  hint: remove err(…):
```


```dif error R12
void main() {
    int n = 3
    print(n ?? 0)
}
```

```
main.dif:3:13: error[R12]: `??` needs a ?T or T!E on the left, found int
  hint: remove the ?? and its default
```

```dif error R8
void main() {
    int n = none
    print(n)
}
```

```
main.dif:2:13: error[R8]: expected int, found none
  hint: declare it as ?int n
```

```dif error R12
enum E { Bad }

int!E get() {
    return ok(1)
}
```

```
main.dif:4:12: error[R12]: `ok` is only a pattern in switch
  hint: return the value directly: return 1
```

Beispiele zu R8, R10 und R13 in Teil 2b:

```dif error R8
void main() {
    ?int x = 4
    print(x == 3)
}
```

```
main.dif:3:13: error[R8]: cannot apply `==` to ?int and int
  hint: x == some(3)
```

Weitere Beispiele:

```dif error R10
?int half(int n) {
    if n % 2 != 0 {
        return none
    }
    return n / 2
}

void main() {
    switch half(8) {
    case some(int h):
        print(h)
    }
}
```

```
main.dif:9:5: error[R10]: switch does not handle none
  hint: add case none: …, or default:
```

```dif error R8
enum E { Bad }

int!E get() {
    return 1
}

void main() {
    switch get() {
    case ok(str s):
        print(s)
    case err(E e):
        print(e)
    }
}
```

```
main.dif:9:13: error[R8]: `ok` holds an int
  hint: case ok(int s):
```

```dif error R13
enum HallErr { Full }

void!HallErr book(int n) {
    if n > 5 {
        return err(HallErr.Full)
    }
    print("booked {n}")
}

void main() {
    book(3)
}
```

```
main.dif:11:5: error[R13]: result of `book(3)` is never used
  hint: use it with try, ?? or switch, or discard it explicitly with `_ = book(3)`
```

### R13 — Jeder Aufruf hat eine Wirkung

**Regel.** Ein Ergebnis vom Typ `T!E` oder `?T` wird immer verwendet: `try`, `??`, `switch` oder bewusst `_ =`. Jeder andere Aufruf braucht eine Wirkung: sein Ergebnis wird verwendet, oder er ändert etwas (`mut`, `self`), macht Ein-/Ausgabe, ruft C oder startet einen Task. Ein Aufruf ohne Wirkung ist ein Fehler.

**Grammatik:** `call_stmt` und `discard` aus R3.

Teil 1 und Teil 2a legen fest:

- Ein Aufruf als Anweisung, dessen Ergebnis nicht `void` ist, ist `R13`, Hinweis `use it, or discard it explicitly with _ = …` — außer die Funktion hat eine Wirkung (ab Teil 2a).
- **Wirkung (Teil 2a).** Eine Funktion oder Methode hat eine Wirkung, wenn sie einen `mut`-Parameter oder `self` ändert, `print` aufruft oder eine Funktion mit Wirkung aufruft; der Compiler leitet das über alle Aufrufe hinweg ab (auch rekursiv). Ein Aufruf mit Wirkung darf als Anweisung stehen, auch wenn er einen Wert liefert. Ergebnisse vom Typ `T!E` und `?T` werden trotzdem immer verwendet (Teil 2b).
- `_ = f()` verwirft ein Ergebnis bewusst.
- Ein Aufruf einer `void`-Funktion als Anweisung ist erlaubt.

Teil 2b legt fest:

- Ein Aufruf als Anweisung, dessen Ergebnis ein `T!E` oder `?T` ist (auch `void!E`), ist immer `R13`, gleich ob die Funktion eine Wirkung hat. Meldung `` result of `book(3)` is never used ``, Hinweis `` use it with try, ?? or switch, or discard it explicitly with `_ = book(3)` ``. Verwendet ist es durch `try`, `??`, `switch`, `_ =`, aber auch als Argument, Rückgabewert oder Initialwert einer Variablen.
- `try f()` als Anweisung ist der Aufruf `f()` mit entpacktem Ergebnis: ist es nicht `void`, gilt dafür die Regel von oben (R13 bei einer Funktion ohne Wirkung).
- `_ = f()` verwirft auch ein `T!E` und `?T` bewusst; der Fehler geht dabei verloren.

Teil 2c legt fest:

- Eine eingebaute Methode hat eine Wirkung, wenn die Tabellen in §4 sie als „ändert“ führen (`push`, `sort`, `remove` …); ebenso `print`, `io.eprint`, `io.write_file` und `os.exit`. Alle anderen (`len`, `trim`, `slice`, …) haben keine: ein Aufruf als Anweisung ohne Verwendung des Ergebnisses ist `R13`. Ergebnisse vom Typ `?T` (`xs.pop()`, `s.find("x")`) und `T!E` (`io.read_file`) werden immer verwendet (Teil 2b), auch wenn die Methode `self` ändert: `_ = xs.pop()`.

Teil 2d legt fest (Funktionsliterale, R26):

- **Wirkung eines Aufrufs mit Literal.** Ein Aufruf hat eine Wirkung, wenn die Funktion oder Methode eine hat (wie oben) oder eines seiner Literale (auch ein Block). Ein Literal hat eine Wirkung, wenn sein Rumpf `print` oder `io.eprint` aufruft, C ruft oder einen Aufruf mit Wirkung enthält, darunter eine Methode, die hinter einen Griff schreibt; ändert er nur seine eigenen Parameter und Variablen, hat er keine (an einem Namen der Umgebung darf er nichts ändern, R6). Eine Funktion, die nur einen Funktionsparameter aufruft, hat dadurch keine Wirkung: sie kommt vom Literal am Aufruf. Darum ist `xs.any(bool(int x) { print(x) … })` als Anweisung gültig, `xs.filter(bool(int x) { return x > 1 })` dagegen `R13`, Meldung `` result of `xs.filter(bool(int x) { return x > 1 })` is never used ``, Hinweis `` use it, or discard it explicitly with `_ = xs.filter(bool(int x) { return x > 1 })` ``. Steht der Aufruf über mehrere Zeilen, zeigt die Meldung seinen Text mit einem Leerzeichen für jeden Zeilenumbruch samt Einrückung.
- Der Aufruf eines Funktionsparameters im Rumpf (`f(x)`) hat für sich keine Wirkung; ein Ergebnis, das nicht `void` ist, wird verwendet oder mit `_ =` verworfen.

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

### R13, Teil 2d — Wirkung eines Aufrufs mit Literal

Die Regeln stehen bei R13 oben (Teil 2d). Ein Literal mit Wirkung (`print`, ein Aufruf mit Wirkung, eine Methode auf einem Griff) macht den Aufruf als Anweisung gültig; ohne sie bleibt der Aufruf `R13`. Ein Literal, das etwas ringsum ändern wollte, ist stattdessen `R6` (Teil 2d).

```dif snippet
[]int xs = [1, 2, 3]
xs.any(bool(int x) {
    print(x)
    return x == 2
})
```

Ausgabe: `1`, `2`.

```dif error R13
void main() {
    []int xs = [1, 2, 3]
    xs.filter(bool(int x) { return x > 1 })
}
```

```
main.dif:3:5: error[R13]: result of `xs.filter(bool(int x) { return x > 1 })` is never used
  hint: use it, or discard it explicitly with `_ = xs.filter(bool(int x) { return x > 1 })`
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

Teil 2c ergänzt (alle Meldungen auf englisch, wie in Teil 1; `<…>` sind die tatsächlichen Werte):

| Fall | Meldung |
|---|---|
| Überlauf bei `+ - *`, Vorzeichen `-` und `/` in jedem Ganzzahltyp (`u8`: `200 + 100`, `i8`: `-128 / -1`) | `integer overflow in +` (bzw. `-`, `*`, `/`) |
| Shift mit Weite außerhalb `0 … Bits-1` des Typs | `shift by <n> out of range 0..<Bits-1>`, z. B. `shift by 8 out of range 0..7` |
| `abs()` von `-128`, `-32768`, … und dem kleinsten `int` | `integer overflow in abs` |
| Index außerhalb von Liste oder Array, auch `remove` | `index <i> out of bounds for length <n>` |
| `insert` mit Index außerhalb `0 … n` | `insert index <i> out of bounds for length <n>` |
| `slice(a, b)` mit `a < 0`, `a > b` oder `b > n` (Liste, Array, `str`) | `slice <a>..<b> out of bounds for length <n>` |
| `str.slice` mitten in einem Zeichen | `slice <a>..<b> is not on character boundaries` |
| `T(x)` ungenau (R31) | `300 does not fit in u8`, `2.5 is not an exact int`, `NaN is not an exact int`, `9007199254740993 is not an exact f64`, `55296 is not a valid rune` |
| `repeat` und `str.repeat` mit negativer Anzahl | `repeat count <n> is negative` |
| `split` mit leerem Trenner | `split needs a non-empty separator` |
| `replace` mit leerem Suchtext | `replace needs a non-empty text to find` |
| `NaN` als Schlüssel einer Map | `NaN cannot be a map key` |
| gleiche Elemente zweimal mit `mut` in einem Aufruf (R6) | `overlapping mut arguments: index <i>` |
| `os.exit` mit einem Code außerhalb `0 … 255` | `exit code <n> out of range 0..255` |
| `assert(false)` | `assertion failed: <Bedingung>` (Quelltext, Zeilenumbrüche als Leerzeichen) |
| ein Argument von `os.args()` ist kein gültiges UTF-8 | `invalid UTF-8 in argument <i>` (`<i>` ab 0, wie der Index in `os.args()`) |
| ein Wert von `os.env(name)` ist kein gültiges UTF-8 | `invalid UTF-8 in environment variable <name>` |
| Speicher erschöpft | `out of memory` (ohne Zeile und Spalte: der Speichermangel hängt an keiner Stelle im Quelltext) |

Die Position ist die des Operators, bei einem Index die des `[`, bei einem Aufruf (`T(x)`, Methode, `os.args()`, `os.env(…)`) die des Namens vor der `(`. Nur `out of memory` hat keine Position: die Meldung nennt statt Zeile und Spalte nur die Datei, `panic: out of memory (main.dif)`. Was beim Kompilieren feststeht (nur Literale und `const`: Index in ein Array, `T(x)`, Shifts), ist `error[R14]` mit derselben Meldung.

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
- **Zweige, die nie ankommen.** Ein Zweig darf statt mit einem Wert mit `return`, `break` oder `continue` enden; er gibt dann keinen Wert und gilt für den Typ nicht, mindestens ein Zweig muss einen Wert geben, sonst `R15` am `if`, Meldung `no branch of this if gives a value`. Beim Rücksprung wird alles Gehaltene freigegeben wie bei `try` (R12, R29); `break` und `continue` brauchen eine umgebende Schleife (R3).
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

```dif
enum E { Neg }

int!E root(int n) {
    int x = if n >= 0 { n } else { return err(E.Neg) }
    return x
}

void main() {
    for int v in [3, -1, 5] {
        int y = if v >= 0 { v } else { continue }
        print(y)
    }
    print(root(-1))
}
```

Ausgabe: `3`, `5`, `err(E.Neg)`. Der Zweig mit return kommt nie bei x an und gibt vorher frei, was gehalten wird.

```dif error R15
void main() {
    int x = if true { return } else { return }
    print(x)
}
```

```
main.dif:2:13: error[R15]: no branch of this if gives a value
```

```dif error R15
void main() {
    int n = 3
    int x = if n > 0 { n } else {
        n = 1
    }
    print(x)
}
```

```
main.dif:3:33: error[R15]: this branch of if gives no value
```

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

*Ausarbeitung in Teil 3, bis auf die Zeilen `use std.…`.*

```ebnf
use_decl = "use" "std" "." IDENT ;      (* Teil 2c; use name, use c und as folgen *)
```

Teil 2c legt fest (nur die Standardbibliothek; eigene Module, `as` und `pub` folgen):

- `use std.io` und `use std.os` machen den Namen `io` bzw. `os` bekannt; Funktionen und Typen daraus schreibt man mit dem Modul davor: `io.read_file(path)`, `io.IoErr`, `io.IoErr.NotFound(path)`, `os.args()`. Die `use`-Zeilen stehen am Anfang der Datei vor allen anderen Deklarationen (R1; die Reihenfolge der übrigen ist beliebig), eine Zeile nur einmal. Ein `use` hinter einer anderen Deklaration und eine doppelte Zeile sind `R16` am `use`, Meldung `` `use` comes before all other declarations `` bzw. `` `std.io` is already used ``, Hinweis `move it to the top of the file` bzw. `remove this line`.
- Ein Modul, das nicht existiert (`use std.json`), ist `R16` am Namen, Meldung `` unknown standard module `json` ``, Hinweis `available: io, os`.
- Die Benutzung von `io.…` oder `os.…` ohne `use` ist `R16` am Namen, Meldung `` `io` is not imported ``, Hinweis `add use std.io at the top of the file`. Ein Name `io` oder `os` als Variable, Parameter oder Funktion ist `R4`, sobald das Modul benutzt wird.
- Die Textform eines Typs aus einem Modul nennt ihn ohne Modul (`IoErr.NotFound("a.txt")`, R28).

### R17 — C, Pointer, `unsafe`

**Regel.** Pointer gibt es nur in `unsafe { }` und in C-Deklarationen; dort gelten C-Regeln, ein roher Pointer `*T` darf wie in C NULL sein (`p.is_null()`). Strings und Griffe, die C zurückgibt, sind `?T`; Parameter an C sind nicht optional, außer eine `extern`-Deklaration erlaubt `?`. Pointer auf opake C-Typen sind Griffe; nennt der Import einen Freigeber (`release T with f`), werden sie gezählt und `f` läuft genau einmal. Strings an C gelten nur für die Dauer des Aufrufs, ein NUL-Byte darin bricht ab. Nicht genannte Felder eines C-structs sind 0.

*Ausarbeitung mit M4 (`extern`) und v0.1 (`use c`).*

### R18 — Tests

**Regel.** Tests stehen neben dem Code: `test "name" { }`, geprüft mit `expect(cond)`. `dif test` führt sie aus, ab v0.2 auch die Codebeispiele in `///`-Kommentaren.

```ebnf
test_decl = "test" STRING [ "panics" STRING ] block ;
```

Teil 2b legt fest:

- **`test`-Blöcke** stehen nur auf oberster Ebene (R1); in einer Funktion ist `test` `R18`, Meldung `tests are declared at the top level`, Hinweis `move it out of the function`. Ein Test hat keinen Parameter und keinen Rückgabewert; `return` ohne Wert beendet ihn erfolgreich. Alle Deklarationen des Moduls sind darin sichtbar. `dif build` und `dif run` übergehen Tests, `dif check` prüft sie.
- **Der Name** ist ein einfacher String ohne `{ }` und nicht leer; sonst `R18`, Meldung `a test needs a name` bzw. `a test name is plain text`, Hinweis `remove the { }`. Zwei Tests desselben Moduls mit gleichem Namen sind `R18` am zweiten Namen, Meldung `` test "sum" is declared twice ``, Hinweis `first declared at main.dif:1:6`.
- **`expect(cond)`** ist eine eingebaute Funktion (§4) mit einer `bool`-Bedingung (R9). Sie steht nur direkt in einem `test`-Block, auch in `if`, `while`, `for` und `switch` darin; sonst `R18`, Meldung `` `expect` is only for tests ``, Hinweis `use assert(n > 0) in functions` (mit der Bedingung). Ist die Bedingung `false`, scheitert der Test an dieser Stelle; die übrigen Anweisungen laufen nicht mehr.
- **Teilausdrücke.** Die Bedingung wird als Baum gelesen: `&&`, `||`, `!` und Klammern bilden das Gerüst, jeder Vergleich (`==`, `!=`, `<`, `<=`, `>`, `>=`) hat zwei Operanden, jeder andere Ausdruck am Blatt zählt für sich. Zu jedem Operanden und Blatt, das kein Literal und weder `true` noch `false` ist, zeigt der Fehlschlag die Zeile `<Quelltext> = <Wert>`; Werte stehen in der Textform von R28, ein `str` in Anführungszeichen. Jeder Quelltext erscheint einmal, in der Reihenfolge des Quelltexts; was wegen `&&`/`||` nicht ausgewertet wurde, fehlt. Jeder Teilausdruck wird nur einmal ausgewertet.
- **`try` im Test** gilt für jedes `T!E` und `?T`, ohne dass der Test einen Fehlertyp hat (R12). Scheitert es, scheitert der Test an der Stelle des `try` mit `try failed: ` und der Textform des Fehlers bzw. `none`.
- **`test "…" panics "text" { }`** besteht nur, wenn der Test mit einem Panic (R14) abbricht und dessen Meldung (ohne `panic: ` und ohne Position) `text` enthält; nie wird gegen den Quelltext verglichen. Ein leerer Text ist `R18`, Meldung `` panics needs a message text ``, Hinweis `give the part of the panic message to look for`. Ein `expect`, das scheitert, ist kein Panic, sondern ein Fehlschlag des Tests.
- **`dif test [pfad] [filter] [--json]`** übersetzt alle `.dif`-Dateien unter `pfad` (eine Datei oder ein Ordner, rekursiv; Standard `.`) und führt die Tests aus, deren Name `filter` enthält (Groß- und Kleinschreibung zählt; ohne `filter` alle). Der Filter braucht also einen Pfad: `dif test . addition`. **Jeder Test läuft in einem eigenen Prozess**; ein Panic, ein Absturz in C-Code oder eine Zeitüberschreitung (10 Sekunden) trifft nur diesen Test. Die Tests laufen parallel, die Ausgabe steht in der Reihenfolge der Dateien (nach Pfad) und darin der Deklaration. Was ein Test mit `print` ausgibt, wird aufgefangen und nur bei einem Fehlschlag gezeigt. Doc-Tests, `--coverage` und `--sanitize` folgen mit v0.2.
- **Text.** Je Test eine Zeile `ok   <datei>:<zeile> test "<name>"` (der Name wie im Quelltext, maskiert wie ein `str` in einer Textform, R28) bzw. `FAIL <datei>:<zeile> test "<name>"` (`<zeile>` ist die des Worts `test`, der Pfad so, wie ihn `dif test` gefunden hat). Nach einer `FAIL`-Zeile stehen, um 4 Leerzeichen eingerückt: der Grund `<datei>:<zeile>:<spalte>: <Meldung>` (ohne Position nur die Meldung), darunter um 8 eingerückt die Zeilen der Teilausdrücke, und, wenn der Test etwas ausgegeben hat, die Zeile `    output:` mit den Zeilen der Ausgabe, um 8 eingerückt. Zuletzt `test result: <n> passed, <m> failed`. Alles geht auf stdout.
- **Gründe** (Meldung, Position):
  - `expect failed: <Bedingung>`, an `expect`; die Bedingung wie im Quelltext, jeder Zeilenumbruch samt Einrückung durch ein Leerzeichen ersetzt.
  - `try failed: <Fehler>`, an `try`; `none` bei `?T`.
  - `panic: <Meldung>`, an der Position des Panics (R14).
  - `` expected a panic containing "text", but the test finished `` bzw. `` expected a panic containing "text", got "<Meldung>" ``, ohne Position.
  - `killed by signal <n>` und `timed out after 10 s`, ohne Position.
- **Exit-Code von `dif test`:** 0, wenn alle gewählten Tests bestehen; 1, wenn einer scheitert oder keiner gewählt ist (Meldung `error: no tests found` bzw. `` error: no test matches "text" `` auf stderr); 2, wenn die Dateien nicht übersetzbar sind (die Diagnosen wie bei `dif check`, kein Test läuft) oder der Aufruf falsch ist.
- **`--json`** schreibt statt des Texts genau ein JSON-Objekt in einer Zeile auf stdout (Meldungen auf stderr und Exit-Codes bleiben). Es hat `"tests"` (Liste in Textreihenfolge), `"passed"` und `"failed"`. Jeder Test hat `"file"`, `"line"`, `"name"` und `"status"` (`"ok"` oder `"fail"`); ein gescheiterter außerdem `"failure"` mit `"kind"` (`expect`, `try`, `panic`, `panics`, `crash`, `timeout`), `"line"`, `"column"` und `"offset"` (Byte-Offset ab 0; ohne Position `null`), `"message"` (wie im Text, ohne Position), `"values"` (Liste von `{"expr", "value"}`, bei `expect`, sonst leer) und `"output"` (aufgefangenes stdout).

Der Pfad und die Zeilen- und Spaltenangaben stehen in den Beispielen für die Datei `main.dif`; `dif test main.dif` gibt zu diesem Modul aus:

```dif test
int add(int a, int b) {
    return a + b
}

enum ParseErr {
    Empty
}

int!ParseErr first(int n) {
    if n == 0 {
        return err(ParseErr.Empty)
    }
    return n
}

test "addition" {
    expect(add(2, 3) == 5)
    expect(add(-1, 1) == 0)
}

test "first of zero" {
    int n = try first(7)
    expect(n == 7)
    int m = try first(0)
    expect(m == 0)
}

test "overflow" panics "integer overflow" {
    int big = 9223372036854775807
    print(big + 1)
}

test "wrong sum" {
    int a = 2
    print("checking")
    expect(a + 2 == 5 || a == 0)
}
```

```
ok   main.dif:16 test "addition"
FAIL main.dif:21 test "first of zero"
    main.dif:24:13: try failed: ParseErr.Empty
ok   main.dif:28 test "overflow"
FAIL main.dif:33 test "wrong sum"
    main.dif:36:5: expect failed: a + 2 == 5 || a == 0
        a + 2 = 4
        a = 2
    output:
        checking
test result: 2 passed, 2 failed
```

Mit `--json` (eine Zeile, Exit-Code 1):

```
{"tests":[{"file":"main.dif","line":16,"name":"addition","status":"ok"},{"file":"main.dif","line":21,"name":"first of zero","status":"fail","failure":{"kind":"try","line":24,"column":13,"offset":329,"message":"try failed: ParseErr.Empty","values":[],"output":""}},{"file":"main.dif","line":28,"name":"overflow","status":"ok"},{"file":"main.dif","line":33,"name":"wrong sum","status":"fail","failure":{"kind":"expect","line":36,"column":5,"offset":523,"message":"expect failed: a + 2 == 5 || a == 0","values":[{"expr":"a + 2","value":"4"},{"expr":"a","value":"2"}],"output":"checking\n"}}],"passed":2,"failed":2}
```

```dif error R18
void check(int n) {
    expect(n > 0)
}
```

```
main.dif:2:5: error[R18]: `expect` is only for tests
  hint: use assert(n > 0) in functions
```

```dif error R18
test "sum" {
    expect(1 + 1 == 2)
}

test "sum" {
    expect(2 + 2 == 4)
}
```

```
main.dif:5:6: error[R18]: test "sum" is declared twice
  hint: first declared at main.dif:1:6
```

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

*Offene Frage:* speichert ein Griff (`shared`, Kanal) Funktionswerte, die ihn selbst eingefangen haben, entsteht ein Zyklus; R26 schließt Zyklen nur für Werte aus.

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

**Regel.** Ein Funktionswert hat den Typ `R(T1, T2)` und wird geschrieben wie eine Funktion ohne Namen: `R(T1 a, T2 b) { … }`. Ein Literal steht nur direkt als Argument eines Aufrufs (nie in einer Variable, einem struct, einer Liste, in `spawn` oder als Typargument) und lebt nur während dieses Aufrufs. Beim Auswerten nimmt es eine Momentaufnahme (eine Kopie) jedes Namens ringsum, den es benutzt, und ändert nie etwas ringsum. Ein Literal ohne Parameter darf als Block `{ … }` direkt hinter der schließenden Klammer stehen; `return` und `try` darin beenden den Block. Benannte Funktionen und Varianten-Konstruktoren haben keine Umgebung; ab M3 sind sie gewöhnliche Werte, die man speichern darf, in M2 gehen sie nur als Argument an einen Funktionstyp; Literale kommen danach auch als Werte (unten, „Ab M3“). Ändert ein Funktionswert einen Parameter, steht `mut` in seinem Typ.

```ebnf
func_type   = type "(" [ ftype_param { "," ftype_param } [ "," ] ] ")" ;   (* nur als Typ eines Parameters, R2 *)
ftype_param = [ "mut" ] type ;
func_lit    = type "(" [ lit_param { "," lit_param } [ "," ] ] ")" block ;   (* nur als arg, §2 *)
lit_param   = [ "mut" ] type IDENT ;
```

**Grammatik:** `func_type`, `func_lit` hier; `param` (R2), `arg` und das Block-Argument am Ende von `postfix` (§2), `block` (R3).

Teil 2d legt fest, was zu M2 gehört: Literale und Blöcke an Aufrufen, Funktionstypen als Typ eines Parameters, die Methoden aus §4.7. Speichern von Funktionen in Variablen, Feldern und Listen, benannte Parameter (R25) und eigene Generics (R27) folgen mit M3; Funktionstypen in `extern`-Deklarationen und C-Callbacks (R24) mit einer späteren Scheibe. Bis dahin passt an einen Funktionstyp ein Literal, eine benannte Funktion oder ein Varianten-Konstruktor.

**Literal.**

- Ein Literal hat den Rückgabetyp vorn (R2), dann Parameter wie eine Funktion (ohne Funktionstypen unter ihnen) und einen Rumpf: `bool(Person p) { return p.age >= 18 }`. Es ist kein Wert, den man ablegen kann: es steht nur als Argument. **Erkennung.** Ein Argument ist ein Literal, wenn es mit `X (` beginnt, direkt danach `mut` oder ein `type` mit Namen (`int x`, `mut int x`) steht oder die Klammer leer ist, und hinter der `)` ein `{` folgt. „Dieselbe Zeile“ heißt dabei die Zeile im Quelltext: in Klammern erzeugt der Lexer kein Zeilenende (§1.3), die Regel prüft die physische Zeile. Die Form entscheidet so: `mut` oder `type Name` nach der `(` ist nur bei einem Literal möglich (ein Argument ist ein Ausdruck, `int x` ist keiner); beginnt `X` mit `?`, `[` oder `map[` oder ist ein eingebauter Typname, ist es immer ein Literal. Nur `Name() { … }` mit einem Namen und leerer Klammer kann beides sein, ein Literal ohne Parameter mit dem Rückgabetyp `Name` oder ein Aufruf von `Name` mit Block-Argument. Das entscheidet der Checker über den Namen: benennt er einen Typ (`struct`, `enum`), ist es ein Literal, benennt er eine Funktion oder Methode, ein Aufruf mit Block. Ein Name ist nie beides (R4). `Name(x) { … }` mit Ausdrücken ist immer ein Aufruf mit Block, `T(x)` ohne `{` die Umwandlung (R31).
- Sein Typ ist `R(T1, T2)`; er muss dem Funktionstyp des Parameters genau gleichen: Rückgabetyp, Parametertypen und `mut` (Namen zählen nicht). Sonst `R8` am Literal, Meldung `` expected bool(int), found int(int) ``, ohne Hinweis. In Meldungen stehen Funktionstypen so: `bool(int)`, `void(mut App, Msg)`, `int!ParseErr(int)`.
- **Außerhalb eines Arguments** ist ein Literal `R26` an seinem ersten Zeichen, Meldung `` a function literal can only be an argument of a call ``, Hinweis `pass it directly as an argument of a call`; sein Rumpf wird dann nicht weiter geprüft. Das gilt für jede Stelle: Initialwert, Zuweisung, `return`, Operand, Listen- oder Struct-Literal, `_ =`.
- Der Rumpf ist ein eigener Funktionsrumpf (R3, R8): `return` verlässt nur das Literal und liefert einen `R`; ein Literal mit Rückgabetyp endet auf jedem Weg mit `return` und Wert, sonst `R8` am ersten Zeichen des Literals, Meldung `` the literal does not return a value on every path ``, Hinweis `end it with return, e.g. return false` (wie bei Funktionen mit `0`, `false`, `""` oder `…`). `try` gilt für das Literal wie für eine Funktion (R12): es ist `R12`, wenn der Rückgabetyp `R` kein `T!E` oder `?T` ist; ein Fehler beendet nur das Literal, nicht die Funktion ringsum. `break` und `continue` sehen keine Schleife außerhalb des Literals: sie sind dort `R11` (`` `break` outside a loop ``), auch wenn das Literal in einer Schleife steht.
- **Namen.** Parameter und Variablen des Literals folgen R4 ohne Ausnahme: ein Name, der an dieser Stelle schon gilt (auch ein Parameter der umgebenden Funktion oder eines äußeren Literals), wird nicht noch einmal deklariert. Literale dürfen in Literalen stehen; ein inneres sieht die Namen aller äußeren.

**Funktionstypen als Parameter.**

- Ein Funktionstyp `R(T1, mut T2)` steht nur als Typ eines Parameters einer Funktion oder Methode: `void times(int n, void() body)`. Überall sonst (Variable, Feld, Rückgabetyp, Elementtyp, Typ eines Literal-Parameters) ist er `R26` am Typ, Meldung `` a function type is only allowed for a parameter ``, ohne Hinweis. Ein `?` oder `!E` vor der Klammer gehört zum Rückgabetyp `R`; `void` darf der Rückgabetyp sein.
- Im Rumpf ist ein solcher Parameter ein Name, der nur aufgerufen oder als Argument weitergereicht wird: `body()`, `f(x)`, `each(xs, f)`. Aufruf und Argumente folgen den Regeln für Funktionen (R6, R8): `` `f` takes 1 argument, found 2 ``, `mut` am Argument genau dort, wo der Typ es verlangt; ein `mut` im Funktionstyp zählt für R6 als Änderung des Orts, auch wenn kein Rumpf zu sehen ist. Jede andere Benutzung (zuweisen, vergleichen, zurückgeben, in ein struct legen) ist `R26` am Namen, Meldung `` `f` is a function and can only be called or passed as an argument ``, ohne Hinweis. Darum kann in M2 nichts über das Ende des Aufrufs hinaus leben: ein Literal wird nie gespeichert oder zurückgegeben.
- **Benannte Funktionen und Konstruktoren.** An der Stelle eines Funktionstyps darf statt eines Literals der Name einer Funktion oder ein Varianten-Konstruktor stehen (`xs.filter(is_even)`, `xs.map(Shape.Box)`); sein Typ ist der der Deklaration (`bool(int)`, `Shape(int)`, mit `mut` wie dort) und muss genau passen (`R8` wie beim Literal). Für die Wirkung (R13) zählt die übergebene Funktion wie ein Literal. Eine Funktion ist sonst kein Wert: jede andere Benutzung ihres Namens als der Aufruf (`_ = is_even`, Zuweisung, `return`, Feld) ist `R26` am Namen, mit derselben Meldung wie bei einem Funktionsparameter, `` `is_even` is a function and can only be called or passed as an argument ``, ohne Hinweis. Speichern in Variablen, Feldern und Listen kommt mit M3 (unten, „Ab M3“).
- Ein Aufruf eines Funktionsparameters hat selbst keine Wirkung (R13): ein Ergebnis, das nicht `void` ist, wird verwendet oder mit `_ =` verworfen.

**Block-Argument.**

- Ein Block steht direkt hinter der `)` eines Aufrufs, mit dem `{` in derselben Zeile: `times(3) { … }`, `ui.tiles() { … }`, `str body = try net.retry(3) { … }`. Er ist ein Literal ohne Parameter und zugleich das **letzte Argument**: sein Typ ist der Typ des letzten Parameters der Funktion, der `R()` sein muss. Die Klammern bleiben, auch wenn sie leer sind. Ein Block steht nur hinter einer `)`; `a.b { … }` ist `S1`. Nach dem schließenden `}` endet der Ausdruck (die Kette `postfix`, §2); ein weiteres `.name` oder `(` ist `S1`.
- Der Block zählt als Argument: hat der Aufruf dann zu viele, ist es `R8` (`` `times` takes 2 arguments, found 3 ``). Ist der letzte Parameter kein `R()` (ein Parameter anderen Typs oder ein Funktionstyp mit Parametern), ist es `R26` am `{`, Meldung `` `filter` cannot take a block: its last parameter has type bool(int) `` (bei `[]int.push` mit Typ `int`), Hinweis bei einem Funktionstyp `a block has no parameters; pass a literal instead`, sonst keiner; der Block wird dann nicht weiter geprüft. Typen im Block (`return`) prüft R8 mit dem `R` des Parameters.
- Steht das `{` erst in der nächsten Zeile, ist das `R3` an der Stelle direkt hinter der `)`, Meldung `` the block argument of `times` must start on the same line ``, Hinweis: die Zeile mit `{` am Ende; der Block wird danach gelesen wie ein Block-Argument, es folgt keine Meldung über die Argumente.
- **Kopfzeilen** (R3): im Kopf von `if`, `while`, `for` und `switch` beendet ein `{` außerhalb von Klammern den Kopf, wie beim Struct-Literal. Ein Literal steht in den Klammern seines Aufrufs und ist darum im Kopf überall erlaubt: `if xs.any(bool(int x) { return x > 1 }) {`. Ein Block-Argument im Kopf braucht Klammern um den ganzen Aufruf: `if (ready() { … }) {`; ohne sie ist das `{` der Rumpf, und der Aufruf hat ein Argument zu wenig (`R8`). Der Hinweis `a block in a header needs parentheses around the call: if (ready() { … }) {` (mit dem Aufruf des Programms) steht, wenn dieser Aufruf unmittelbar vor dem `{` des Kopfes endet, außerhalb von Klammern steht und der letzte Parameter der Funktion ein Funktionstyp `R()` ohne Parameter ist, der das fehlende Argument wäre; sonst gibt es nur die Meldung über die Argumentzahl.
- **Kanonisch.** Ein Literal ohne Parameter als letztes Argument ist dasselbe wie ein Block (`times(3, void() { … })` und `times(3) { … }`); der Compiler nimmt beides an, `dif fmt` schreibt den Block, in einer Kopfzeile dagegen das Literal (eine Regel ohne Tiefe: der Kopf enthält nie einen Block). Klammern, die dadurch nichts mehr bewirken, entfernt `dif fmt` (R3); notwendige bleiben, es entfernt nie nur Klammern, die ein Struct- oder Map-Literal oder einen Block im Kopf schützen. Ein Rumpf aus genau einer Anweisung ohne eigenen Block steht einzeilig, wenn die Zeile dann höchstens 100 Zeichen lang ist, sonst mehrzeilig; mehrere Anweisungen oder eine Anweisung mit eigenem Block (`if`, `for` …) stehen mehrzeilig, ein Block immer.

**Umgebung: Werte einfangen (R6).**

- Ein Literal sieht alle Namen, die an seiner Stelle gelten (Variablen, Parameter, in einer Methode `self` der Methode, die Namen äußerer Literale), nicht ein eigenes `self`. Es **fängt ein**: beim Auswerten des Literals, also wenn der Aufruf seine Argumente auswertet (links nach rechts, mit den anderen Argumenten), wird jeder benutzte Name in seinen Wert kopiert. Der Rumpf liest nur diese Kopien, nie den Ort ringsum: eine Zuweisung zwischen zwei Aufrufen ändert ein Literal, das schon eingefangen hat, nicht; ein neu ausgewertetes Literal sieht den neuen Wert. Ein inneres Literal fängt auch ein, was es von äußeren Literalen benutzt; das äußere fängt es dann ebenfalls ein. `const`-Namen und Funktionen werden nicht eingefangen.
- Kopieren ist billig und wie überall Copy-on-Write (R5); eine Kopie eines Griffs (R5) ist derselbe Griff. Ein benutztes Feld `p.x` fängt das ganze `p` ein; eingefangen wird mit einem Zählerschritt, nie mit einer tiefen Kopie (Copy-on-Write). Ein Literal in einer Schleife fängt bei jeder Auswertung neu ein. Darum geht, was eine Wirkung auf etwas außerhalb braucht, über einen Griff: `conn.transaction() { try conn.debit(from, amount) }` oder `page.list() { page.li(…) }` (PLAN §4.10); wer eine Zahl oder Liste ringsum ändern will, schreibt eine `for`-Schleife oder gibt sie als Parameter hinein.
- Ein Literal **ändert nie** etwas ringsum, ohne Ausnahme: jede Änderung an einem eingefangenen Namen ist `R6` (Regel und Meldung: R6, Teil 2d). Es braucht darum kein `mut` am Literal, und für „Keine Aliase“ (R6) zählt es nicht.
- Die Parameter des Literals sind Kopien wie bei Funktionen (R5, R6); `mut` steht nur, wo der Funktionstyp es verlangt. Ein `mut`-Parameter des Literals ändert den Ort, den der Funktionsrumpf mit `f(mut x)` übergibt, nicht einen Namen ringsum.
- **Wirkung** (R13): ein Literal hat eine Wirkung, wenn sein Rumpf eine hat (`print`, Aufruf mit Wirkung, Methode auf einem Griff); ein Aufruf hat sie, wenn die Funktion oder eines seiner Literale sie hat. So darf `xs.any(bool(int x) { print(x) … })` als Anweisung stehen, `xs.filter(bool(int x) { return x > 1 })` nicht.
- **Lebendigkeit** (R6): was das Literal liest, gilt am Aufruf als gelesen (die Kopie entsteht dort).
- Aliasregel: entfällt für Literale (R6, Teil 2d).

**Ab M3: Funktionswerte speichern.** Festgelegt, in M2 noch nicht benutzbar. Benannte Funktionen und Varianten-Konstruktoren kommen zuerst als gespeicherte Werte (Variable, Feld, Liste, `return`), Literale danach; ein Literal wird dann ein Wert aus Funktion und eingefangenen Kopien (in C ein Paar `{fn, env}`, `env` fehlt bei benannten Funktionen).

- **Gleichheit** (R28): zwei Funktionswerte sind gleich, wenn sie von derselben Quelltextstelle stammen (Datei, Zeile und Spalte des Literals; bei einer benannten Funktion dieselbe Funktion) und alle eingefangenen Werte gleich sind. Ein Literal ist nie gleich einer benannten Funktion.
- **Textform** (R28): ein Literal `fn@main.dif:12:5` (Pfad wie in Meldungen, Zeile und Spalte seiner Stelle, also genau die Stelle, die auch die Gleichheit meint); eine benannte Funktion oder ein Varianten-Konstruktor ihr Name (`add`, `Shape.Box`).
- **Keine Zyklen:** ein Literal fängt Werte ein, die beim Auswerten schon existieren, und kann sich nie selbst enthalten; ein Wert verweist nie auf einen späteren. Darum kommen Funktionswerte ohne GC aus.

```dif
void times(int n, void() body) {
    for int i in range(n) {
        body()
    }
}

int sum_with([]int xs, int(int) f) {
    int total = 0
    for int x in xs {
        total += f(x)
    }
    return total
}

void main() {
    times(3) {
        print("hi")
    }
    int factor = 10
    print(sum_with([1, 2, 3], int(int x) { return x * factor }))
    factor = 100
    print(sum_with([1, 2, 3], int(int x) { return x * factor }))
}
```

Ausgabe: `hi`, `hi`, `hi`, `60`, `600`.

```dif
enum ParseErr {
    Negative(int n)
}

int!ParseErr check(int n) {
    if n < 0 {
        return err(ParseErr.Negative(n))
    }
    return n
}

void main() {
    []int xs = [1, -2, 3]
    [](int!ParseErr) rs = xs.map(int!ParseErr(int x) {
        int v = try check(x)
        return v * 2
    })
    print(rs)
    if xs.any(bool(int x) {
        if x < 0 {
            return true
        }
        return false
    }) {
        print("has negative")
    }
}
```

Ausgabe: `[ok(2), err(ParseErr.Negative(-2)), ok(6)]`, `has negative`.

Ein Literal liest die Kopie von vor dem Aufruf: während `sort_by` die Liste umordnet, sieht das Literal `xs` noch wie zuvor (`xs[0]` ist immer `3`):

```dif snippet
[]int xs = [3, 1, 2]
xs.sort_by(bool(int a, int b) { return a < b && xs[0] == 3 })
print(xs)
```

Ausgabe: `[1, 2, 3]`.

```dif error R26
void main() {
    _ = bool(int x) { return x > 1 }
}
```

```
main.dif:2:9: error[R26]: a function literal can only be an argument of a call
  hint: pass it directly as an argument of a call
```

```dif
enum Shape {
    Dot
    Box(int size)
}

bool is_even(int n) {
    return n % 2 == 0
}

void main() {
    []int xs = [1, 2, 3, 4]
    print(xs.filter(is_even))
    print(xs.map(Shape.Box))
}
```

Ausgabe: `[2, 4]`, `[Shape.Box(1), Shape.Box(2), Shape.Box(3), Shape.Box(4)]`.

```dif error R26
bool is_even(int n) {
    return n % 2 == 0
}

void main() {
    _ = is_even
}
```

```
main.dif:6:9: error[R26]: `is_even` is a function and can only be called or passed as an argument
```

Beide Formen von `Name() { … }` (Literal mit dem Typ `Shape`, Aufruf der Funktion `one` mit Block):

```dif
enum Shape {
    Dot
    Box(int size)
}

int one(void() body) {
    body()
    return 1
}

int plus(int n) {
    return n + 1
}

void main() {
    print([1, 2].map(Shape(int n) { return Shape.Box(n) }))
    print(plus(one() { print("hi") }))
}
```

Ausgabe: `[Shape.Box(1), Shape.Box(2)]`, `hi`, `2`.

Ein Funktionstyp mit `mut`: das Literal trägt es ebenfalls, der Aufruf `f(mut x)` ändert `x`:

```dif
void apply_to(mut int x, void(mut int) f) {
    f(mut x)
}

void main() {
    int n = 1
    apply_to(mut n, void(mut int v) { v += 10 })
    print(n)
}
```

Ausgabe: `11`.

```dif error R8
bool ready(bool() check) {
    return check()
}

void main() {
    if ready() {
        print("go")
    }
}
```

```
main.dif:6:8: error[R8]: `ready` takes 1 argument, found 0
  hint: a block in a header needs parentheses around the call: if (ready() { … }) {
```

```dif error R8
void main() {
    []int xs = [1, 2, 3]
    print(xs.filter(int(int x) { return x }))
}
```

```
main.dif:3:21: error[R8]: expected bool(int), found int(int)
```

```dif error R3
void times(int n, void() body) {
    for int i in range(n) {
        body()
    }
}

void main() {
    times(3)
    {
        print("hi")
    }
}
```

```
main.dif:8:13: error[R3]: the block argument of `times` must start on the same line
  hint: times(3) {
```

```dif error R26
void main() {
    []int xs = [1, 2, 3]
    print(xs.filter() {
        return true
    })
}
```

```
main.dif:3:23: error[R26]: `filter` cannot take a block: its last parameter has type bool(int)
  hint: a block has no parameters; pass a literal instead
```

```dif error R6
void main() {
    []int xs = [3, 1, 2]
    xs.sort_by(bool(int a, int b) {
        xs.push(a)
        return a < b
    })
    print(xs)
}
```

```
main.dif:4:9: error[R6]: the change to `xs` is lost: `xs` is a copy
  hint: a function literal sees a copy; return the value or use a for loop
```

```dif error R6
void main() {
    []int xs = [3, 1, 2]
    int last = 0
    []int big = xs.filter(bool(int x) {
        last = x
        return x > 1
    })
    print(big)
    print(last)
}
```

```
main.dif:5:9: error[R6]: the change to `last` is lost: `last` is a copy
  hint: a function literal sees a copy; return the value or use a for loop
```

```dif error R4
void main() {
    []int xs = [1, 2, 3]
    int x = 2
    print(xs.filter(bool(int x) { return x > 1 }))
}
```

```
main.dif:4:30: error[R4]: `x` is already declared at main.dif:3:9
  hint: names are not redeclared inside their block; choose another name
```

```dif error R11
void main() {
    []int xs = [1, 2, 3]
    for int n in range(2) {
        if xs.any(bool(int x) {
            if x > n {
                break
            }
            return false
        }) {
            print(n)
        }
    }
}
```

```
main.dif:6:17: error[R11]: `break` outside a loop
```

### R27 — Generics

**Regel.** Typparameter stehen in eckigen Klammern hinter dem Namen und werden beim Aufruf meist abgeleitet; jede Instanz wird eigener Code. Eine Einschränkung steht vor dem Typparameter wie ein Typ vor dem Namen: `equal`, `ordered`, `hashable`, `text`.

*Ausarbeitung mit M3.*

### R28 — Fähigkeiten

**Regel.** Jeder Typ ist `equal` (`==`) und `text` (Ausgabe, `"{x}"`) (Funktionswerte: Teil 2d); `ordered` (`<`) sind Zahlen, `rune` und `str`; `hashable` ist alles, was `equal` ist. Griffe vergleichen ihre Identität, Maps sind gleich, wenn ihre Einträge gleich sind.

Teil 1 legt die Textform fest: `int` dezimal mit `-` für negative Zahlen, `bool` als `true`/`false`, `str` unverändert. `str` vergleicht `<` bytweise (UTF-8).

Teil 2a ergänzt:

- Ein `struct` ist `equal`, wenn alle Felder es sind; `==` vergleicht Feld für Feld. Ein `enum` vergleicht Variante und Werte. `<` gibt es für beide nicht (`R8`).
- Die Textform sieht aus wie das Literal: `Point{x: 1, y: 2}` (Felder in Deklarationsreihenfolge), `Seat.Taken`, `BookErr.NoSuchSeat(3, 5)`. Ein `str` darin steht in Anführungszeichen mit Escapes (`Person{name: "Ada"}`); allein ausgegeben bleibt ein `str` unverändert.

Teil 2b ergänzt `?T` und `T!E`:

- Sie sind `equal` und `text`, wenn `T` (und `E`) es sind. `==` vergleicht: zwei `none` sind gleich, zwei Werte, wenn ihre Werte gleich sind; bei `T!E` sind zwei Fehler gleich, wenn ihre Fehler gleich sind, ein Wert und ein Fehler nie. `==` und `!=` verlangen gleiche Typen (R8). `x == 3` mit `x` vom Typ `?int` ist `R8`, Meldung `` cannot apply `==` to ?int and int ``, Hinweis `x == some(3)`; `r == 3` mit `r` vom Typ `int!E` ebenso (`` cannot apply `==` to int!E and int ``), Hinweis `use switch to look at the value`. `none` und `err(e)` nehmen den Typ des anderen Operanden an. `<` gibt es nicht (`R8`).
- Die Textform nennt die Form wie ein Muster (R10): `some(4)`, `none`, `ok(30)`, `err(AgeErr.Negative)`; bei `void!E` `ok` ohne Klammern. Ein `str` darin steht in Anführungszeichen (`some("a")`).

Teil 2c ergänzt Zahlen, `rune`, Listen, Arrays und Maps:

- **Fähigkeiten.** Alle Zahlentypen und `rune` sind `equal`, `text`, `ordered` und `hashable` (auch `rune` ist `ordered`); `bool` und `str` bleiben, wie sie waren. `[]T`, `[N]T` und `map[K]V` sind `equal`, `text` und `hashable`, wenn ihre Elemente es sind, aber nie `ordered`.
- **Gleichheit.** `==` an `f64` ist IEEE (`NaN != NaN`, `0.0 == -0.0`). Listen und Arrays sind gleich bei gleicher Länge und gleichen Elementen in Reihenfolge, Maps bei gleichen Schlüsseln mit gleichen Werten, gleich in welcher Reihenfolge. Ein `[]T` und ein `[N]T` vergleicht man nicht (`R8`). Weil `NaN != NaN`, ist eine Liste mit `NaN` nie gleich zu sich selbst.
- **Text.** `int` und die anderen Ganzzahltypen dezimal. `f64`: R29. `rune` allein ausgegeben (`print(r)`, `"{r}"`) ist das Zeichen unverändert, in einer Textform (Liste, Struct, `some(…)`) steht es in `'`: `['a', 'b']`. Listen und Arrays `[1, 2, 3]` (leer `[]`), Maps `{"a": 1, "b": 2}` in der Reihenfolge der Einfügung (leer `{}`), die Elemente jeweils in ihrer Textform. Ein `str` in einer Textform steht in `"`; maskiert werden `"`, `\`, Zeilenumbruch, Tabulator, Wagenrücklauf (`\"`, `\\`, `\n`, `\t`, `\r`), NUL (`\0`) und die übrigen Steuerzeichen unter 0x20 als `\u{…}`; Klammern bleiben unmaskiert. Ein `rune` in einer Textform maskiert dasselbe, statt `"` aber `'`.
- **Schlüssel.** Zwei Schlüssel einer Map sind gleich, wenn `==` sie gleich nennt; `NaN` ist kein Schlüssel (R14).
- **Sortieren.** `sort` braucht `ordered`; andere Typen sind `R28`, Meldung `` `sort` needs an ordered element type, found P ``, Hinweis `only numbers, rune and str can be sorted`. `contains`, `index_of` und `count` verlangen `equal`; das ist in Teil 2c jeder Typ; Funktionswerte (R26) sind es ab M3 nach Teil 2d. Beispiel in §4.4.

Teil 2d ergänzt Funktionswerte (R26), ab M3, wenn sie gespeichert werden dürfen (in M2 kommen sie nur als Argument vor und werden weder verglichen noch ausgegeben):

- **Fähigkeiten.** Ein Funktionswert ist `equal`, `text` und `hashable` (wenn alle eingefangenen Werte es sind), nie `ordered`.
- **Gleichheit.** Gleiche Quelltextstelle des Literals (Datei, Zeile, Spalte; bei einer benannten Funktion dieselbe Funktion) und gleiche eingefangene Werte (nach diesem Abschnitt); ein Literal ist nie gleich einer benannten Funktion.
- **Text.** Ein Literal `fn@main.dif:12:5`: das Wort `fn`, `@`, der Pfad so, wie ihn Meldungen nennen, Zeile und Spalte des Literals. Eine benannte Funktion oder ein Varianten-Konstruktor ist sein Name (`add`, `Shape.Box`). Die eingefangenen Werte erscheinen nicht.

### R29 — Zahlen

**Regel.** Zahlen: Ganzzahl-`/` rundet Richtung 0, `%` hat das Vorzeichen des Dividenden; Floats folgen IEEE 754 (`inf`, `NaN`, `NaN == NaN` ist false); Ganzzahl-Division durch 0, `INT_MIN / -1` und Shifts ab der Bitbreite brechen ab. Ausdrücke und Argumente werden von links nach rechts ausgewertet.

**Grammatik:** `INT` aus §1.4 und die Operatoren aus §2.

Teil 1 legt den Ganzzahl-Teil fest:

- `int` ist immer 64 Bit mit Zweierkomplement, Bereich `-9223372036854775808 … 9223372036854775807`.
- `a / b` rundet Richtung 0, `a % b` hat das Vorzeichen von `a`, es gilt `(a / b) * b + a % b == a`. `INT_MIN % -1` ist `0`.
- `<<` und `>>` verschieben um `0 … 63` Stellen; `>>` ist arithmetisch (füllt mit dem Vorzeichen). `<<` rechnet auf den 64 Bits: was über Bit 63 hinausgeschoben wird, geht verloren, ohne Abbruch, beim Kompilieren wie zur Laufzeit. Andere Weiten brechen ab (R14).
- `+% -% *%` rechnen modulo 2⁶⁴.
- Auswertungsreihenfolge: Operanden von links nach rechts, Argumente von links nach rechts, bei einer Zuweisung erst der Ort, dann der Wert. Bei `xs[f()] = g()` heißt das: zuerst die Teilausdrücke des Ortes von links nach rechts (`f()`, auch verschachtelte Indizes und Felder), dann der Wert (`g()`), erst danach die Indexprüfung (R14) und das Schreiben; ändert `g()` die Liste, gilt die Prüfung für die Liste nach `g()`. Dasselbe gilt für `+=` und die anderen Zuweisungsoperatoren (der alte Wert wird erst nach `g()` gelesen) und für `m[k] = v` (erst `k`, dann `v`).

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

### R29, Teil 2c — Breiten und `f64`

**Regel.** R29 gilt für alle Zahlentypen (R8, Teil 2c).

**Grammatik:** `FLOAT` und `format` (§1.4), die Operatoren aus §2.

Teil 2c legt fest:

- **`i8 … u64`** rechnen wie `int` auf ihrer Breite: `+ - *` und Vorzeichen `-` brechen ab, wenn das Ergebnis nicht in den Typ passt (`integer overflow in +`); `/` rundet Richtung 0, `%` hat das Vorzeichen des Dividenden, Division und `%` durch 0 brechen ab, ebenso `MIN / -1` (`-128 / -1` bei `i8`; `MIN % -1` ist `0`). `+% -% *%` rechnen modulo 2^Bits des Typs. `~` kehrt die Bits des Typs um. `<<` und `>>` verschieben um `0 … Bits-1` Stellen, andere Weiten brechen ab (`shift by 8 out of range 0..7` bei `u8`); `<<` verliert Bits über der Breite ohne Abbruch, `>>` füllt bei Typen mit Vorzeichen mit dem Vorzeichen, bei `u8 … u64` mit 0. Die Weite hat den Typ des Operanden (R8).
- **`f64`** ist IEEE 754 binary64 mit Rundung zum nächsten Wert, bei Gleichstand zum geraden. `+ - * /`, `%` (wie C `fmod`, Vorzeichen des Dividenden) und das Vorzeichen `-` brechen nie ab: ein Überlauf ist `inf` (ein Literal, das `inf` oder fälschlich 0 würde, ist dagegen `R8`, siehe dort), `1.0 / 0.0` ist `inf`, auch beim Kompilieren, `0.0 / 0.0` und `x % 0.0` sind `NaN`. Jede Operation rundet für sich auf `f64`: der Compiler fasst `a * b + c` nicht zusammen (kein FMA) und rechnet nie mit größerer Breite. Vergleiche mit `NaN` sind `false`, außer `!=`.
- **Textform von `f64`:** die kürzeste Dezimalfolge, die beim Zurücklesen (Literal, `parse_f64`) denselben Wert ergibt. Hat der Wert die wissenschaftliche Schreibweise `d.ddd × 10^e`, steht er ohne Exponenten, wenn `-6 ≤ e ≤ 20` (`5`, `22.5`, `0.000001`, `100000000000000000000`), sonst als `d.ddde<e>` ohne `+` und führende Nullen (`1e21`, `1.5e-7`). Ein ganzer Wert hat keinen Dezimalpunkt. `0`, `-0`, `inf`, `-inf` und `NaN` stehen so da.
- **Formatangaben** (§1.4) gelten nur für `f64`; an einem anderen Typ ist die Angabe `R8` am Ausdruck, Meldung `` the format .2f needs f64, found int ``, Hinweis `f64(n)`. `{x:.Nf}` schreibt `N` Nachkommastellen genau wie C `printf("%.Nf")` (der exakte Wert wird gerundet, bei genauer Mitte zur geraden Ziffer: `{2.5:.0f}` ist `2`); `{x:g}` schreibt wie C `%g` (sechs gültige Ziffern, ohne überflüssige Nullen, Exponent als `e+21`). `inf`, `-inf` und `NaN` schreiben beide wie die Textform.

```dif snippet
u8 b = 200
print(b +% 100)
f64 x = 22.5
print(x)
print(0.1 + 0.2)
print(7.0 / 2)
print(1.0 / 0.0)
f64 pct = 9.0 / 40.0 * 100
print("{pct:.1f} %")
print("{x:g}")
print(int(x.round()))
print(1e21)
print(0.0000001)
print(100.0)
```

Ausgabe: `44`, `22.5`, `0.30000000000000004`, `3.5`, `inf`, `22.5 %`, `22.5`, `23`, `1e21`, `1e-7`, `100`.

```dif panic "integer overflow in +"
u8 a = 200
u8 b = 100
print(a + b)
```

```
panic: integer overflow in + (main.dif:4:13)
```

```dif error R8
void main() {
    int n = 3
    print("{n:.2f}")
}
```

```
main.dif:3:13: error[R8]: the format .2f needs f64, found int
  hint: f64(n)
```

### R30 — Maps

**Regel.** `map[K]V` schreibt man `{k: v, …}`; `m[k]` liefert `?V`, `m[k] = v` setzt, die Reihenfolge ist die der Einfügung.

```ebnf
map_lit = "{" { NL } [ entry { "," { NL } entry } [ "," ] { NL } ] "}" ;
entry   = expr ":" expr ;
```

**Grammatik:** `map_type` (R2), `map_lit` und `entry` hier, `m[k]` als `postfix` (§2) und `place` (R3). In einer Kopfzeile steht ein Map-Literal in Klammern (R3).

Teil 2c legt fest:

- **Typ und Literal.** `map[K]V` verbindet Schlüssel `K` mit Werten `V`; `K` ist `hashable` (R28), `V` ein beliebiger Typ. `{"a": 1, "b": 2}` hat die Typen der Stelle (R8); ohne Stelle die des ersten Eintrags, bei Schlüsseln und Werten je für sich nach der Regel der Liste (R5, Teil 2c: erstes Element, das kein Literal ist); ein späterer Schlüssel oder Wert anderen Typs ist `R8`, Meldung `` expected str, found int `` am Ausdruck. Das leere `{}` braucht eine Stelle (`map[str]int m = {}`), sonst `R8`, Meldung `` `{}` needs a place of type map[K]V ``, ohne Hinweis. Steht ein Schlüssel zweimal im Literal, ist das `R30` am zweiten, wenn beide Literale oder `const` sind (Meldung `` duplicate key "a" ``, Hinweis `first used at main.dif:2:22`); sonst setzt der spätere Eintrag den Wert des früheren, wie `m[k] = v`. Mehrzeilig und mit abschließendem Komma wie `struct_lit` (R5, kanonisch §0).
- **Lesen.** `m[k]` hat den Typ `?V`: `some(v)`, wenn `k` vorkommt, sonst `none`; nie ein Abbruch. `k` hat den Typ `K` (R8). Man benutzt es mit `??`, `switch` oder `try` (R12): `counts[w] ?? 0`.
- **Schreiben.** `m[k] = v` ersetzt den Wert von `k` oder fügt den Eintrag hinten an. Für R6 gilt `m[k] = v` als Änderung von `m`, auch wenn `m[k]` kein Ort ist. Ersetzen ändert die Stelle des Eintrags nicht; wer `k` mit `remove` entfernt und wieder setzt, steht hinten. `m[k] += 1` und die anderen Zuweisungsoperatoren sind `R8` am Operator, Meldung `` cannot apply `+=` to ?int and int ``, Hinweis bei `+=` mit Zahlenwerten `counts.add("a", 1)` (§4.5), bei den anderen Operatoren `counts["a"] = (counts["a"] ?? 0) - 1` (mit `m[k]` aus dem Quelltext).
- **Kein Ort.** Ein Eintrag wird nur durch `m[k] = v` ersetzt. `m[k].f = v`, `m[k].push(x)` (eine Methode, die `self` ändert) und `mut m[k]` sind `R30` am Anfang des Orts, Meldung `` a map entry is changed by assigning it ``, Hinweis `read it, change the copy, and store it: m[k] = v`.
- **Reihenfolge.** Einträge liegen in der Reihenfolge der Einfügung; `for K k, V v in m`, `m.keys()`, `m.values()` und die Textform halten sie ein. Die Ausgabe eines Programms ist darum bei gleicher Eingabe immer gleich.
- **Schlüssel.** `K` ist jeder Typ, der `equal` ist; zwei Schlüssel sind dieselben, wenn `==` sie gleich nennt (`0.0` und `-0.0` sind derselbe Schlüssel). Ein Schlüssel, der irgendwo ein `NaN` enthält (`f64`, Liste, Struct), bricht ab (R14, `NaN cannot be a map key`), und zwar bei jeder Operation, die einen Schlüssel nimmt: `m[k] = v`, `m[k]`, `m.has(k)` und `m.remove(k)`; die Position ist die des `[` bzw. des Methodennamens. Es gibt nur diese eine Regel, auch wenn die Map leer ist. Gleichheit, Textform: R28; Methoden: §4.5; Kopie und Schnappschuss: R5, Teil 2c.

```dif
void main() {
    map[str]int counts = {}
    for str w in "to be or not to be".split(" ") {
        counts[w] = (counts[w] ?? 0) + 1
    }
    for str k, int v in counts {
        print("{k}: {v}")
    }
    print(counts)
    print(counts["be"])
    print(counts["xx"])
    print(counts.has("or"))
    print(counts.remove("or"))
    print(counts.keys())
    print(counts.len())
}
```

Ausgabe: `to: 2`, `be: 2`, `or: 1`, `not: 1`, `{"to": 2, "be": 2, "or": 1, "not": 1}`, `some(2)`, `none`, `true`, `true`, `["to", "be", "not"]`, `3`.

```dif error R8
void main() {
    map[str]int counts = {}
    counts["a"] += 1
    print(counts)
}
```

```
main.dif:3:17: error[R8]: cannot apply `+=` to ?int and int
  hint: counts.add("a", 1)
```

```dif error R30
struct P {
    int x
}

void main() {
    map[str]P m = {"a": P{x: 1}}
    m["a"].x = 2
    print(m)
}
```

```
main.dif:7:5: error[R30]: a map entry is changed by assigning it
  hint: read it, change the copy, and store it: m[k] = v
```

```dif error R30
void main() {
    map[str]int m = {"a": 1, "b": 2, "a": 3}
    print(m)
}
```

```
main.dif:2:38: error[R30]: duplicate key "a"
  hint: first used at main.dif:2:22
```

```dif error R11
void main() {
    map[str]int counts = {}
    for str k in counts {
        print(k)
    }
}
```

```
main.dif:3:18: error[R11]: a loop over a map needs key and value
  hint: for str k, int v in counts
```

### R31 — Erlaubte Umwandlungen

**Regel.** Ein Wert vom Typ `T` passt dort, wo `?T` oder `T!E` erwartet wird; `ok` gibt es nur in Mustern, `some(x)` ist ein Ausdruck (R12). Ein `str` passt dort, wo C `c.str` erwartet, eine benannte Funktion an einen C-Funktionszeiger. Sonst wird nie umgewandelt (R8).

**Grammatik:** keine eigene. `T(x)` ist ein gewöhnlicher Aufruf (`postfix`, §2), dessen Name ein Typ ist: `int`, `i8 … u64`, `f64`, `rune`, `str`.

Teil 2c legt fest:

- **Von selbst** passt nur, was die Regel nennt: ein `T` an der Stelle eines `?T` oder `T!E` (R12), später `str` an `c.str` und eine benannte Funktion an einen C-Funktionszeiger (R17). Alles andere, auch `u8` → `int`, `int` → `f64`, `rune` → `int`, `[]T` → `[N]T`, ist `R8` an der Stelle, mit dem Hinweis auf `T(x)`, soweit es erlaubt ist.
- **`T(x)`** wandelt ausdrücklich um, in genau diesen Fällen, und immer **exakt oder mit Abbruch**:

  | Ziel | Argument | Ergebnis |
  |---|---|---|
  | Ganzzahltyp | Ganzzahltyp | derselbe Wert; passt er nicht, `300 does not fit in u8` |
  | Ganzzahltyp | `f64` | der Wert, wenn er ganz ist und passt; sonst `2.5 is not an exact int` (auch bei `NaN`, `inf`) bzw. `1e30 does not fit in int` |
  | Ganzzahltyp | `rune` | der Codepoint; passt er nicht, `128512 does not fit in u8` |
  | `f64` | Ganzzahltyp | der Wert, wenn er als `f64` genau darstellbar ist (alle bis 2^53); sonst `9007199254740993 is not an exact f64` |
  | `f64` | `f64` | derselbe Wert |
  | `rune` | Ganzzahltyp | das Zeichen mit diesem Codepoint; außerhalb `0 … 10FFFF` oder ein Surrogat: `55296 is not a valid rune` |
  | `str` | `rune` | ein Text aus diesem einen Zeichen |

  Bei `T(x)` mit dem Typ von `x` selbst ist das Ergebnis `x`. Gerundet wird nie von selbst: wer von `f64` zu einem Ganzzahltyp will, sagt `x.round()`, `x.trunc()`, `x.floor()` oder `x.ceil()` zuerst (`int(x.round())`).
- **Nicht erlaubt** ist jede andere Umwandlung, `R31` am Namen des Typs, Meldung `` cannot convert str to int ``, mit Hinweis: von `str` nach Zahl `use "12".parse_int()` bzw. `parse_f64()` (mit dem Argument aus dem Quelltext), von einem anderen Typ nach `str` `build text with interpolation: "{x}"`, von `bool` oder nach `bool` `compare explicitly: n != 0` bzw. `use if b { 1 } else { 0 }`, sonst keiner. Auch `Point(x)` mit einem Typ, der keine Umwandlung ist, ist `R31`.
- **Zur Übersetzungszeit.** Ein Argument, das nur aus Literalen und `const` besteht, wird beim Kompilieren umgewandelt; scheitert das, ist es `error[R14]` mit der Meldung des Abbruchs und dem Hinweis `round explicitly first with round(), trunc(), floor() or ceil()` (bei `f64` → Ganzzahltyp, sonst keiner). Das Literal hat dabei keine Stelle (`int(2.5)`: `f64`; `u8(300)`: `int`).
- **Meldung des Abbruchs.** Sie nennt den Zieltyp von `T(x)` und den Wert in seiner Textform (R29): `<wert> does not fit in <T>`, wenn der Wert ganz ist, aber nicht in `T` passt (`300 does not fit in u8`, `-1 does not fit in u64`, `1e30 does not fit in int`), `<wert> is not an exact <T>`, wenn er nicht ganz ist (`2.5 is not an exact int`, `NaN is not an exact i8`) oder bei `f64(n)` nicht genau darstellbar (`9007199254740993 is not an exact f64`), `<n> is not a valid rune` bei `rune(n)`. Für `int` steht `int`, nicht `i64`.
- **Prüfen statt abbrechen.** Jeder Ganzzahltyp hat `to_i8() … to_u64()` und `to_int()`, die ein `?T` liefern: `none`, wo `T(x)` abbräche (§4.3).
- **Position.** Der Abbruch und `R31` stehen am Namen des Typs vor der `(`.

```dif snippet
int n = 300
print(n.to_u8())
print(n.to_u16())
print(f64(n) / 8)
print(i8(-128))
rune r = 'A'
print(rune(int(r) + 1))
print(str(r))
print(int('é'))
```

Ausgabe: `none`, `some(300)`, `37.5`, `-128`, `B`, `A`, `233`.

```dif panic "2.5 is not an exact int"
f64 x = 2.5
print(int(x))
```

```
panic: 2.5 is not an exact int (main.dif:3:11)
```

```dif panic "300 does not fit in u8"
int n = 300
print(u8(n))
```

```
panic: 300 does not fit in u8 (main.dif:3:11)
```

```dif panic "2.5 is not an exact u8"
f64 x = 2.5
print(u8(x))
```

```
panic: 2.5 is not an exact u8 (main.dif:3:11)
```

```dif error R14
void main() {
    print(int(2.5))
}
```

```
main.dif:2:11: error[R14]: 2.5 is not an exact int
  hint: round explicitly first with round(), trunc(), floor() or ceil()
```

```dif error R31
void main() {
    int n = int("12")
    print(n)
}
```

```
main.dif:2:13: error[R31]: cannot convert str to int
  hint: use "12".parse_int()
```

```dif error R8
void main() {
    int n = 3
    f64 x = n
    print(x)
}
```

```
main.dif:3:13: error[R8]: expected f64, found int
  hint: f64(n)
```

---

## 4. Eingebaute Funktionen und Methoden (Teil 1, 2c und 2d)

Signaturen stehen wie Deklarationen (R7): `R T.name(params)`; `T` ist der Elementtyp einer Liste, eines Arrays oder die Typen einer Map. „ändert“ in der letzten Spalte heißt: die Methode ändert `self`, braucht also einen Ort vor dem Punkt (R7) und hat eine Wirkung (R13). Alle anderen Methoden lesen nur. Eigene Methoden an diesen Typen gibt es nicht (R7). Abbrüche (R14) nennen ihre Meldung in der Tabelle von R14, Teil 2c.

### 4.1 Funktionen

| Funktion | Bedeutung |
|---|---|
| `void print(text T x)` | schreibt `"{x}"` und einen Zeilenumbruch auf stdout |
| `range(int n)`, `range(int a, int b)` | Ganzzahlen `0 … n-1` bzw. `a … b-1`; nur als Quelle einer `for`-Schleife |
| `void expect(bool cond)` | prüft in einem `test`-Block `cond` (R18) |
| `void assert(bool cond)` | bricht ab, wenn `cond` falsch ist: `assertion failed: <Bedingung>` (R14); `assert` prüft im Programm, `expect` im `test`-Block (R18) |
| `[]T repeat(T x, int n)` | eine Liste aus `n` Kopien von `x`; `n` darf 0 sein, negativ bricht ab; `T` ist der Elementtyp der Stelle (R8), sonst der von `x` |
| `T(x)` | Umwandlung in `T`, exakt oder Abbruch (R31) |

`print` ist ein Aufruf mit Ein-/Ausgabe und erfüllt damit R13, ebenso `assert`. `expect` gilt nur in `test`-Blöcken und ist dort eine Wirkung. stdout wird beim Programmende, vor jedem Lesen von stdin (§4.6), vor jedem `io.eprint` und vor der Meldung eines Abbruchs geschrieben, damit Eingabeaufforderungen und die Reihenfolge der Ausgabe stimmen.

### 4.2 `str` und `rune`

Ein `str` ist unveränderlich (R5, Teil 2c). Längen, Positionen und `slice` zählen Bytes (UTF-8); ein `str` hat keinen Index. `lower` und `upper` bilden Zeichen für Zeichen nach der einfachen Fallzuordnung von Unicode (`UnicodeData`) ab, ohne Mehrzeichen-Abbildungen: `"Äpfel".lower()` ist `"äpfel"`, `"straße".upper()` ist `"STRAßE"`. `is_letter` gilt für Zeichen der Unicode-Kategorien `L*`, `is_digit` für `Nd`, `is_space` und `trim` für die Unicode-Eigenschaft `White_Space` (Leerzeichen, Tabulator, Zeilenumbrüche, U+00A0 …). Länge, Position und `slice` zählen weiter Bytes.

| Signatur | Verhalten | Wirkung |
|---|---|---|
| `int str.len()` | Länge in Bytes | |
| `bool str.is_empty()` | `len() == 0` | |
| `[]rune str.runes()` | die Codepoints in Reihenfolge | |
| `[]str str.split(str sep)` | teilt an jedem `sep`, von links, ohne Überlappung; aufeinanderfolgende `sep` geben leere Texte; `"".split(",")` ist `[""]`; leeres `sep` bricht ab | |
| `[]str str.lines()` | teilt an `\n`; ein `\r` direkt vor dem Zeilenende entfällt in jeder Zeile, auch in der letzten ohne `\n`; ein letzter Zeilenumbruch erzeugt keine leere Zeile; `""` gibt `[]` | |
| `str str.trim()`, `trim_start()`, `trim_end()` | entfernt Leerraum an beiden Enden, am Anfang, am Ende | |
| `str str.lower()`, `upper()` | jedes Zeichen einzeln in Klein- bzw. Großbuchstaben (einfache Fallzuordnung); der Rest bleibt | |
| `bool str.contains(str s)`, `starts_with(str s)`, `ends_with(str s)` | Teiltext, Anfang, Ende; der leere Text ist immer enthalten | |
| `?int str.find(str s)` | Byte-Position des ersten Vorkommens, sonst `none`; `find("")` ist `some(0)` | |
| `str str.replace(str from, str to)` | ersetzt jedes Vorkommen von `from` von links ohne Überlappung; leeres `from` bricht ab | |
| `str str.repeat(int n)` | `n` Kopien hintereinander; `n < 0` bricht ab | |
| `str str.slice(int a, int b)` | die Bytes `a … b-1`; `0 ≤ a ≤ b ≤ len()`, und `a` und `b` liegen auf Zeichengrenzen, sonst Abbruch | |
| `?int str.parse_int()` | genau `[+-]?[0-9]+`: optionales Vorzeichen, mindestens eine ASCII-Ziffer, führende Nullen erlaubt (`"007"` ist `some(7)`, Daten haben sie oft), kein Leerraum, kein `_`; passt der Wert nicht in `int`, `none`; alles andere `none` | |
| `?f64 str.parse_f64()` | genau `[+-]?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?` (wie das Literal, aber mit Vorzeichen, mit führenden Nullen, ohne `_`; `".5"` und `"5."` sind `none`) oder `[+-]?inf` oder `NaN` (die Wörter der Textform, R29); der Wert wird auf den nächsten `f64` gerundet; wäre er `inf` oder würde eine Ziffernfolge ungleich 0 zu 0, `none` (wie beim Literal, R8); alles andere `none` | |
| `bool rune.is_digit()` | Unicode-Kategorie `Nd` (`'٣'` ja, `'½'` nein) | |
| `bool rune.is_letter()` | Unicode-Kategorien `L*` (`'ä'` ja) | |
| `bool rune.is_space()` | `White_Space` (Unicode) | |
| `str(r)` | der Text aus dem einen Zeichen `r` (R31) | |

```dif snippet
str s = "  Hallo, Welt  "
print(s.trim().upper())
print(s.len())
print("äö".len())
print("äö".runes().len())
print("a,b,,c".split(","))
print("a\nb\n".lines())
print("Hallo".slice(1, 3))
print("Hallo".find("ll"))
print("x".find("y"))
print("42".parse_int() ?? 0)
print("4x".parse_int())
print("+007".parse_int())
print("-1.5e2".parse_f64())
print("ab".repeat(3))
print("a-b-c".replace("-", "+"))
print("Äpfel".lower())
print("straße".upper())
print('7'.is_digit())
print('x'.is_letter())
print(' '.is_space())
```

Ausgabe: `HALLO, WELT`, `15`, `4`, `2`, `["a", "b", "", "c"]`, `["a", "b"]`, `al`, `some(2)`, `none`, `42`, `none`, `some(7)`, `some(-150)`, `ababab`, `a+b+c`, `äpfel`, `STRAßE`, `true`, `true`, `true`.

```dif panic "slice 1..2 is not on character boundaries"
str s = "äö"
print(s.slice(1, 2))
```

```
panic: slice 1..2 is not on character boundaries (main.dif:3:13)
```

### 4.3 Zahlen

Für jeden Ganzzahltyp `I` (`int`, `i8 … u64`):

| Signatur | Verhalten | Wirkung |
|---|---|---|
| `I I.abs()` | Betrag; nur für `int`, `i8`, `i16`, `i32`; der kleinste Wert bricht ab | |
| `I I.min(I b)`, `I I.max(I b)` | der kleinere bzw. größere von `self` und `b` | |
| `?T I.to_T()` | `T` ist `int`, `i8`, `i16`, `i32`, `u8`, `u16`, `u32` oder `u64` (`to_int()`, `to_u8()`, …): `some(T(self))`, wenn der Wert hineinpasst, sonst `none` | |

Für `f64`:

| Signatur | Verhalten | Wirkung |
|---|---|---|
| `f64 f64.round()` | auf die nächste ganze Zahl, bei genauer Mitte von 0 weg (`2.5` → `3`, `-2.5` → `-3`) | |
| `f64 f64.trunc()`, `floor()`, `ceil()` | zu 0 hin, nach unten, nach oben gerundet | |
| `f64 f64.sqrt()` | Quadratwurzel; negativ gibt `NaN` | |
| `f64 f64.abs()` | Betrag (löscht das Vorzeichenbit, bricht nie ab) | |
| `f64 f64.min(f64 b)`, `f64 f64.max(f64 b)` | der kleinere bzw. größere von `self` und `b`; ist ein Operand `NaN`, ist das Ergebnis der andere (wie C `fmin`, `fmax`) | |

Die Ergebnisse sind wieder `f64`; wer eine Ganzzahl will, schreibt `int(x.round())` (R31). `NaN` und `inf` bleiben bei `round` & Co. unverändert. Umwandlungen, Formatangaben und Textform: R31, R29, R28. Eine Methode auf einem Zahl-Literal steht in Klammern: `(5).abs()`.

### 4.4 Listen und Arrays

Für eine Liste `[]T`, in der `T` den Elementtyp meint. Ein Array `[N]T` hat alle Methoden, die die Länge nicht ändern (also `len`, `is_empty`, `first`, `last`, `contains`, `index_of`, `count`, `sort`, `reverse`, `swap`, `slice`, `join`, `filter`, `map`, `sort_by`, `any`, `all`); `push`, `pop`, `insert` und `remove` an einem Array sind `R4`, Meldung `` `[3]int` has no method `push` ``, Hinweis `use a list: []int`. `filter`, `map`, `sort_by`, `any` und `all` stehen mit den Funktionsliteralen in §4.7.

| Signatur | Verhalten | Wirkung |
|---|---|---|
| `int [].len()` | Anzahl der Elemente | |
| `bool [].is_empty()` | `len() == 0` | |
| `void []T.push(T x)` | hängt `x` an; amortisiert O(1) | ändert |
| `?T []T.pop()` | entfernt das letzte Element und liefert es; `none` bei einer leeren Liste | ändert |
| `void []T.insert(int i, T x)` | fügt `x` vor Position `i` ein; `0 ≤ i ≤ len()`, sonst Abbruch | ändert |
| `T []T.remove(int i)` | entfernt das Element an `i` und liefert es; `i` außerhalb `0 … len()-1` bricht ab | ändert |
| `?T [].first()`, `last()` | erstes bzw. letztes Element, `none` bei leerem | |
| `bool [].contains(T x)` | `x` kommt vor (`==`) | |
| `?int [].index_of(T x)` | Position des ersten Elements, das `x` gleicht, sonst `none` | |
| `int [].count(T x)` | Anzahl der Elemente, die `x` gleichen | |
| `void [].sort()` | sortiert aufsteigend; stabil; `T` ist `ordered` (R28); `NaN` steht hinter allen Zahlen | ändert |
| `void [].reverse()` | kehrt die Reihenfolge um | ändert |
| `void [].swap(int i, int j)` | vertauscht die Elemente an `i` und `j`; beide Indizes werden geprüft, einer außerhalb `0 … len()-1` bricht ab (R14, Meldung `index <i> out of bounds for length <n>`, Position am Namen `swap`; ist `i` falsch, nennt sie `i`); bei `i == j` bleibt die Liste unverändert | ändert |
| `[]T [].slice(int a, int b)` | eine neue Liste aus den Elementen `a … b-1`; `0 ≤ a ≤ b ≤ len()` | |
| `str []str.join(str sep)` | die Elemente mit `sep` dazwischen; nur für `[]str` | |

`repeat(x, n)` (§4.1) baut Listen. `==` vergleicht Länge und Elemente (R28). Die leere Liste hat kein erstes Element: `pop`, `first` und `last` geben `none`. Ein `slice` ist eine Kopie, keine Sicht (R5). `swap` ändert die Liste wie jede andere ändernde Methode (R6, R7), auch bei `i == j`; sie ersetzt `swap(mut xs[i], mut xs[j])` (R6). `pop` und `remove` ändern die Liste wie jede andere Methode (R6): wird die Liste danach nie gelesen, ist das eine verlorene Änderung mit dem Hinweis `use xs.last() if the list is not needed afterwards` bzw. `use xs[i] if the list is not needed afterwards`.

```dif snippet
[]int xs = [5, 3, 8]
xs.push(1)
xs.insert(0, 9)
print(xs)
xs.sort()
print(xs)
print(xs.pop())
print(xs.first())
print(xs.index_of(8))
print(xs.contains(7))
print(xs.slice(1, 3))
xs.reverse()
print(xs)
int removed = xs.remove(0)
print("{removed} {xs.len()}")
print(xs.count(3))
[]int empty = []
print(empty.pop())
print(empty)
print(["b", "a"].join("-"))
print(repeat(0, 3))
[]str names = ["a", "b", "c"]
names.swap(0, 2)
names.swap(1, 1)
print(names)
```

Ausgabe: `[9, 5, 3, 8, 1]`, `[1, 3, 5, 8, 9]`, `some(9)`, `some(1)`, `some(3)`, `false`, `[3, 5]`, `[8, 5, 3, 1]`, `8 3`, `1`, `none`, `[]`, `b-a`, `[0, 0, 0]`, `["c", "b", "a"]`.

```dif panic "index 3 out of bounds for length 3"
[]int xs = [1, 2, 3]
xs.swap(0, 3)
print(xs)
```

```
panic: index 3 out of bounds for length 3 (main.dif:3:8)
```

```dif error R28
struct P {
    int x
}

void main() {
    []P ps = [P{x: 2}, P{x: 1}]
    ps.sort()
    print(ps)
}
```

```
main.dif:7:8: error[R28]: `sort` needs an ordered element type, found P
  hint: only numbers, rune and str can be sorted
```

```dif error R7
[]int make() {
    return [1, 2]
}

void main() {
    make().push(3)
}
```

```
main.dif:6:5: error[R7]: `[]int.push` changes self and needs a variable
  hint: store the value in a variable first
```

### 4.5 `map`

Für `map[K]V`; Literal, `m[k]` und `m[k] = v` stehen in R30.

| Signatur | Verhalten | Wirkung |
|---|---|---|
| `int map.len()` | Anzahl der Einträge | |
| `bool map.has(K k)` | `k` kommt vor | |
| `bool map[K]V.remove(K k)` | entfernt den Eintrag; `true`, wenn er da war | ändert |
| `void map[K]V.add(K k, V x)` | nur für Zahlen-`V` (alle Ganzzahlbreiten und `f64`): addiert `x` zum Wert von `k`; fehlt `k`, beginnt die Summe bei 0 und der Eintrag wird hinten angefügt; ein Überlauf bricht ab wie `+` (R29, `integer overflow in +`, Position am Namen `add`); ein anderes `V` ist `R8` | ändert |
| `[]K map.keys()` | die Schlüssel in Einfügereihenfolge | |
| `[]V map.values()` | die Werte in derselben Reihenfolge | |

`keys` und `values` sind Kopien (R5). Eine leere Map hat Länge 0 und Textform `{}`. `add` ersetzt `m[k] += x`, das `R8` bleibt (R30): Hinweis `m.add(k, x)`. Bei einem `V`, das keine Zahl ist, ist `add` `R8` am Namen, Meldung `` `add` needs number values, found str ``, Hinweis `use m[k] = x`.

```dif snippet
map[str]int counts = {}
counts.add("a", 2)
counts.add("b", -1)
counts.add("a", 3)
print(counts)
map[str]f64 total = {}
total.add("x", 0.5)
total.add("x", 0.25)
print(total)
```

Ausgabe: `{"a": 5, "b": -1}`, `{"x": 0.75}`.

```dif panic "integer overflow in +"
map[str]u8 m = {}
m.add("a", 200)
m.add("a", 100)
print(m)
```

```
panic: integer overflow in + (main.dif:4:7)
```

```dif error R8
void main() {
    map[str]str m = {}
    m.add("a", "b")
    print(m)
}
```

```
main.dif:3:5: error[R8]: `add` needs number values, found str
  hint: use m[k] = x
```

### 4.6 `io` und `os`

Beide Module sind Teil der Standardbibliothek und werden mit `use std.io` bzw. `use std.os` geladen (R16). Ein `str` ist immer gültiges UTF-8: enthält gelesener Text ungültige Bytes, ist das ein Fehler (`IoErr.InvalidUtf8`), nie ein stiller Ersatz.

`std.io` hat einen Fehlertyp, der jeder Funktion mit `!` gehört:

```text
enum IoErr {
    NotFound(str path)
    NoPermission(str path)
    InvalidUtf8(str path)
    Other(str path, str reason)
}
```

`reason` ist der Text des Betriebssystems; er ist nicht festgelegt. `path` ist bei stdin `"<stdin>"`. Von außen heißt der Typ `io.IoErr`.

| Signatur | Verhalten | Wirkung |
|---|---|---|
| `io.lines()` | Quelle einer `for`-Schleife (R11) mit Elementen vom Typ `str!io.IoErr`: die Zeilen von stdin ohne `\n` (ein `\r` direkt vor dem Zeilenende entfällt in jeder Zeile, auch in der letzten ohne `\n`), bis zum Ende der Eingabe; eine letzte Zeile ohne Zeilenumbruch zählt; leere Eingabe gibt keine Durchläufe; eine Zeile mit ungültigem UTF-8 ist `err(IoErr.InvalidUtf8("<stdin>"))` und das letzte Element, danach endet die Schleife. Die Schleifenform steht in R11 („Folgen von Ergebnissen“): `for str line in try io.lines()` entpackt jedes Element, die Funktion muss `U!io.IoErr` liefern (auch `void!io.IoErr main()`); ohne `try` ist die Variable ein `str!io.IoErr` | |
| `?str!io.IoErr io.read_line()` | die nächste Zeile von stdin wie bei `lines()`; `ok(none)` am Ende der Eingabe, `err(IoErr.InvalidUtf8("<stdin>"))` bei ungültigem UTF-8 | ändert die Eingabe |
| `str!io.IoErr io.read_file(str path)` | der ganze Inhalt der Datei | |
| `void!io.IoErr io.write_file(str path, str text)` | legt die Datei an oder überschreibt sie mit den Bytes von `text` | ändert die Datei |
| `[]str!io.IoErr io.read_lines(str path)` | die Zeilen der Datei nach der Regel von `lines()`: ohne `\n`, ein `\r` direkt vor dem Zeilenende entfällt (auch in der letzten Zeile), eine letzte Zeile ohne `\n` zählt, eine leere Datei gibt `[]`; Fehler wie `read_file` (ungültiges UTF-8 ist `IoErr.InvalidUtf8(path)`) | |
| `void!io.IoErr io.write_lines(str path, []str lines)` | schreibt jede Zeile gefolgt von `\n` (auch die letzte; eine leere Liste gibt eine leere Datei); sonst wie `write_file`; ein `\n` im Text einer Zeile bleibt, die Datei hat dann mehr Zeilen | ändert die Datei |
| `void io.eprint(text T x)` | schreibt `"{x}"` und einen Zeilenumbruch auf stderr | ändert stderr |
| `[]str os.args()` | die Argumente des Programms ohne den Programmnamen | |
| `void os.exit(int code)` | schreibt stdout und stderr, beendet das Programm mit Exit-Code `code` (0 … 255, sonst Abbruch); `defer` läuft nicht (R23) | beendet |
| `?str os.env(str name)` | der Wert der Umgebungsvariablen; `none`, wenn sie nicht gesetzt ist, `some("")`, wenn sie leer ist | |

**Eingabe ist ein Strom.** stdin gehört dem Programm genau einmal; `io.lines()` und `io.read_line()` teilen eine Leseposition und lesen Zeile für Zeile erst, wenn sie gebraucht werden (nicht vorab). Die Schleife über `io.lines()` liest bei jedem Durchlauf eine Zeile (sie ist vom Schnappschuss in R11 ausgenommen); nach einem `break` liest `io.read_line()` oder eine zweite `io.lines()`-Schleife die nächste ungelesene Zeile. Eine Zeile mit ungültigem UTF-8 wird dabei verbraucht. stdout wird vor jedem Lesen geschrieben (§4.1).

**Argumente und Umgebung.** Ein Argument oder ein Wert von `os.env`, der kein gültiges UTF-8 ist, bricht ab: `invalid UTF-8 in argument <i>` (Index ab 0 in `os.args()`) bzw. `invalid UTF-8 in environment variable <name>`. Der Abbruch geschieht im Aufruf von `os.args()` bzw. `os.env(name)` (Position und Meldung in der Tabelle von R14), nicht erst beim Benutzen des Wertes.

`io.read_line`, `io.write_file`, `io.write_lines`, `io.eprint` und `os.exit` haben eine Wirkung (R13); `io.read_file`, `io.read_lines` und `os.args` nicht: ihr Ergebnis wird verwendet. Der Fehler `io.IoErr.NotFound(path)` entsteht, wenn die Datei fehlt, `NoPermission(path)`, wenn das Betriebssystem den Zugriff verweigert, `InvalidUtf8(path)`, wenn der Inhalt kein gültiges UTF-8 ist, sonst `Other(path, reason)`.

```dif
use std.io
use std.os

str describe(io.IoErr e) {
    switch e {
    case io.IoErr.NotFound(str path):
        return "missing: {path}"
    case io.IoErr.NoPermission(str path):
        return "no permission: {path}"
    case io.IoErr.InvalidUtf8(str path):
        return "not UTF-8: {path}"
    case io.IoErr.Other(str path, str reason):
        return "{path}: {reason}"
    }
}

void main() {
    switch io.read_file("does-not-exist.txt") {
    case ok(str text):
        print(text.len())
    case err(io.IoErr e):
        print(describe(e))
    }
    print(os.args().len())
    print(os.env("NO_SUCH_VARIABLE_XYZ") ?? "unset")
    if os.args().len() > 5 {
        os.exit(3)
    }
    print("done")
}
```

Ausgabe: `missing: does-not-exist.txt`, `0`, `unset`, `done`.

`write_lines` und `read_lines` sind das Paar zu `lines()`. Das Beispiel schreibt auf `/dev/stdout` (nur Linux und macOS) statt in eine Datei, damit es nichts hinterlässt, und liest eine fehlende Datei:

```dif
use std.io

void!io.IoErr main() {
    try io.write_lines("/dev/stdout", ["one", "", "three"])
    switch io.read_lines("no-such-dir/lines.txt") {
    case ok([]str ls):
        print(ls.len())
    case err(io.IoErr e):
        print(e)
    }
}
```

Ausgabe: `one`, ``, `three`, `IoErr.NotFound("no-such-dir/lines.txt")`.

Eine Zeile mit `read_line` lesen (ohne Eingabe ist das Ergebnis `ok(none)` und `try` gibt `none`, `??` setzt den Vorgabewert ein):

```dif
use std.io

void!io.IoErr main() {
    str name = try io.read_line() ?? "world"
    print("hello {name}")
}
```

Ausgabe: `hello world`.

Ein Programm, das alle Zahlen seiner Eingabe addiert (hier mit leerer Eingabe):

```dif
use std.io

void!io.IoErr main() {
    int total = 0
    for str line in try io.lines() {
        total += line.trim().parse_int() ?? 0
    }
    print(total)
}
```

Ausgabe: `0`.

```dif error R16
void main() {
    for str line in io.lines() {
        print(line)
    }
}
```

```
main.dif:2:21: error[R16]: `io` is not imported
  hint: add use std.io at the top of the file
```

### 4.7 Listen mit Funktionsargument (Teil 2d)

Für eine Liste oder ein Array `[]T`. Der Funktionstyp des Arguments steht in der Signatur (R26); an der Stelle steht ein Literal: `xs.filter(bool(int x) { return x > 1 })`. Außer `sort_by` ändert keine dieser Methoden `self`; die Wirkung eines Aufrufs kommt vom Literal (R13, R26). Ein Literal liest Kopien der Namen ringsum und ändert nie etwas dort (R26, R6 Teil 2d); wer ringsum etwas ändern will, schreibt eine `for`-Schleife. Ein Array hat alle fünf (`sort_by` ändert die Länge nicht); `filter` und `map` liefern eine Liste.

| Signatur | Verhalten | Wirkung |
|---|---|---|
| `[]T [].filter(bool(T) f)` | eine neue Liste aus den Elementen, für die `f` `true` liefert, in der alten Reihenfolge; `f` läuft einmal je Element, von vorn nach hinten | |
| `[]R []T.map(R(T) f)` | eine neue Liste gleicher Länge aus den Ergebnissen von `f`; `f` läuft einmal je Element, von vorn nach hinten | |
| `void []T.sort_by(bool(T, T) less)` | sortiert nach `less(a, b)` („`a` steht echt vor `b`“); stabil: Elemente, bei denen weder `less(a, b)` noch `less(b, a)` gilt, behalten ihre Reihenfolge; Verfahren und Reihenfolge der Aufrufe sind festgelegt (unten) | ändert |
| `bool [].any(bool(T) f)` | `true`, wenn `f` für ein Element `true` liefert; hört beim ersten Treffer auf; leere Liste: `false` | |
| `bool [].all(bool(T) f)` | `true`, wenn `f` für jedes Element `true` liefert; hört beim ersten `false` auf; leere Liste: `true` | |

- **`map` ohne eigene Generics.** `R` ist der Rückgabetyp des Literals; er ist jeder Typ außer `void`, auch `?U` und `U!E` (Ergebnis: `[](int!E)`, R12). Das Ergebnis hat den Typ `[]R`; verlangt die Stelle einen anderen, ist es `R8` am Namen `map`, Meldung `` expected []int, found []str ``. Ist `R` `void`, ist es `R8` am Literal, Meldung `` `map` needs a literal that returns a value, found void(int) ``, Hinweis `use a for loop to do something for each element`. Der Ergebnistyp aus dem Literal ist eine Eigenschaft dieser eingebauten Methode (wie bei `repeat`, §4.1); für eigene Funktionen gibt es ihn erst mit Generics (R27, M3).
- **Elemente.** Die Elemente gehen als Kopien in `f` (R5). Ein Literal, das die Liste liest, über die der Aufruf läuft, liest ihre Kopie von vor dem Aufruf (R26).
- **`sort_by`.** Sinnvoll ist eine strenge Ordnung (`<`, nie `<=`). Das Verfahren ist festgelegt, damit auch ein Literal mit Wirkung (R13) ein reproduzierbares Programm ergibt: Mergesort von oben nach unten. Eine Liste mit höchstens einem Element ruft `less` nie auf. Sonst wird sie bei `mid = len / 2` (abgerundet) geteilt, erst die linke Hälfte `[0, mid)`, dann die rechte `[mid, len)` sortiert und zusammengeführt: solange beide Hälften Elemente haben, ruft das Mischen `less(r, l)` mit den vordersten Elementen `r` der rechten und `l` der linken Hälfte auf und nimmt `r`, wenn es `true` liefert, sonst `l`; ist eine Hälfte leer, folgt der Rest der anderen. Das ergibt Stabilität, und auch eine inkonsistente `less` (etwa bei `NaN`) liefert bei jedem Lauf dieselbe Umordnung derselben Elemente; nichts bricht ab. `sort_by` ändert `self` und braucht einen Ort vor dem Punkt (R7); das Literal sieht die Liste, wie sie vor dem Sortieren war (R26). `T` muss nicht `ordered` sein: die Ordnung kommt vom Literal.
- **Randfälle.** Eine leere Liste ruft `f` nie auf: `filter` und `map` liefern `[]`, `any` `false`, `all` `true`, `sort_by` ändert nichts. Ein Abbruch (R14) im Literal beendet das Programm wie überall. `try` im Literal gehört zum Literal (R26): `map` mit `int!E` liefert eine Liste von Ergebnissen, die man mit `for T x in try rs` entpackt (R11).

```dif
struct Person {
    str name
    int age
}

void main() {
    []Person people = [
        Person{name: "Ada", age: 36},
        Person{name: "Bo", age: 17},
        Person{name: "Cy", age: 52},
        Person{name: "Di", age: 36},
    ]
    int limit = 18
    []Person adults = people.filter(bool(Person p) { return p.age >= limit })
    print(adults.map(str(Person p) { return p.name }).join(", "))
    people.sort_by(bool(Person a, Person b) { return a.age < b.age })
    print(people.map(str(Person p) { return p.name }).join(" "))
    print(people.any(bool(Person p) { return p.age < 18 }))
    print(people.all(bool(Person p) { return p.age >= 18 }))
}
```

Ausgabe: `Ada, Cy, Di`, `Bo Ada Di Cy`, `true`, `false`.

```dif error R8
void main() {
    []int xs = [1, 2, 3]
    print(xs.map(void(int x) { print(x) }))
}
```

```
main.dif:3:18: error[R8]: `map` needs a literal that returns a value, found void(int)
  hint: use a for loop to do something for each element
```

```dif error R8
void main() {
    []str names = ["a", "b"]
    []int xs = names.map(str(str n) { return n })
}
```

```
main.dif:3:22: error[R8]: expected []int, found []str
```
