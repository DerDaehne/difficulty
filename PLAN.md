# difficulty — Idee & Plan

> Schwierig ist nur der Name: eine kompilierte Sprache, die sich schreibt wie ein angenehmeres C.
> C-Optik, statische Typen, native Binaries und direkter Zugriff auf C-Bibliotheken, aber ohne
> Zeiger, `null`, stille Überläufe und verschluckte Fehler. Die Einfachheit von Python und die
> Klarheit von C: keine Syntax, für die man einen Doktor braucht, kein Boilerplate.
> Die Schwierigkeit steckt im Compiler, nicht im Programm.

Stand: 2026-11-22 · Status: Entwurf, alle Grundsatzentscheidungen getroffen (§11). Verbindliche Sprachbeschreibung: [docs/spec.md](docs/spec.md).

---

## 1. Kurzfassung

| | |
|---|---|
| **Was** | Statisch typisierte, kompilierte Sprache. Optik von C (Typ vorn, `{}`), Einfachheit von Python (`for int i in range(n)`, keine Semikolons, keine Klammern um Bedingungen). |
| **Für wen** | Wer native Programme in der Nähe von C schreiben will, ohne die Fallen von C. Dieselben Eigenschaften (eine Schreibweise, eindeutige Meldungen mit Reparaturvorschlag) helfen Werkzeugen und KI-Agenten: ein Agent soll ein korrektes Programm im ersten oder zweiten Anlauf schreiben, ohne die Sprache aus dem Training zu kennen; das misst §10. |
| **Backend** | C11 → System-C-Compiler (gcc/clang). Schnell wie C, Binaries in KB statt MB. |
| **C-Bibliotheken** | Header direkt importieren (`use c "sqlite3.h"`), `.so`/`.dll` linken oder zur Laufzeit laden, C-Quellen mitbauen. |
| **Speicher** | Wertsemantik + Referenzzählung + Copy-on-Write. Kein GC, keine Lifetimes, kein Borrow-Checker. |
| **Testen** | `test`-Blöcke in der Sprache; `dif test` mit Prozess-Isolation, Coverage, Sanitizern und Doc-Tests. |
| **Compiler** | In Rust, ein einzelnes Binary `dif`, zunächst ohne Crate-Abhängigkeiten. |
| **Dateien** | `.dif` |

```dif
void main() {
    print("Hallo, Welt")
}
```

```
$ dif run hello.dif            # kompilieren + ausführen
$ dif build hello.dif -o hello # natives Binary, Ziel ≤ 16 KB
$ dif test hello.dif           # Tests
```

---

## 2. Ziele

Ein Agent, der difficulty nur aus einer kurzen Referenz kennt, soll ein korrektes Programm im ersten oder zweiten Anlauf schreiben, und das Ergebnis soll schnell und klein sein. Daraus folgen diese Ziele:

| Ziel | Wie difficulty es erreicht |
|---|---|
| Vertraute Syntax | C-Optik, Python-Einfachheit; Abweichungen nur, wo sie einen Fehler verhindern |
| Fehler vor dem Lauf | statische Typen, `dif check` ohne Ausführung, Fehlercode = Regelnummer, Hinweise mit Fix |
| Nichts passiert heimlich | keine impliziten Umwandlungen, kein `null`, EOF und fehlende Schlüssel stehen im Typ, unbenutzte Ergebnisse sind Fehler |
| Sichtbare Änderungen | `mut` in Signatur und Aufruf, Methoden arbeiten auf der Variable vor dem Punkt, keine veränderlichen Globalen |
| Echte Datentypen | `bool`, Ganzzahlen fester Breite, `enum` mit Payload, `xs[i] = v` |
| Ehrliche Tests | `test`-Blöcke in der Sprache, `expect` vergleicht Werte und zeigt Teilwerte |
| Geschwindigkeit | nativer Code über C, Ziel ≤ 1,5× C `-O2`; Copy-on-Write statt Kopien |
| Kleine Programme | Hello World ≤ 16 KB, keine Runtime außer einem kleinen C-Header |
| C als Nachbar | C-Header importieren, `.so`/`.dll` linken oder laden |

---

## 3. Grundsätze und Regeln

**Leitbild:** Lesbar wie C, einfach wie Python. Keine Symbol-Akrobatik (Rust), kein Boilerplate (Java), keine Objekte, aber Funktionen an structs.

### 3.1 Design-Grundsätze

Zehn Sätze, aus denen alle Regeln folgen. Wer sie kennt, kann die meisten Regeln selbst erraten.

1. **Lesbar wie C, einfach wie Python.** Wo C, Python oder Go etwas schon gleich schreiben, schreiben wir es auch so.
2. **Eine Sache, eine Schreibweise.** Keine Synonyme; `dif fmt` bringt jeden Code in dieselbe Form.
3. **Nichts passiert heimlich.** Keine impliziten Umwandlungen, kein `null`, keine stillen Defaults, kein „wahr-ish“.
4. **Was sich ändert, sieht man.** An einer Zuweisung, an `mut` in Signatur und Aufruf oder als Variable vor dem Punkt eines Methodenaufrufs.
5. **Jede Deklaration nennt ihren Typ.** Man muss nie nachrechnen, was eine Variable ist.
6. **Werte statt Referenzen.** Kopieren ist billig, heimliches Teilen ist unmöglich.
7. **Fehler sind Werte, Bugs sind Abbrüche.** Was schiefgehen kann, steht im Typ; was nie passieren darf, stoppt das Programm mit Position.
8. **C ist ein Nachbar.** C-Header importieren, Bibliotheken linken oder laden, ohne Glue-Code.
9. **Tests gehören zum Code.** Sie stehen daneben und laufen mit einem Befehl.
10. **Der Compiler erklärt sich.** Jede Regel hat eine Fehlermeldung, die sie nennt und einen Fix vorschlägt.

### 3.2 Die Regeln

Vollständig und ohne „außer“. Diese Liste ist auch der Kern von `llms.txt`.

| # | Regel | Beispiel |
|---|---|---|
| R1 | Ein Programm besteht nur aus Deklarationen: `const`, `struct`, `enum`, Funktionen, `test`, `use`, `extern`. Es startet in `void main()` oder `void!E main()`. | `void main() { print("hi") }` |
| R2 | Jede Deklaration nennt ihren Typ vorn: Variablen, Parameter, Felder, Schleifen- und `case`-Variablen. | `int n = 0` · `int add(int a, int b)` · `for int i in range(3)` |
| R3 | Anweisungen enden am Zeilenende. Blöcke stehen in `{}`, Bedingungen ohne Klammern. Die Kopfzeile von `if`, `while`, `for` und `switch` endet am ersten `{`. | `if n > 0 { n -= 1 }` · `if hall.is_full() { … }` |
| R4 | Variablen sind veränderlich, `const` nicht. Global gibt es nur `const`. Ein Name gilt bis zum Ende seines Blocks und wird darin nicht neu deklariert. | `const int MAX = 8` |
| R5 | Zuweisung und Rückgabe kopieren den Wert; die Kopie ist unabhängig, intern aber billig. Ein **Griff** ist ein Wert, dessen Kopie dasselbe Objekt bezeichnet (Kanal, Task, `shared`, Datei, C-Objekt und die Handle-Typen der Standardbibliothek wie `db.Conn`, `html.Page`, `xml.Writer`, `ui.Builder`): kopiert wird der Griff, nicht das Ding. Seine Methoden dürfen hinter den Griff schreiben, ohne den Wert zu ändern: kein `mut self`, kein `mut` am Aufruf, kein `R6`. Griffe entstehen nur in der Standardbibliothek und aus C (opake Typen); eigene Typen sind in v0 nie Griffe. Parameter ohne `mut` sind lokale Kopien, wie in C. | `Point b = a` · `b.x = 9` ändert `a` nicht |
| R6 | Soll eine Funktion ein Argument ändern, steht `mut` in der Signatur und am Aufruf; die Änderung landet beim Aufrufer. Eine Änderung, die danach nie gelesen wird, ist ein Fehler (sie träfe nur eine Kopie). `mut` nimmt Orte: Variablen, Felder und Elemente. Ein Ort geht höchstens einmal veränderbar in einen Aufruf: als `self` oder als `mut`-Argument; überlappen zwei Orte, ist das ein Fehler, bei Elementen mit Laufzeit-Index ein Abbruch. Liest derselbe Aufruf einen Ort zusätzlich, bekommt der lesende Teil einen Schnappschuss. | `void shift(mut Point p, int dx)` · `shift(mut scene.player.pos, 5)` · `normalize(mut tasks[i])` · `void nudge(Point p) { p.x += 1 }` → Fehler |
| R7 | Eine Methode ist eine Funktion an einem Typ: `R T.name(params)`. In ihr ist `self` der Ort, an dem sie aufgerufen wird. Ändert sie `self`, steht `mut self` als erster Parameter. Methoden ohne `mut self` gehen auch auf Ausdrücken. | `hall.book(3, 5)` · `(dx * dx + dy * dy).sqrt()` · `xs.push(4)` · `int Hall.free() { return self.seats.count(Seat.Free) }` |
| R8 | Typen mischen sich nie von selbst. Operatoren verlangen gleiche Typen; `T(x)` wandelt exakt um oder bricht ab; absichtlich runden mit `round()`/`trunc()`; Text baut man mit `"{x}"`. Ausdrücke nur aus Literalen rechnen im Typ ihrer Stelle; ohne Stelle ist ein Literal ohne Punkt `int`, mit Punkt `f64`; Float-Literale werden auf den nächsten Wert gerundet. „Steht beim Kompilieren fest“ heißt: nur Literale und `const`. | `f64(n)` · `int(x.round())` · `f64 h = 7 / 2` ist `3.5` · `int k = 7 / 2` ist `3` · `print(7 / 2)` ist `3` |
| R9 | Bedingungen sind `bool`, nichts anderes. | `if xs.len() > 0` statt `if xs` |
| R10 | `switch` deckt jeden möglichen Wert ab: alle Varianten oder `default`. Jeder `case` ist ein eigener Block und endet von selbst. Varianten eines `enum` schreibt man mit ihrem Typ; `T!E` und `?T` sind keine `enum`s, sie haben die eingebauten Formen `ok`, `err`, `some`, `none`. | `case Seat.Taken:` · `case ok(int n):` |
| R11 | Schleifen: `for T x in …` (über `range(n)`, `range(a, b)`, Listen, `io.lines()`, Kanäle), `for int i, T x in xs` (Index und Element), `for K k, V v in m` (Schlüssel und Wert) und `while cond`. `break` und `continue` gehören zur innersten Schleife; `break` steht nie direkt in einem `switch`, weil es dort in C und Go den `switch` verließe. | `for str line in io.lines() { … }` · `case Msg.Quit: return` |
| R12 | Kann eine Funktion scheitern, steht es im Rückgabetyp: `T!E` heißt Wert oder `err(e)`, `?T` heißt Wert oder `none`. Erfolg gibt man einfach zurück. `try x` reicht einen Fehler gleichen Typs weiter; `try x or E.V(…)` übersetzt ihn vorher in einen eigenen. | `int!AgeErr parse_age(str s)` · `try s.parse_int() or AgeErr.NotANumber(s)` |
| R13 | Ein Ergebnis vom Typ `T!E` oder `?T` wird immer verwendet: `try`, `??`, `switch` oder bewusst `_ =`. Jeder andere Aufruf braucht eine Wirkung: sein Ergebnis wird verwendet, oder er ändert etwas (`mut`, `self`), macht Ein-/Ausgabe, ruft C oder startet einen Task. Ein Aufruf ohne Wirkung ist ein Fehler. | `int a = parse_age(s) ?? 0` · `hall.book(3, 5)` allein → Fehler · `add(2, 3)` allein → Fehler |
| R14 | Programmierfehler brechen mit Meldung und Position ab: Index außerhalb, Überlauf, Division durch 0, ungenaue Umwandlung, `assert`. Immer, auch im Release. | `panic: index 40 out of bounds (kino.dif:31:13)` |
| R15 | `if` mit `else` liefert einen Wert, wenn jeder Zweig mit einem Ausdruck endet. | `str s = if ok { "ja" } else { "nein" }` |
| R16 | Eine Datei ist ein Modul. `use name` lädt `name.dif` neben der Datei, `use std.x` die Standardbibliothek, `use c "x.h"` einen C-Header; `as` gibt jedem Modul einen anderen Namen; zyklische `use` sind ein Fehler. `pub` vor Typen, Funktionen, Feldern und Methoden macht sie sichtbar. Funktionen ohne `self` gehören in ein Modul, nicht an einen Typ (`c.to_str(p)`, `chan.new[int](64)`). | `use geo` · `use rekta as ui` · `use c "math.h" link "m"` |
| R17 | Pointer gibt es nur in `unsafe { }` und in C-Deklarationen; dort gelten C-Regeln, ein roher Pointer `*T` darf wie in C NULL sein (`p.is_null()`). Strings und Griffe, die C zurückgibt, sind `?T`; Parameter an C sind nicht optional, außer eine `extern`-Deklaration erlaubt `?`. Pointer auf opake C-Typen sind Griffe; nennt der Import einen Freigeber (`release T with f`), werden sie gezählt und `f` läuft genau einmal. Strings an C gelten nur für die Dauer des Aufrufs, ein NUL-Byte darin bricht ab. Nicht genannte Felder eines C-structs sind 0. | `unsafe { *void p = malloc(64) }` · `?rk.window w = rk.view_window(v)` |
| R18 | Tests stehen neben dem Code: `test "name" { }`, geprüft mit `expect(cond)`. `dif test` führt sie aus, auch die Codebeispiele in `///`-Kommentaren. | `test "add" { expect(add(2, 3) == 5) }` |
| R19 | *(nach v0)* `spawn f(args)` startet `f` parallel und liefert `task[T]`; `t.wait()` holt das Ergebnis. Argumente gehen ohne `mut` hinein. Ein Task lebt nur in der Funktion, die ihn gestartet hat: verlässt sie ihren Rumpf, werden ihre noch laufenden Tasks erst abgebrochen, dann abgewartet. Ein Abbruch im Task kommt beim Warten an. Nach `t.cancel()` kehrt jeder blockierende std-Aufruf im Task mit Fehler zurück. `main` läuft immer im Hauptthread des Betriebssystems, Tasks nie. | `task[int] a = spawn fib(40)` · `a.wait()` |
| R20 | *(nach v0)* Tasks tauschen Werte nur über Kanäle aus: `chan[T]`, `ch.send(v)`, `for T x in ch` (endet nach `ch.close()`). Ein Kanal ist ein Griff: Kopien zeigen auf denselben Kanal. Gesendete Werte gehören danach dem Empfänger. Griffe zählen atomar; ein C-Griff darf nur in einen Task, wenn sein Import `send T` sagt. | `chan[int] ch = chan.new[int](64)` |
| R21 | *(nach v0)* `parallel for T x in xs { … yield v }` verteilt die Durchläufe auf alle Kerne und liefert die `yield`-Werte als Liste in Eingabereihenfolge. Der Rumpf liest Variablen von außen nur. | `[]int sq = parallel for int x in xs { yield x * x }` |
| R22 | *(nach v0)* `share(x)` friert einen Wert ein und liefert `shared[T]`: nur lesbar, und ohne Kopie an beliebig viele Tasks übergebbar. Ändern heißt, eine neue Version zu bauen und erneut zu teilen. | `shared[Layout] snap = share(layout)` |
| R23 | `defer X` führt X am Ende des umgebenden Blocks aus, als stünde es dort, auf jedem Weg hinaus (Ende, `return`, `try`, `break`); Argumente werden erst dann ausgewertet. Mehrere `defer` laufen in umgekehrter Reihenfolge. Ein Ergebnis vom Typ `T!E` braucht `_ =`. Ein Abbruch (R14) beendet das Programm ohne `defer`. | `defer rk.ui_close()` · `defer _ = f.close()` |
| R24 | Erwartet C ein Paar aus Funktion und `void *user`, übergibt man eine dif-Funktion und, wenn sie Zustand braucht, eine Variable; die Funktion bekommt die Variable als ersten Parameter, mit `mut`, wenn sie sie ändert (R6). Keine Funktion: `none`. Der Kontext gilt, solange der Aufruf läuft, in den die Variable mit `mut` hineinging; ein Callback außerhalb dieses Aufrufs oder aus einem anderen Thread bricht mit Meldung ab. | `rk.app_main(rk.app_desc{…, launch: launch, user: mut app})` |
| R25 | Parameter mit Default stehen hinten und werden beim Aufruf beim Namen genannt (`name: wert`), alle anderen der Reihe nach. | `ui.tile("0.3", label: "notes")` |
| R26 | Ein Funktionswert hat den Typ `R(T1, T2)` und wird geschrieben wie eine Funktion ohne Namen: `R(T1 a, T2 b) { … }`. Ein Literal steht nur direkt als Argument eines Aufrufs (nie in einer Variable, einem struct, einer Liste, in `spawn` oder als Typargument) und lebt nur während dieses Aufrufs: es nimmt beim Auswerten eine Momentaufnahme jedes Namens ringsum, den es benutzt, und ändert nie etwas ringsum (eine Änderung daran ist R6). Ein Literal ohne Parameter darf als Block `{ … }` direkt hinter der schließenden Klammer stehen; `return` und `try` darin beenden den Block. Benannte Funktionen und Varianten-Konstruktoren haben keine Umgebung und sind gewöhnliche Werte, die man speichern darf. Ändert ein Funktionswert einen Parameter, steht `mut` in seinem Typ. | `people.filter(bool(Person p) { return p.age >= 18 })` · `db.transaction() { … }` · `on_change: Msg.Draft` · `void(mut A, M) update` |
| R27 | Typparameter stehen in eckigen Klammern hinter dem Namen und werden beim Aufruf meist abgeleitet; jede Instanz wird eigener Code. Eine Einschränkung steht vor dem Typparameter wie ein Typ vor dem Namen: `equal`, `ordered`, `hashable`, `text`. | `[]B convert[A, B]([]A xs, B(A) f)` · `T max[ordered T](T a, T b)` · `struct Pair[A, B]` |
| R28 | Jeder Typ ohne Funktionswert-Felder ist `equal` (`==`) und `text` (Ausgabe, `"{x}"`); `ordered` (`<`) sind Zahlen und `str`; `hashable` ist alles, was `equal` ist. Griffe vergleichen ihre Identität, Maps sind gleich, wenn ihre Einträge gleich sind. | `Point{x: 1, y: 2} == Point{x: 1, y: 2}` · `map[Point]str` |
| R29 | Zahlen: Ganzzahl-`/` rundet Richtung 0, `%` hat das Vorzeichen des Dividenden; Floats folgen IEEE 754 (`inf`, `NaN`, `NaN == NaN` ist false); Ganzzahl-Division durch 0, `INT_MIN / -1` und Shifts ab der Bitbreite brechen ab. Ausdrücke und Argumente werden von links nach rechts ausgewertet. | `-7 / 2` ist `-3` · `-7 % 3` ist `-1` · `1.0 / 0.0` ist `inf` |
| R30 | `map[K]V` schreibt man `{k: v, …}`; `m[k]` liefert `?V`, `m[k] = v` setzt, die Reihenfolge ist die der Einfügung. | `counts[w] = (counts[w] ?? 0) + 1` |
| R31 | Ein Wert vom Typ `T` passt dort, wo `?T` oder `T!E` erwartet wird; `ok` und `some` gibt es nur in Mustern. Ein `str` passt dort, wo C `c.str` erwartet, eine benannte Funktion an einen C-Funktionszeiger. Sonst wird nie umgewandelt (R8). | `return n` in `int!AgeErr` · `expect(parse(s) == tasks)` |

### 3.3 Sonderfälle, die wir aufgelöst haben

| Sonderfall im Entwurf | Auflösung |
|---|---|
| `self` ohne Typ in einer sonst typisierten Parameterliste | `self` wird nicht deklariert, es ist Teil des Methoden-Begriffs (R7) |
| `mut self` abgeleitet statt geschrieben | entfällt (ADR 016): `mut self` steht in der Signatur wie `mut` an einem Parameter (R7, R6) |
| `mut` am Aufruf, aber nicht vor Methoden | `mut` gehört nur zur Übergabe von Argumenten; die Variable vor dem Punkt gehört der Methode (R6, R7) |
| `mut T x` in der Signatur **und** `mut x` am Aufruf | nur noch am Aufruf; der Compiler sieht am Rumpf, welche Parameter sich ändern (R6) |
| vergessenes `mut` → Änderung landet still in einer Kopie, auch wenn die Funktion ein Ergebnis liefert | eine Funktion, die einen Parameter ändert, bekommt ihn mit `mut`; fehlt es, Fehler mit Fix (R6) |
| Aufruf ohne Ergebnis und ohne Änderung | ein Aufruf ohne Wirkung ist ein Fehler (R13) |
| `f64(n)` bricht ab, `int(2.5)` ist ein Compilerfehler | eine Regel: `T(x)` ist exakt oder bricht ab; steht der Wert schon beim Kompilieren fest, meldet es der Compiler (R8, R14) |
| `break` im `switch` (in C verlässt es den `switch`, in Go auch) | `break` gehört nur zu Schleifen und steht nie direkt im `switch` (R11) |
| `default` bei `enum` optional, bei `int`/`str` Pflicht | eine Regel: jeder mögliche Wert ist abgedeckt (R10) |
| `ok`/`err`/`some`/`none` ohne Typ, obwohl Varianten immer einen Typ tragen | `T!E` und `?T` sind keine `enum`s, sondern eingebaute Formen (R10) |
| Kanal, Task, `shared`, C-Objekt verhalten sich nicht wie unabhängige Kopien | sie sind Griffe: kopiert wird der Griff (R5) |
| C-structs mit 50 Feldern, ohne Defaults in der Deklaration | C-Regel: nicht genannte Felder sind 0 (R17) |
| C-Callbacks brauchen App-Zustand, es gibt aber keine veränderlichen Globalen | Kontext-Variable aus `main` (R24) |
| `try` passt nur bei gleichem Fehlertyp | `try x or E.V(…)` übersetzt (R12) |
| `int gcd(int a, int b) { … a = t }` wäre durch die Ableitung ein `mut`-Parameter geworden | `mut` steht in der Signatur, Parameter ohne `mut` sind Kopien wie in C (R5, R6) |
| Änderung an einer Kopie ging still verloren (`for Task t in …`, `nudge(Point p)`) | eine nie gelesene Änderung ist ein Fehler (R6) |
| `hall.book(3, 5)` war nach R13 gültig, im Beispiel ein Fehler | `T!E`- und `?T`-Ergebnisse werden immer verwendet (R13) |
| `if f() {` und `ui.tiles {` waren mehrdeutig | Block nur direkt hinter `)`, Kopfzeile endet am ersten `{` (R3, R26) |
| Literale konnten über Generics oder `spawn` entkommen | Literale stehen nur direkt als Argument (R26) |
| `a.merge(a)` las eine Variable, die derselbe Aufruf ändert | der lesende Teil bekommt einen Schnappschuss (R6) |
| Builder brauchten versteckten globalen Zustand | Builder als Parameter und Griff (R5, §4.10) |
| R24 hielt Zeiger auf `main`-Variablen ohne Grenze | Kontext gilt nur während des Aufrufs mit `mut` (R24) |
| „c.str und Griffe sind sicher“ stimmte nicht | sichere Standards für C (R17) |
| Ereignisschleifen konnten nicht enden | Tasks werden beim Verlassen abgebrochen, `case Msg.Quit: return` (R11, R19) |

Zwei einfache Bilder genügen: Ein Funktionsaufruf `f(x)` liest `x`, `f(mut x)` ändert `x`. Ein Methodenaufruf `x.m()` arbeitet immer auf `x`, wie `this` in C++/Java. Wer in einer Funktion mit einer Arbeitskopie rechnen will, legt sie dort an: `Hall work = hall`.

### 3.4 Wie neue Features hineinkommen

- Ein Feature muss sich als **eine Regel ohne „außer“** schreiben lassen, sonst wird es umgestaltet oder gestrichen.
- Jede Regel hat eine Fehlermeldung, die die Regelnummer nennt (`error[R6]: …`, `dif explain R6`), und Golden-Tests für den gültigen Fall und den Verstoß.
- Schreiben C, Python oder Go es schon gleich, übernehmen wir es. Eine Abweichung braucht einen Fehler, den sie verhindert.
- Ein neues Schlüsselwort kommt nur, wenn keine Bibliotheksfunktion reicht.
- Regelnummern sind stabil, weil sie Fehlercodes sind; neue Regeln hängen hinten an.

Schlüsselwörter (25): `const struct enum if else while for in break continue return switch case default try or mut use pub test extern unsafe void self defer`, nach v0 dazu `spawn parallel yield`. Genaue Einteilung (reservierte Literale, Kontextwörter, vordefinierte Namen): docs/spec.md §1.5
Kontextwörter: `as` in `use`-Zeilen; `c link pkg source impl load strip release with send` in `use c`/`extern`-Zeilen; `panics` im Testkopf
Literale/Konstruktoren: `true false none some ok err` · eingebaute Funktionen: `print range repeat expect assert`

---

## 4. Die Sprache in Beispielen

Alles hier ist Entwurf, verbindlich ist die Spezifikation [docs/spec.md](docs/spec.md).

### 4.1 Grundsyntax

```dif
const int LIMIT = 100                  // einzige Form globaler Werte: Konstanten

int add(int a, int b) {
    return a + b
}

void main() {
    int x = add(2, 3)
    int total = 0
    for int i in range(x) {            // 0 .. x-1;  range(a, b) = a .. b-1
        total += i
    }
    if total > 5 && x != LIMIT {
        print("total = {total}")       // Interpolation mit {ausdruck}
    } else if total == 0 {
        print("nichts")
    } else {
        print("klein")
    }
    str kind = if x % 2 == 0 { "gerade" } else { "ungerade" }   // if als Ausdruck
    print(kind)
}
```

- Anweisungen enden am Zeilenende, keine Semikolons. `{}` ist immer Pflicht, Klammern um Bedingungen nicht (`if (x > 0)` kompiliert trotzdem, `dif fmt` entfernt sie).
- Explizites `return`. Kein Top-Level-Code: ein Programm besteht aus Deklarationen und `void main()`.
- Kommentare nur `//`, Doku-Kommentare `///` (werden von `dif doc` gelesen und als Doc-Tests ausgeführt).
- Präzedenz wie C, mit drei Korrekturen: Bit-Operatoren binden stärker als Vergleiche (C-Falle `x & 1 == 0`), `a < b < c` ist ein Fehler, `&&` und `||` gemischt nur mit Klammern. `try` bindet wie ein Vorzeichen (`try a + try b`), `or` gehört zum `try` davor, `??` bindet am schwächsten.
- Eine Anweisung läuft in der nächsten Zeile weiter, wenn die Zeile mit einem Operator oder `,` endet, die nächste mit `.` beginnt oder man innerhalb von `( … )` bzw. `[ … ]` ist. `\{` schreibt eine wörtliche Klammer in einen String. Genaue Regeln: docs/spec.md §1.
- In Typen bindet `!E` am schwächsten: `[]Task!FileErr` ist eine Liste oder ein Fehler; `?[]T` ist eine optionale Liste, `[]?T` eine Liste von Optionalen.
- Kein `++`/`--`: `x += 1`. Wrapping-Arithmetik bewusst mit `+%`, `-%`, `*%`.

### 4.2 Typen

| Typ | Bedeutung |
|---|---|
| `int` | immer 64 Bit, Überlauf = Panic |
| `i8 i16 i32 u8 u16 u32 u64` | feste Breiten |
| `f64 f32` | Gleitkomma; Ausgabe in der kürzesten Form, die beim Zurücklesen denselben Wert ergibt (`5`, `22.5`, `0.30000000000000004`); andere Formate per `{x:g}`, `{x:.2f}` |
| `bool` | `true`/`false`; nur `bool` darf in `if`/`while` stehen |
| `str` | UTF-8, unveränderlich; `s.len()` in Bytes, `s.runes()` zum Iterieren |
| `rune` | Unicode-Codepoint, Literal `'ä'` |
| `[N]T` | Array fester Länge, liegt inline |
| `[]T` | Liste, wächst, Copy-on-Write |
| `map[K]V` | Map in Einfügereihenfolge (deterministische Ausgabe) |
| `?T` | Wert oder `none` |
| `T!E` | Wert oder Fehler `err(e)`; `void!E` = nichts oder Fehler |
| `void` | kein Rückgabewert |
| `struct`, `enum` | eigene Typen; `enum`-Varianten mit Payload |
| `*T` | C-Pointer, nur in `unsafe` und an der C-Grenze |

Umwandlungen folgen einer Regel (R8): `T(x)` wandelt exakt um oder bricht ab. Wer runden will, sagt es vorher.

```dif
int n = 7
f64 f = f64(n)          // exakt (bricht nur ab, wenn |n| > 2^53)
f64 x = 2.5
int i = int(x)          // panic: 2.5 is not an exact int (main.dif:4:9)
int j = int(x.round())  // 3, absichtlich gerundet
u8 b = u8(n)            // exakt, bricht ab, wenn n nicht in 0..255 liegt
?u8 c = n.to_u8()       // prüfen statt abbrechen: none, wenn es nicht passt
int k = int(2.5)        // steht schon beim Kompilieren fest → Compilerfehler, gleiche Regel
str s = "{n}"           // Zahl -> Text über Interpolation
str t = "41" + 1        // Compilerfehler: str + int
```

### 4.3 Wertsemantik, Methoden und `mut`

```dif
struct Point {
    int x
    int y
}

Point Point.moved(int dx) {            // Methode: self ist der Ort vor dem Punkt
    Point q = self                     // Kopie von self
    q.x += dx
    return q
}

void Point.scale(mut self, int k) {              // mut self: ändert den Aufrufer
    self.x *= k
    self.y *= k
}

void shift(mut Point p, int dx) {      // mut in der Signatur: ändert das Argument des Aufrufers
    p.x += dx
}

int gcd(int a, int b) {                // ohne mut: a und b sind lokale Kopien, wie in C
    while b != 0 {
        int t = b
        b = a % b
        a = t
    }
    return a
}

void main() {
    Point a = Point{x: 1, y: 2}
    Point b = a                  // Kopie, a bleibt {1, 2}
    b.x = 10                     // Zuweisung
    shift(mut b, 5)              // mut auch am Aufruf → b.x == 15
    b.scale(2)                   // Methode: arbeitet immer auf b → {30, 4}
    Point c = b.moved(1)
    print("{a} {b} {c}")         // Point{x: 1, y: 2} Point{x: 30, y: 4} Point{x: 31, y: 4}
    print(gcd(48, 18))           // 6

    shift(b, 5)                  // error[R6]: shift takes p as mut. hint: shift(mut b, 5)

    []int xs = []
    for int i in range(1_000_000) {
        xs.push(i)               // in place, amortisiert O(1): xs ist nicht geteilt
    }
    []int ys = xs                // O(1): nur Referenzzähler +1
    xs[0] = 42                   // jetzt geteilt → Copy-on-Write, ys bleibt unverändert
}
```

- Keine Klassen, keine Vererbung. Methoden heißen `Rückgabetyp Typ.name(…)` und werden nur im Modul des Typs definiert; `grep "Point.scale"` findet die Definition. Außer `mut self` steht `self` nie in der Parameterliste.
- `mut` steht in der Signatur und am Aufruf, an beiden Stellen sieht man, was sich ändert. Parameter ohne `mut` sind lokale Kopien wie in C (`gcd`).
- Eine Änderung, die danach nie gelesen wird, ist ein Fehler, weil sie nur eine Kopie träfe: `void nudge(Point p) { p.x += 1 }` und `for Task t in tasks { t.done = true }` werden gemeldet, mit Hinweis auf `mut Point p` bzw. `tasks[i].done = true`.
- Ob eine Methode `self` ändert, steht in der Signatur (`mut self`); der Compiler prüft es wie bei `mut`-Parametern (R6).
- Jede Zuweisung und Übergabe ist logisch eine Kopie. Teuer wird das nie unnötig: `str`, `[]T`, `map` sind referenzgezählt und werden erst beim Schreiben kopiert, und nur wenn sie geteilt sind. Große Parameter übergibt der Compiler intern per Zeiger.
- **Keine Referenzen im sicheren Code → keine Zyklen → Referenzzählung allein räumt alles deterministisch ab.** Darum braucht difficulty weder GC noch Borrow-Checker.
- Eine Variable geht pro Aufruf höchstens einmal veränderbar hinein: `swap(mut a, mut a)` und `a.merge(mut a)` sind Compilerfehler (keine Aliase). `a.merge(a)` ist erlaubt: das gelesene `a` ist ein Schnappschuss.
- Lokale Variablen sind veränderlich wie in C; `const int MAX = 8` macht sie unveränderlich. Kein Shadowing.

### 4.4 Kontrollfluss und `switch`

```dif
enum Shape {
    Circle(f64 r)
    Rect(f64 w, f64 h)
    Dot
}

f64 area(Shape s) {
    switch s {
    case Shape.Circle(f64 r):
        return 3.14159 * r * r
    case Shape.Rect(f64 w, f64 h):
        return w * h
    case Shape.Dot:
        return 0
    }
}   // fehlt ein case: Compilerfehler mit Liste der fehlenden Varianten

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
```

- `switch` hat kein Fallthrough, ein `case` endet von selbst. `break` steht nie direkt im `switch` (in C und Go würde es dort den `switch` verlassen, nicht die Schleife; das wäre eine Falle). Wer aus einer Schleife heraus will, entscheidet das nach dem `switch` oder kehrt mit `return` zurück; `continue` ist eindeutig und erlaubt.
- Über einem `enum` muss jede Variante behandelt sein oder ein `default` stehen; über `int`/`str` ist `default` Pflicht.
- Varianten sind immer qualifiziert: `Seat.Taken`, `BookErr.NoBlock(n)`.
- Schleifen: `for int i in range(n)`, `for int i in range(a, b)`, `for Seat s in hall.seats`, `for str line in io.lines()`, `while cond`, dazu `break` und `continue`. `for` iteriert über einen Schnappschuss (O(1) dank Referenzzählung), Änderungen am Original im Rumpf sind also sicher.

### 4.5 Fehler: `?T`, `T!E`, `try`, `??`, Panics

Zwei Arten von Fehlern, sauber getrennt:

- **Erwartbare Fehler** (Eingabe falsch, Datei fehlt) stehen im Typ: `T!E`. Erfolg gibt man direkt zurück, Fehler mit `err(...)`.
- **Programmierfehler** (Index außerhalb, Überlauf, Division durch 0, `assert`) sind Panics: Abbruch mit Meldung und `.dif`-Position, immer aktiv, auch im Release-Build.

```dif
enum AgeErr {
    Empty
    NotANumber(str text)
    OutOfRange(int n)
}

int!AgeErr parse_age(str s) {
    if s.len() == 0 {
        return err(AgeErr.Empty)
    }
    int n = try s.parse_int() or AgeErr.NotANumber(s)   // ParseErr → AgeErr übersetzt
    if n < 0 || n > 150 {
        return err(AgeErr.OutOfRange(n))
    }
    return n                                      // Erfolg: Wert direkt
}

int!AgeErr total_age(str a, str b) {
    return try parse_age(a) + try parse_age(b)    // try reicht den Fehler sofort weiter
}

void main() {
    int age = parse_age("42") ?? 0                // Default ausdrücklich
    switch parse_age("x") {
    case ok(int n):
        print("Alter {n}")
    case err(AgeErr e):
        print("Fehler: {e}")                      // Fehler: NotANumber(text: "x")
    }
}
```

Ergebnisse darf man nicht still ignorieren:

```dif
hall.book(3, 5)              // error[R13]: result of type `void!BookErr` is ignored
try hall.book(3, 5)          // weiterreichen
_ = hall.book(3, 5)          // bewusst verwerfen
```

Panic-Ausgabe:

```
panic: index 40 out of bounds for []Seat of length 40
  at kino.dif:31:13 in Hall.book
```

`void!E main()` ist erlaubt: ein Fehler aus `main` wird auf stderr ausgegeben, Exit-Code 1.

### 4.6 Testen

Tests gehören zur Sprache; `dif test` ist das Werkzeug drumherum.

```dif
/// Addiert zwei Zahlen.
/// ```dif
/// expect(add(2, 3) == 5)
/// ```
int add(int a, int b) {
    return a + b
}

test "addition" {
    expect(add(2, 3) == 6)
    expect(add(-1, 1) == 0)
}

test "index außerhalb" panics "out of bounds" {
    []int xs = [1, 2, 3]
    print(xs[5])
}
```

```
$ dif test math.dif
FAIL math.dif:10 test "addition"
    expect(add(2, 3) == 6)
           add(2, 3) = 5
ok   math.dif:15 test "index außerhalb"
ok   math.dif:3  doc-test add
1 of 3 tests failed
```

| Werkzeug | Was es tut |
|---|---|
| `test "name" { }` | Testblock neben dem Code; `try` im Test lässt ihn bei Fehler mit Position scheitern |
| `expect(cond)` | zeigt bei Fehlschlag die Werte der Teilausdrücke |
| `test "…" panics "text" { }` | besteht nur, wenn die Panic-**Meldung** den Text enthält (nie gegen den Quelltext) |
| `dif test [pfad] [filter] [--json]` | jeder Test in eigenem Kindprozess: Panics, Segfaults in C-Code und Timeouts treffen nur diesen Test; parallel per Default (keine veränderlichen Globalen → nichts geteilt) |
| `dif test --coverage` | nicht getestete Zeilen und Zweige, auf `.dif`-Zeilen abgebildet (gcov über `#line`) |
| `dif test --sanitize` | AddressSanitizer + UBSan, wichtig für C- und `unsafe`-Code, Fundstellen als `.dif`-Position |
| Doc-Tests | Codeblöcke in `///`-Kommentaren und Markdown (`llms.txt`, `docs/`) laufen als Tests, in drei Arten: ` ```dif ` ist ein ganzes Modul, ` ```dif snippet ` wird in `void main()` eingebettet, ` ```dif error R13 ` erwartet genau diese Diagnose. Doku kann nicht veralten |

Nach v0: Benchmarks (`bench "push" { }`, `dif bench`), Property-Tests/Fuzzing.

### 4.7 C-Bibliotheken

Weil das Backend C erzeugt, ist C ein Nachbar: Der C-Compiler kümmert sich um ABI und Struct-Layout, wir brauchen nur Namen und Typen. Fünf Wege, vom bequemsten zum genauesten:

**1. Header importieren** (`use c`): alles aus dem Header ist sofort nutzbar.

```dif
use c "math.h" link "m"
use c "sqlite3.h" link "sqlite3" strip "sqlite3_"     // sqlite3_open → sqlite3.open
use c "SDL3/SDL.h" link pkg "sdl3" as sdl             // Flags über pkg-config

void main() {
    print("cbrt(27) = {math.cbrt(27)}")
    str v = c.to_str(sqlite3.libversion()) ?? "?"    // c.str → str: kopiert + UTF-8-Prüfung
    print("SQLite {v}")
}
```

So funktioniert es: `dif` lässt eine Probe-Datei mit `#include <sqlite3.h>` durch `cc -E -dD` laufen und liest die vorverarbeiteten Deklarationen mit einem eigenen C-Deklarationsparser, ohne libclang. Typabbildung:

| C | dif |
|---|---|
| `int32_t`, `uint8_t`, `double`, `_Bool` … | `i32`, `u8`, `f64`, `bool` |
| `int`, `long`, `size_t` … | `c.int`, `c.long`, `c.size` |
| `const char*` als Parameter | `?c.str`: man übergibt einfach `str` (oder `none` für `NULL`); dif-Strings sind intern NUL-terminiert, der Pointer gilt, solange der String lebt; ein C-struct hält die dif-Werte in seinen Feldern selbst am Leben |
| `const char*` als Rückgabe | `?c.str`, lesbar über `c.to_str(p) -> ?str` |
| Pointer auf opakes `struct S` (`rk_view*`) | Griff `?S`; ein Callback-Parameter vom Typ `S` wird beim Aufruf auf `NULL` geprüft (Abbruch wie bei `T(x)`) |
| andere Pointer `T*` | `*T`, nur in `unsafe`, darf wie in C NULL sein (`p.is_null()`) |
| `struct S { … }` | struct mit Feldern; nicht genannte Felder im Literal sind 0, `c.sizeof(S)` gibt die Größe |
| Paar aus Funktionspointer und `void *user` | dif-Funktion plus Kontext-Variable (R24) |
| `enum`, `#define N 42` | Konstanten |
| Funktionsmakros, varargs (`printf`) | werden nicht importiert; `dif doc sqlite3` listet sie mit Grund, nie still verworfen |

**2. `.so`/`.dll` direkt linken**

```dif
use c "./vendor/foo.h" link "./vendor/libfoo.so"   // Pfad relativ zu DIESER .dif-Datei
```

Relative Bibliotheken werden mit rpath `$ORIGIN` gelinkt: Binary und `.so` im selben Ordner, läuft. Unter Windows (später) liegt die `.dll` neben der `.exe`. `link "name"` sucht wie `-lname`, `link pkg "name"` fragt pkg-config, `dif build --static` linkt statisch, wo `.a`-Dateien vorhanden sind.

**3. Zur Laufzeit laden** (Plugins, optionale Bibliotheken):

```dif
use c "plugin.h" load                 // nichts linken; Symbole werden beim Öffnen geholt

void!dl.Err main() {
    plugin.Lib p = try plugin.open(dl.name("plugin"))   // libplugin.so / plugin.dll / libplugin.dylib
    int r = p.plugin_run(42)
    print("plugin: {r}")
}   // letzte Kopie von p weg → dlclose / FreeLibrary
```

`open` löst alle Symbole des Headers auf einmal auf; fehlt eines, kommt `err(dl.Err.MissingSymbol("plugin_run"))` zurück statt eines Absturzes beim ersten Aufruf.

**4. C-Quellen mitbauen**

```dif
use c "./vendor/miniz.h" source "./vendor/miniz.c"                          // .c-Datei mitkompilieren
use c "./vendor/stb_image.h" impl "STB_IMAGE_IMPLEMENTATION"                // Single-Header-Bibliothek
```

**5. Von Hand deklarieren** (`extern`), wenn man genauere Typen will als der Header hergibt:

```dif
extern "stdlib.h" {
    ?c.str getenv(c.str name)         // NULL wird zu none
    *void malloc(c.size n)
    void free(*void p)
}

void main() {
    switch getenv("HOME") {
    case some(c.str _):
        print("HOME ist gesetzt")
    case none:
        print("HOME fehlt")
    }
    unsafe {                          // Pointer nur hier; hier gelten C-Regeln
        *void buf = malloc(64)
        assert(!buf.is_null(), "malloc failed")
        free(buf)
    }
}
```

Handdeklarationen prüft der echte Header: das erzeugte C weist jede `extern`-Funktion einem Funktionspointer der deklarierten Signatur zu, eine Abweichung meldet `dif` an der `extern`-Zeile, nie als rohen C-Fehler.

```c
#include <stdlib.h>
static void *(*const dif_chk_malloc)(size_t) = malloc;
```

`dif bindgen sqlite3.h --strip sqlite3_ > sqlite3.dif` erzeugt aus einem Header einen solchen `extern`-Block als lesbare, versionierbare Datei (gleicher Parser wie `use c`).

**6. Callbacks mit Zustand und Aufräumen** (R23, R24), am Beispiel rekta-core: C ruft `build` und `on_press` später zurück und reicht nur `void *user` durch. dif bindet dort eine Variable, die gerade mit `mut` in einem laufenden Aufruf steckt (hier: `rk.app_main(…, user: mut app)` in `main`), und erzeugt den Trampolin; ruft C außerhalb dieses Aufrufs oder aus einem fremden Thread zurück, bricht der Trampolin mit Meldung ab. `defer` schließt, was geöffnet wurde.

```dif
use c "rekta.h" link pkg "rekta" strip "rk_" as rk

struct App {
    int count = 0
}

void add_one(mut App app, rk.view button) {
    app.count += 1
}

void build(mut App app) {
    if rk.ui_open(rk.ui{size: c.sizeof(rk.ui), kind: rk.UI_VSTACK}) {
        defer rk.ui_close()
        rk.ui_put(rk.ui{size: c.sizeof(rk.ui), kind: rk.UI_LABEL, text: "{app.count}"})
        rk.ui_put(rk.ui{size: c.sizeof(rk.ui), kind: rk.UI_BUTTON, text: "+1", on_press: add_one, user: mut app})
    }
}
```

Regeln (R17): Funktionen mit `*T` in der Signatur sind nur in `unsafe` aufrufbar. Für alle anderen gelten sichere Standards: Parameter sind nicht optional, Strings gelten nur für die Dauer des Aufrufs, Griffe mit Freigeber (`use c "sqlite3.h" link "sqlite3" release sqlite3 with sqlite3_close`) werden gezählt und genau einmal freigegeben. Was danach bleibt, ist die ehrlich benannte Vertrauensgrenze: Fehler in der C-Bibliothek selbst und Bibliotheken, die übergebene Pointer gegen ihre Dokumentation behalten. Nach v0: dif-Funktionen exportieren (`export int add(int a, int b)`, `dif build --lib` → `.so`/`.dll` + Header).

### 4.8 Module

```dif
// geo.dif
pub struct Point {
    f64 x
    f64 y
}

pub f64 dist(Point a, Point b) {
    f64 dx = a.x - b.x
    f64 dy = a.y - b.y
    return (dx * dx + dy * dy).sqrt()
}
```

```dif
// main.dif
use geo                                   // ./geo.dif relativ zu DIESER Datei, nie zum CWD

void main() {
    f64 d = geo.dist(geo.Point{x: 0, y: 0}, geo.Point{x: 3, y: 4})
    print("{d}")                          // 5
}
```

Eine Datei ist ein Modul, `pub` exportiert, sonst privat. `std.*` ist in den Compiler eingebettet und lässt sich nicht still durch lokale Dateien ersetzen.

### 4.9 Nebenläufigkeit (nach v0)

Weil es keine Referenzen und keine veränderlichen Globalen gibt, können zwei Tasks nie dieselbe Variable ändern: **Data Races sind schon durch das Design unmöglich**, ohne Borrow-Checker und ohne Locks. Tasks treffen sich nur an Kanälen und beim Warten; das macht Deadlocks erkennbar und Tests reproduzierbar. Kein `async`/`await`: Funktionen werden nicht „gefärbt“, blockierender Code bleibt einfacher Code.

```dif
use std.time
use std.fs

int count_words(str text) {
    return text.split(" ").len()
}

void produce(int n, chan[int] out) {      // out ist ein Griff: send ändert den Kanal, nicht out
    for int i in range(n) {
        out.send(i * i)
    }
    out.close()
}

void main() {
    // 1. Tasks: parallel rechnen, Ergebnis abholen
    task[int] a = spawn count_words("ein langer text")
    task[int] b = spawn count_words("noch einer")
    print("Wörter: {a.wait() + b.wait()}")

    // 2. Kanal: Producer/Consumer
    chan[int] squares = chan.new[int](64)
    spawn produce(10, squares)
    for int sq in squares {                // endet, wenn produce den Kanal schließt
        print(sq)
    }

    // 3. Parallele Schleife: alle Kerne, Reihenfolge bleibt
    []str texts = ["a b", "c d e", "f"]
    []int counts = parallel for str t in texts {
        yield count_words(t)
    }

    // 4. Timeout und Abbruch
    str big = fs.read("big.txt") ?? ""
    task[int] slow = spawn count_words(big)
    switch slow.wait_for(time.sec(2)) {
    case some(int n):
        print("{n} Wörter")
    case none:
        slow.cancel()                      // kooperativ: der Task fragt task.cancelled() ab
        print("zu langsam")
    }
}   // verlässt main den Rumpf, werden noch laufende Tasks abgebrochen und abgewartet

test "producer" {
    chan[int] ch = chan.new[int](1)
    spawn produce(3, ch)
    []int got = []
    for int x in ch {
        got.push(x)
    }
    expect(got == [0, 1, 4])
}
```

```
$ dif test shop.dif --schedules 200      # 200 verschiedene Abläufe, deterministisch per Seed
ok   shop.dif:40 test "producer" (200 schedules)

$ dif run shop.dif
panic: deadlock: all tasks are blocked
  main     waits in ch.recv()    at shop.dif:21:9
  produce  waits in out.send()   at shop.dif:8:9
```

| Baustein | Was er bringt |
|---|---|
| `spawn` / `task[T]` / `wait` | mehrere Kerne nutzen; Fehler und Abbrüche kommen beim Aufrufer an; keine verwaisten Threads |
| `chan[T]` | Producer/Consumer, Pipelines, Server; Werte werden übertragen, nie geteilt |
| `parallel for … yield` | Rechenlast mit einem Wort parallelisieren, Ergebnis in stabiler Reihenfolge, `print` in Reihenfolge der Durchläufe |
| `wait_for`, `recv_for`, `try_recv`, `cancel` | Timeouts, nicht blockierendes Abholen und Abbruch als Methoden, ohne neue Syntax |
| abbrechbares IO | nach `t.cancel()` kehren blockierende std-Aufrufe im Task (`net`, `fs`, `sleep`, `recv`, `wait`) sofort mit `err(…Cancelled)` bzw. `none` zurück; `try` reicht es nach oben |
| `shared[T]` | große Daten (Layout-Baum, Cache, Textur) ohne Kopie an viele Tasks; unveränderlich, also weiter race-frei; atomare Zählung nur für `shared` |
| Inbox-Muster + `time.every`/`time.after` | auf mehrere Quellen warten, ohne `select`: alle senden Varianten eines `enum` in einen Kanal |
| Hauptthread-Garantie + `ch.fd()` | `main` läuft im OS-Hauptthread (SDL, GTK, Cocoa verlangen das); `ch.fd()` wird lesbar, sobald Nachrichten da sind, für C-Eventloops ohne Polling |
| Deadlock-Erkennung | statt stillem Hängen: wer wartet worauf, an welcher Zeile |
| reproduzierbare Tests | `dif test` fährt Nebenläufigkeit deterministisch (Wechsel nur an Kanälen und `wait`, Reihenfolge per Seed); `--schedules N` probiert N Abläufe, ein Fehlschlag nennt den Seed zum Wiederholen |

**Prüfstein Webbrowser:** UI-Schleife im Hauptthread, Downloads als Tasks, alles fließt über eine Inbox, Layout geht als `shared` an die Raster-Tasks, ein geschlossener Tab bricht seine Downloads ab.

```dif
enum Msg {
    Page(int tab, str html)
    Input(ui.Event e)
    Tick
}

str!net.Err fetch(int tab, str url, chan[Msg] inbox) {
    str html = try net.get(url)
    inbox.send(Msg.Page(tab, html))
    return html
}

void main() {
    Browser b = Browser{}
    chan[Msg] inbox = chan.new[Msg](256)
    spawn time.every(time.ms(16), inbox, Msg.Tick)
    task[str!net.Err] load = spawn fetch(1, "https://example.org", inbox)
    ui.watch_fd(inbox.fd())
    for Msg m in inbox {
        switch m {
        case Msg.Page(int tab, str html):
            b.open(tab, html)
        case Msg.Input(ui.Event e):
            if e.is_close_tab() {
                load.cancel()
            }
        case Msg.Tick:
            shared[Layout] snap = share(b.layout())
            []Tile tiles = parallel for Rect r in screen_tiles() {
                yield raster(snap, r)
            }
            ui.present(tiles)
        }
    }
}
```

Runtime: zuerst ein OS-Thread pro Task (pthreads, wenige hundert Zeilen C in `dif_rt.h`), später leichte Tasks (M:N-Scheduler) für sehr viele gleichzeitige Verbindungen, ohne dass sich Code ändert. Referenzzählung bleibt nicht atomar: Werte, die in einen Task oder Kanal gehen, werden übertragen (bei letzter Nutzung verschoben, sonst kopiert), `parallel for` liest äußere Variablen ohne Zähleroperationen, und nur `shared[T]` zählt atomar. Mehrere Prozesse (Site-Isolation wie in Chrome) gehen über `std.process` und Pipes, nicht über neue Sprachmittel.

### 4.10 Benannte Parameter, Funktionswerte, Blöcke, Generics (v0)

Drei Sprachmittel machen Bibliotheken so angenehm wie SwiftUI, ohne Schnickschnack:

```dif
void tile(str value, str label = "", Status status = Status.None, ?Msg on_press = none) { … }

ui.tile("{n}")
ui.tile("0.3", label: "release notes", on_press: Msg.OpenNotes)
```

**Benannte Parameter mit Default (R25):** Parameter mit Default stehen hinten und werden beim Aufruf beim Namen genannt, alle anderen der Reihe nach.

```dif
[]Person adults = people.filter(bool(Person p) { return p.age >= 18 })
people.sort_by(bool(Person a, Person b) { return a.age < b.age })

int total = 0
for int x in xs {
    total += x
}

void!DbErr transfer(db.Conn conn, int from, int to, int amount) {
    try conn.transaction() {
        try conn.debit(from, amount)
        try conn.credit(to, amount)
    }
}

str body = try net.retry(3) {
    return try net.get(url)
}

html.Page page = html.Page{}
page.h1("Bestellung {o.id}")
page.list() {
    for Item i in o.items {
        page.li("{i.name}: {i.price:.2f} €")
    }
}
str text = page.text()
```

`db.Conn` und `html.Page` sind Griffe (R5; eine Verbindung bzw. eine Seite mit einem Textpuffer dahinter, kein struct): ihre Methoden schreiben hinter den Griff und ändern ihn selbst nicht. Der Block fängt eine Kopie des Griffs ein, und die ist derselbe Griff; darum wirkt `page.li(…)` im Block auf die Seite von außen, ohne gegen R6 zu verstoßen. Ein Builder ohne Griff müsste sich selbst ändern und ginge so nicht. Eine Zahl oder Liste ringsum ändert ein Block nie: `n += 1` im Block ist R6 (`the change to `n` is lost: `n` is a copy`), dafür gibt es die `for`-Schleife oder einen Parameter.

**Funktionswerte (R26):** Ein Funktionstyp heißt `R(T1, T2)`, ein Literal sieht aus wie eine Funktion ohne Namen: `bool(Person p) { return p.age >= 18 }`. Ein Literal steht nur direkt als Argument und lebt nur während dieses Aufrufs. **Werte-Captures:** Beim Auswerten nimmt es eine Momentaufnahme jedes Namens ringsum, den es benutzt, und ändert nie etwas ringsum; eine Änderung an einem eingefangenen Namen ist R6. Die Kopie eines Griffs ist derselbe Griff, darum können Blöcke über Griffe wirken. Weil Literale nur Werte einfangen, gibt es keine Zyklen und keinen GC, und ab M3 lassen sich Funktionswerte speichern, vergleichen (gleiche Quelltextstelle und gleiche eingefangene Werte) und drucken (`fn@main.dif:12:5`). Ein Literal ohne Parameter darf als **Block** `{ … }` direkt hinter der schließenden Klammer stehen; `return` und `try` darin beenden den Block. Callbacks für später laufen über Nachrichten (`enum`) oder `spawn`.

Das Literal gehört zum Aufrufer: es sieht dessen Variablen (als Kopien), und `self` ist darin eine Kopie des `self` des Aufrufers. Das Element kommt immer als benannter Parameter (`p`), nie implizit.

```dif
struct Pair[A, B] {
    A first
    B second
}

[]B convert[A, B]([]A xs, B(A) f) {
    []B out = []
    for A x in xs {
        out.push(f(x))
    }
    return out
}

[]str names = convert(people, str(Person p) { return p.name })
```

**Generics (R27):** Typparameter in eckigen Klammern nach dem Namen, beim Aufruf meist abgeleitet. Monomorphisierung: ein C-Exemplar pro Instanz. Einschränkungen stehen vor dem Typparameter: `T max[ordered T](T a, T b)`; automatische Fähigkeiten regelt R28.

| | |
|---|---|
| **+** | Transaktionen, Builder, Zeitmessung, Retry und Test-Fixtures erzwingen Vorher/Nachher in der Bibliothek; Struktur = Einrückung; Blöcke kosten nichts, weil der Compiler sie inline einsetzen kann |
| **−** | Man sieht nicht, wie oft ein Block läuft (der Funktionsname muss es sagen); `return` im Block beendet nur den Block; Builder brauchen einen Griff |

### 4.11 UI im SwiftUI-Stil (rekta-Modul)

Mit 4.10 wird aus rekta-core eine Bibliothek, die sich überfliegen lässt. Elemente senden Nachrichten, eine Funktion `update` ändert den Zustand, `view` beschreibt, was zu sehen ist (Elm-Architektur):

```dif
use rekta as ui

struct App {
    int finished = 312
}

enum Msg { BuildOne, OpenNotes }

void update(mut App app, Msg m) {
    switch m {
    case Msg.BuildOne:
        app.finished += 1
    case Msg.OpenNotes:
        ui.toast("Release notes are on their way.")
    }
}

void view(App app, ui.Builder b) {
    b.panorama() {
        b.section("today") {
            b.label("Today", style: ui.Style.Group)
            b.tiles() {
                b.tile("{app.finished}", label: "finished today", status: ui.Status.Success)
                b.tile("0.3", label: "release notes", on_press: Msg.OpenNotes)
            }
            b.button("build one", on_press: Msg.BuildOne)
        }
    }
}

void main() {
    App app = App{}
    ui.run("My tool", mut app, view, update)
}

test "build one zählt" {
    App app = App{}
    update(mut app, Msg.BuildOne)
    expect(app.finished == 313)
}
```

Alle Zustandsänderungen stehen in `update`, testbar ohne Bildschirm; eine neue Aktion ist eine neue Variante, und der Compiler zeigt, wo sie fehlt. `view` bekommt den Builder ausdrücklich, es gibt keinen versteckten globalen Zustand. `ui.Builder` ist ein Griff auf den Zustand in rekta-core, darum braucht `b` kein `mut` und Block und Methode dürfen ihn beide benutzen (R5, R6). Das Modul übersetzt Blöcke in `rk_ui_open`/`rk_ui_close`, setzt `size` und `kind` selbst und nutzt intern den Kontext aus R24. Seine Signatur zeigt R6, R26 und R27 zusammen: `void run[A, M](str title, mut A app, void(A, ui.Builder) view, void(mut A, M) update)`.

Ein realitätsnahes Beispiel, eine Aufgabenliste mit Datei, Eingabefeld, Fehlern und Tests:

```dif
use std.fs
use rekta as ui

const str FILE = "aufgaben.txt"

struct Task {
    str title
    bool done = false
}

struct App {
    []Task tasks = []
    str draft = ""
    str status = ""
}

enum Msg {
    Draft(str text)
    Add
    Toggle(int index)
    ClearDone
}

enum FileErr {
    Io(str path)
    BadLine(int line)
}

str message(FileErr e) {
    switch e {
    case FileErr.Io(str path):
        return "{path} lässt sich nicht schreiben"
    case FileErr.BadLine(int n):
        return "Zeile {n} in {FILE} ist ungültig"
    }
}

[]Task!FileErr parse(str text) {
    []Task tasks = []
    []str lines = text.split("\n")
    for int i in range(lines.len()) {
        str line = lines[i].trim()
        if line == "" {
            continue
        }
        if line.starts_with("[x] ") {
            tasks.push(Task{title: line.slice(4, line.len()), done: true})
        } else if line.starts_with("[ ] ") {
            tasks.push(Task{title: line.slice(4, line.len())})
        } else {
            return err(FileErr.BadLine(i + 1))
        }
    }
    return tasks
}

str render([]Task tasks) {
    []str lines = tasks.map(str(Task t) {
        return if t.done { "[x] {t.title}" } else { "[ ] {t.title}" }
    })
    return lines.join("\n")
}

void!FileErr save([]Task tasks) {
    try fs.write(FILE, render(tasks)) or FileErr.Io(FILE)
}

void persist(mut App app) {
    switch save(app.tasks) {
    case ok:
        app.status = "{app.tasks.len()} Aufgaben gespeichert"
    case err(FileErr e):
        app.status = message(e)
    }
}

void update(mut App app, Msg m) {
    switch m {
    case Msg.Draft(str text):
        app.draft = text
    case Msg.Add:
        str title = app.draft.trim()
        if title == "" {
            return
        }
        app.tasks.push(Task{title: title})
        app.draft = ""
        persist(mut app)
    case Msg.Toggle(int i):
        app.tasks[i].done = !app.tasks[i].done
        persist(mut app)
    case Msg.ClearDone:
        app.tasks = app.tasks.filter(bool(Task t) { return !t.done })
        persist(mut app)
    }
}

void view(App app, ui.Builder b) {
    int open = app.tasks.filter(bool(Task t) { return !t.done }).len()
    b.panorama(title: "Aufgaben") {
        b.section("heute") {
            b.tiles() {
                b.tile("{open}", label: "offen", status: if open == 0 { ui.Status.Success } else { ui.Status.Info })
                b.tile("{app.tasks.len() - open}", label: "erledigt")
            }
            b.hstack() {
                b.field("Neue Aufgabe", text: app.draft, on_change: Msg.Draft, on_submit: Msg.Add)
                b.button("Hinzufügen", on_press: Msg.Add, disabled: app.draft.trim() == "")
            }
            b.rows() {
                for int i in range(app.tasks.len()) {
                    b.check(app.tasks[i].title, on: app.tasks[i].done, on_change: Msg.Toggle(i))
                }
            }
            b.button("Erledigte entfernen", on_press: Msg.ClearDone, disabled: open == app.tasks.len())
            b.label(app.status, style: ui.Style.Small)
        }
    }
}

void main() {
    App app = App{}
    switch parse(fs.read(FILE) ?? "") {
    case ok([]Task tasks):
        app.tasks = tasks
    case err(FileErr e):
        app.status = message(e)
    }
    ui.run("Aufgaben", mut app, view, update)
}

test "laden und speichern" {
    []Task tasks = [Task{title: "Milch", done: true}, Task{title: "Steuer"}]
    expect(render(tasks) == "[x] Milch\n[ ] Steuer")
    expect(parse(render(tasks)) == tasks)
    expect(parse("kaputt") == err(FileErr.BadLine(1)))
}

test "hinzufügen und abhaken" {
    try fs.with_temp_dir() {
        App app = App{}
        update(mut app, Msg.Draft("  Milch kaufen "))
        update(mut app, Msg.Add)
        update(mut app, Msg.Toggle(0))
        expect(app.tasks == [Task{title: "Milch kaufen", done: true}])
        expect(app.draft == "")
        expect((fs.read(FILE) ?? "") == "[x] Milch kaufen")
    }
}

test "leere Eingabe" {
    App app = App{}
    update(mut app, Msg.Add)
    expect(app.tasks.len() == 0)
}
```

### 4.12 Später

JSON typisiert: `json.decode[User](text)` und `json.encode(u)`, die Umwandlung leitet der Compiler für structs, enums, Listen, Maps und `?T` ab; fehlende Pflichtfelder und falsche Typen werden Fehlerwerte (`json.Err.Missing("User.name")`). Dazu Benchmarks in der Sprache (`bench "x" { }`) und Export als `.so`/`.dll`.

---

## 5. Komplettes Beispiel: `kino.dif`

Ein Programmkino mit einem Saal: 5 Reihen × 8 Plätze, Einzel- und Gruppenbuchung, Storno, Saalplan, Tagesabschluss.

```dif
// kino.dif — Programmkino mit einem Saal: 5 Reihen x 8 Plätze, Einzel- und Gruppenbuchung,
// Storno, Saalplan, Tagesabschluss. Befehle von stdin, `q` beendet.
//   dif run examples/kino.dif < sitzung.txt
// `dif test examples/kino.dif` führt die Tests aus.
use std.io

const int ROWS = 5
const int SEATS = 8
const str HELP = "Befehle: b <reihe> <platz> | g <n> | s <reihe> <platz> | p | t | q"

enum Seat { Free, Taken }

enum BookErr {
    NoSuchSeat(int row, int seat)
    AlreadyTaken(int row, int seat)
    NotBooked(int row, int seat)
    BadGroup(int size)
    NoBlock(int size)
}

enum CmdErr {
    Book(BookErr cause)
    BadNumber(str text)
    Missing(str cmd)
}

struct Block {
    int row
    int first
    int size
}

struct Hall {
    []Seat seats = repeat(Seat.Free, ROWS * SEATS)
}

int!BookErr index(int row, int seat) {
    if row < 1 || row > ROWS || seat < 1 || seat > SEATS {
        return err(BookErr.NoSuchSeat(row, seat))
    }
    return (row - 1) * SEATS + (seat - 1)
}

void!BookErr Hall.book(mut self, int row, int seat) {
    int i = try index(row, seat)
    if self.seats[i] == Seat.Taken { return err(BookErr.AlreadyTaken(row, seat)) }
    self.seats[i] = Seat.Taken
}

void!BookErr Hall.cancel(mut self, int row, int seat) {
    int i = try index(row, seat)
    if self.seats[i] == Seat.Free { return err(BookErr.NotBooked(row, seat)) }
    self.seats[i] = Seat.Free
}

// Books the first row with `n` free seats side by side.
Block!BookErr Hall.book_group(mut self, int n) {
    if n < 1 || n > SEATS { return err(BookErr.BadGroup(n)) }
    for int r in range(ROWS) {
        int free = 0
        for int p in range(SEATS) {
            free = if self.seats[r * SEATS + p] == Seat.Free { free + 1 } else { 0 }
            if free == n {
                int first = r * SEATS + p - n + 1
                for int i in range(first, first + n) { self.seats[i] = Seat.Taken }
                return Block{row: r + 1, first: p - n + 2, size: n}
            }
        }
    }
    return err(BookErr.NoBlock(n))
}

str Hall.plan() {
    str out = "         1 2 3 4 5 6 7 8   (. frei, X belegt)"
    for int r in range(ROWS) {
        []Seat row = self.seats.slice(r * SEATS, (r + 1) * SEATS)
        str cells = row.map(str(Seat s) { return if s == Seat.Taken { "X" } else { "." } }).join(" ")
        out += "\nReihe {r + 1}: {cells}"
    }
    return out
}

str Hall.summary() {
    int sold = self.seats.count(Seat.Taken)
    f64 pct = f64(sold) / f64(self.seats.len()) * 100
    return "Tickets verkauft: {sold} von {self.seats.len()}, Auslastung: {pct:.1f} %"
}

str BookErr.message() {
    switch self {
    case BookErr.NoSuchSeat(int r, int p):
        return "Fehler: Reihe {r}, Platz {p} gibt es nicht."
    case BookErr.AlreadyTaken(int r, int p):
        return "Fehler: Reihe {r}, Platz {p} ist schon besetzt."
    case BookErr.NotBooked(int r, int p):
        return "Fehler: Reihe {r}, Platz {p} ist gar nicht gebucht."
    case BookErr.BadGroup(int n):
        return "Fehler: Gruppengröße muss 1 bis {SEATS} sein, nicht {n}."
    case BookErr.NoBlock(int n):
        return "Kein Block mit {n} freien Plätzen nebeneinander mehr frei."
    }
}

str CmdErr.message() {
    switch self {
    case CmdErr.Book(BookErr e):
        return e.message()
    case CmdErr.BadNumber(str text):
        return "Fehler: `{text}` ist keine Zahl."
    case CmdErr.Missing(str cmd):
        return "Fehler: `{cmd}` braucht mehr Zahlen."
    }
}

// The number at word `k` of a command.
int!CmdErr number([]str words, int k) {
    if k >= words.len() { return err(CmdErr.Missing(words[0])) }
    switch words[k].parse_int() {
    case some(int n):
        return n
    case none:
        return err(CmdErr.BadNumber(words[k]))
    }
}

str!CmdErr exec(mut Hall hall, str line) {
    []str w = line.split(" ")
    switch w[0] {
    case "b":
        int row = try number(w, 1)
        int seat = try number(w, 2)
        try hall.book(row, seat) or CmdErr.Book
        return "Reihe {row}, Platz {seat} gebucht."
    case "g":
        int n = try number(w, 1)
        Block blk = try hall.book_group(n) or CmdErr.Book
        return "Gruppe ({n}) gebucht: Reihe {blk.row}, Plätze {blk.first} bis {blk.first + n - 1}."
    case "s":
        int row = try number(w, 1)
        int seat = try number(w, 2)
        try hall.cancel(row, seat) or CmdErr.Book
        return "Reihe {row}, Platz {seat} storniert."
    case "p":
        return hall.plan()
    case "t":
        return hall.summary()
    default:
        return HELP
    }
}

void!io.IoErr main() {
    Hall hall = Hall{}
    print(HELP)
    for str line in try io.lines() {          // endet bei EOF, kein ""-Sonderfall
        str cmd = line.trim()
        if cmd == "q" { break }
        switch exec(mut hall, cmd) {
        case ok(str msg):
            print(msg)
        case err(CmdErr e):
            print(e.message())
        }
    }
    print(hall.summary())
}

test "einzelplatz" {
    Hall h = Hall{}
    try h.book(3, 5)
    expect(h.seats[20] == Seat.Taken)
    expect(h.book(3, 5) == err(BookErr.AlreadyTaken(3, 5)))
    expect(h.book(6, 1) == err(BookErr.NoSuchSeat(6, 1)))
    try h.cancel(3, 5)
    expect(h.cancel(3, 5) == err(BookErr.NotBooked(3, 5)))
    expect(h.seats.count(Seat.Taken) == 0)
}

test "gruppe" {
    Hall h = Hall{}
    expect(try h.book_group(8) == Block{row: 1, first: 1, size: 8})
    try h.book(2, 3)
    Block b = try h.book_group(4)
    expect(b.row == 2 && b.first == 4)
    expect(h.book_group(9) == err(BookErr.BadGroup(9)))
    expect(h.seats.count(Seat.Taken) == 13)
}

test "voll" {
    Hall h = Hall{seats: repeat(Seat.Taken, ROWS * SEATS)}
    expect(h.book_group(1) == err(BookErr.NoBlock(1)))
    expect(h.seats.count(Seat.Free) == 0)
}

test "abschluss und befehle" {
    Hall h = Hall{}
    _ = try h.book_group(8)
    try h.book(3, 5)
    expect(h.summary() == "Tickets verkauft: 9 von 40, Auslastung: 22.5 %")
    expect(try exec(mut h, "g 2") == "Gruppe (2) gebucht: Reihe 2, Plätze 1 bis 2.")
    expect(exec(mut h, "b 3") == err(CmdErr.Missing("b")))
    expect(exec(mut h, "b 3 x") == err(CmdErr.BadNumber("x")))
    expect(exec(mut h, "b 3 5") == err(CmdErr.Book(BookErr.AlreadyTaken(3, 5))))
    expect(try exec(mut h, "hilfe") == HELP)
    expect(h.seats.count(Seat.Taken) == 11)
}
```

**Was das Beispiel zeigt.** Plätze sind ein `enum`, Fehler sind Typen, die `switch` vollständig behandeln muss, `try … or CmdErr.Book` hebt einen Buchungsfehler in den Fehlertyp des Befehls, `mut self` kennzeichnet jede Methode, die den Saal ändert, EOF beendet die Eingabeschleife (`for str line in try io.lines()` mit `void!io.IoErr main`), ein ignorierter Buchungsaufruf ist ein Compilerfehler, und das Binary soll ≤ 64 KB groß werden (gemessen: 59 784 B, `docs/measurements.md`). Das Programm steht als `examples/kino.dif`.

---

## 6. Compiler und Backend (ADR-Entwürfe)

### 6.1 Pipeline

```
.dif ─▶ Lexer ─▶ Parser ─▶ AST ─▶ Namen ─▶ Typen ─▶ Lowering (IR: RC-Ops, Checks) ─▶ C11 ─▶ cc ─▶ Binary
          ▲                                                                            │
   use c: cc -E + C-Deklarationsparser                                         .so/.dll linken, .c mitbauen
        └──────────────── dif check (ohne C-Compiler-Lauf, Ziel < 100 ms / 10k Zeilen) ─┘
```

Ausgabe in v0: **ein C-File pro Programm** (Unity-Build). cc optimiert über alles, jede Generic-Instanz entsteht genau einmal, es gibt keine erzeugten Header. Erst wenn `dif run` das Kriterium aus §6.2 reißt (> 500 ms bei typischen Programmen), kommt ein C-File pro Modul mit Cache.

### 6.2 Backend: C11 an den System-C-Compiler

| | LLVM | Cranelift | QBE | **C11 → gcc/clang** |
|---|---|---|---|---|
| Laufzeit des Ergebnisses | sehr gut | mittel (wenig Optimierung) | gut („70 % der Leistung industrieller Compiler in 10 % des Codes“) | **sehr gut** (gcc/clang `-O2`) |
| Compile-Zeit | langsam | sehr schnell | schnell | mittel, vom C-Compiler bestimmt (zu messen) |
| Binary-Größe | klein | klein | klein | **klein** |
| FFI zu C | C-ABI für structs selbst nachbauen, je Plattform | teils selbst | C-ABI inkl. structs | **exakt und gratis**, echte Header prüfen Signaturen |
| Plattformen | alle | x86_64, arm64, riscv64, s390x | amd64, arm64, riscv64 | **alles mit C-Compiler**, auch WASM (clang) und Mikrocontroller |
| Abhängigkeit des Compilers | ~100 MB, API bricht pro Version | Rust-Crates, mittel | winzig, braucht Assembler + Linker | System-`cc` (überall vorhanden; hier gcc 16.2) |
| Debugging | DWARF | begrenzt | kaum | `#line` → gdb zeigt `.dif`-Zeilen |
| Coverage/Sanitizer | eigene Arbeit | eigene Arbeit | keine | **gcov, ASan, UBSan gratis** |
| Aufwand für uns | hoch | mittel | mittel | **niedrig** |

**Entscheidung (akzeptiert): C11.** Damit liegen wir in allen Kernwünschen (Geschwindigkeit, C-Anbindung, kleine Binaries, Test-Werkzeuge) vorn, bei kleinstem Eigenaufwand. Vorbilder: Nim, Vala, Chicken Scheme, Koka, Cython.

Risiken und Gegenmittel:
- *UB im erzeugten C*: nur definierte Konstrukte erzeugen, Arithmetik über `__builtin_*_overflow`, `-fno-strict-aliasing`, alle Golden-Tests zusätzlich unter `-fsanitize=undefined,address`.
- *C-Fehler beim Nutzer*: per Definition ein Compilerbug; CI kompiliert mit `-Wall -Wextra -Werror`.
- *Abhängigkeit von `cc`*: `dif` nennt beim Fehlen klar, welcher Compiler gesucht wurde (CC-Umgebungsvariable, `gcc`, `clang`).

Wann wir neu entscheiden: Wenn `dif run` bei typischen Programmen über 500 ms liegt, kommt tcc als schneller Entwicklungs-Compiler oder QBE als zweites Backend dazu. Die IR (die wir für das Lowering der Referenzzählung ohnehin brauchen) hält diese Tür offen.

### 6.3 Compiler in Rust

- Summentypen und `match` sind ideal für AST und IR; Speichersicherheit im Compiler selbst; Agenten schreiben Rust zuverlässig; `cargo test` für die Golden-Tests. (Rust ist nur die Implementierungssprache; die Syntax von difficulty ist bewusst nicht Rust.)
- Ergebnis ist ein einzelnes statisches Binary `dif` (Ziel ≤ 5 MB), installiert ist rustc 1.99.
- Zu Beginn keine Crate-Abhängigkeiten: handgeschriebener Lexer und rekursiver Abstiegsparser (bessere Fehlermeldungen als Generatoren). Typ-vorn-Deklarationen sind eindeutig, weil eine Anweisung, die mit `Typ Name` beginnt, immer eine Deklaration ist.
- Verworfen: C (schnell, aber unsicher für einen von Agenten geschriebenen Compiler), Go (GC, größere Binaries), Zig (nicht installiert, Sprache noch instabil). Selbsthosting ist kein Ziel für v0.

### 6.4 Speichermodell und Runtime

- `str`, `[]T`, `map` sind referenzgezählt mit Copy-on-Write; `[N]T` und structs liegen inline; rekursive `enum`s werden automatisch geboxt.
- Das Lowering entfernt unnötige Zähleroperationen (Move bei letzter Nutzung, nicht entweichende Locals), angelehnt an Perceus (Koka).
- Zählung nicht atomar; Threads regelt die Nebenläufigkeit (nach v0).
- Runtime ist ein einzelner Header `rt/dif_rt.h` (≤ 1500 Zeilen C, nur libc): Allokation, RC, Panic, UTF-8-Prüfung, Float-Formatierung, geordnete Map, `dl`-Laden. Stapelüberlauf wird per Guard-Page und `sigaltstack` als Panic gemeldet, nicht als Segfault.

### 6.5 Fehlermodell

`T!E` und `?T` für erwartbare Fehler, Panics für Programmierfehler, keine Exceptions. Die Laufzeitprüfungen sind immer aktiv: Bounds, Überlauf, Division durch 0, UTF-8 an den Grenzen (Eingabe, Dateien, C).

### 6.6 C-Import

- `use c` vorverarbeitet mit dem System-`cc -E -dD` und parst nur Deklarationen (C11 + GCC-Erweiterungen, die übersprungen werden). Keine libclang-Abhängigkeit, der Compiler bleibt klein.
- Weil das Backend C ist, braucht der Import kein Layout- und kein ABI-Wissen: Felder und Aufrufe gehen im erzeugten C einfach über den Namen.
- Ergebnis pro Header und `cc`-Version gecacht, damit `dif check` schnell bleibt.
- Laufzeit-Laden über `dlopen`/`LoadLibrary`, gekapselt in `std.dl`; der Handle ist ein referenzgezählter Wert, der beim letzten Freigeben schließt.

---

## 7. Werkzeuge für Agenten

| Befehl | Zweck |
|---|---|
| `dif run f.dif [args]` | kompilieren und ausführen |
| `dif build f.dif -o out [--static]` | natives Binary |
| `dif check f.dif [--json]` | Parse + Typprüfung ohne C-Compiler-Lauf |
| `dif test [pfad] [filter] [--json] [--coverage] [--sanitize]` | Tests inkl. Doc-Tests |
| `dif fmt [--check]` | kanonische Form |
| `dif fix f.dif` | wendet maschinenlesbare Fix-its an |
| `dif doc <symbol>` | Signatur + Doku nachschlagen, auch für importierte C-Header: `dif doc sqlite3.open` |
| `dif bindgen x.h` | extern-Block aus einem C-Header |
| `dif llms` | gibt die zur Compiler-Version passende `llms.txt` aus |
| `dif explain R13` | lange Erklärung zu einem Fehlercode |

**Editoren:** Syntax-Highlighting, Dateityp-Erkennung, Einrückung und `dif fmt`-Anbindung für **Vim/Neovim** (`editors/vim/`: ftdetect, syntax, ftplugin) und **Emacs** (`editors/emacs/dif-mode.el`). Die Wortlisten kommen aus der Spec; CI prüft, dass keine Datei ein Schlüsselwort vergisst. Später: tree-sitter-Grammatik (Neovim, Emacs 29+, Helix, Zed) und LSP.

Compiler-Meldungen sind englisch (Vorwissen der Modelle, eindeutige Suchbegriffe), mit `--lang de` deutsch.

```
kino.dif:42:9: error[R13]: result of type `void!BookErr` is ignored
   42 |         hall.book(r, p)
      |         ^^^^^^^^^^^^^^^
  hint: propagate with `try`, handle with `switch`, or discard explicitly with `_ =`
```

Fehlercodes sind die Regelnummern aus §3.2: wer `R13` liest, weiß, welche Regel verletzt ist.

```json
{"v":1,"code":"R13","severity":"error","file":"kino.dif","line":42,"col":9,"end_col":24,
 "message":"result of type `void!BookErr` is ignored",
 "hint":"propagate with `try`, handle with `switch`, or discard explicitly with `_ =`",
 "fix":[{"line":42,"col":9,"insert":"try "}]}
```

```
$ dif doc str.split
[]str str.split(str sep)    // Splits at every occurrence of sep. Panics if sep is empty.
```

Diagnosen für typisches Vorwissen greifen vor dem generischen Parserfehler, viele mit Fix-it:

| Gewohnheit | Meldung / Fix |
|---|---|
| `var x = 1`, `auto x = 1`, `let x = 1` | Typ ausschreiben; der Compiler kennt ihn und setzt ihn per `dif fix` ein: `int x = 1` |
| `for (int i = 0; i < n; i++)` | `for int i in range(n)` |
| `i++` | `i += 1` |
| `break` im `case` | kein Fallthrough, darum unnötig; wer die Schleife verlassen wollte: `return` (R11) |
| `shift(p, 5)`, obwohl `shift` `p` ändert | `shift(mut p, 5)` per `dif fix` (R6) |
| `print(mut p)`, obwohl `print` nichts ändert | `mut` entfernen (R6) |
| `self` oder `this` in der Parameterliste | nur `mut self` als erster Parameter; sonst entfernen, `self` ist in jeder Methode da (R7) |
| `p->x` | `p.x` |
| `null`, `nil`, `None` | `?T` und `none` |
| `elif` | `else if` |
| `def`, `fn`, `function` | Rückgabetyp vorn: `int add(int a, int b)` |
| `/* */` | `//` |
| `.Taken` | `Seat.Taken` |
| `length`, `size`, `len(x)` | `x.len()` |
| `E!T` (Zig), `T?` (Swift) | `T!E`, `?T` |
| `int(s)` auf Text | `s.parse_int()` |

---

## 8. Typische Fallen für Agenten und wie difficulty sie vermeidet

| Falle | Lösung |
|---|---|
| Sprache ist nicht im Training | C/Python-vertraute Syntax; `llms.txt` ≤ 24 KB; `dif llms` liefert genau die passende Version |
| Mehrdeutige Argumentgrenzen | Aufrufe immer `f(a, b)` |
| Unklarer Kontrollfluss | `if`, `return`, `break`, `continue` wie in C |
| EOF als leerer String → Endlosschleife | `for str line in io.lines()`, `read_line() -> ?str` |
| Zahlen als Wahrheitswerte | `bool` und `enum`, Bedingungen nur `bool` |
| Ganzzahlen als Floats | echte Ganzzahltypen; `parse_int() -> int!ParseErr` |
| Änderung landet still in einer Kopie | `mut`-Parameter; nie gelesene Änderung ist ein Fehler |
| Liste neu bauen, um ein Element zu ändern | `xs[i] = v`, Copy-on-Write |
| Ergebnis still verworfen | unbenutztes Ergebnis = Compilerfehler |
| Fehler erst beim Ausführen | `dif check`, statische Typen |
| Tests, die nie scheitern | `expect` vergleicht Werte; `panics` prüft nur die Meldung |
| API-Namen raten | `dif doc` (auch für C-Header), einheitliches Benennungsschema, Did-you-mean mit Alias-Tabelle |
| Typ einer Variable erraten | jede Deklaration nennt ihren Typ |
| Unklare Präzedenz | Bit-Ops über Vergleichen, keine Vergleichsketten, `&&`/`||`-Mischung nur mit Klammern |
| Nichtdeterministische Ausgabe | Maps in Einfügereihenfolge, feste Float-Ausgabe, kein Hash-Randomizing |
| Stille Kürzung bei Konvertierungen | verlustbehaftete Konvertierung nur über benannte Funktionen |
| Shadowing-Verwechslungen | kein Shadowing |
| Folgefehler-Lawinen | höchstens ein Folgefehler pro Ursache |
| Doku driftet von der Implementierung weg | Doc-Tests für `///`, `llms.txt` und Spec |
| Doku passt nicht zur installierten Version | `dif llms`, `dif version` nennt die Spec-Version |
| Lifetime-/Borrow-Checker-Schleifen (Rust) | Wertsemantik statt Referenzen |
| C-Bibliothek nutzen heißt Prototypen abtippen | `use c "x.h"` |
| Absturz in C-Code reißt die ganze Testsuite mit | jeder Test in eigenem Prozess |
| Lange Edit-Compile-Schleifen | `dif check` < 100 ms, `dif fix` |
| Undurchsichtige Segfaults | Panics mit `.dif`-Position, erkannter Stapelüberlauf, `--sanitize` |
| Formatierungs-Diffs | `dif fmt` |

---

## 9. Nicht-Ziele

Makros · Klassen, Vererbung, Objekte · Exceptions · Operator-Überladung (v0) · implizite Konvertierungen · Typinferenz für Variablen · GC · veränderliche globale Variablen · `null` · undefiniertes Verhalten im sicheren Code · Interpreter/REPL (v0) · Selbsthosting (v0) · C++-Header

---

## 10. Fahrplan

**Zielwerte** (gemessen ab M2)

| Größe | Ziel |
|---|---|
| Hello-World-Binary (x86_64, dynamisch gegen libc, gestrippt) | ≤ 16 KB |
| `kino`-Binary | ≤ 64 KB |
| fib(30) | ≤ 1,5× C `-O2` |
| 1 000 000 × `push` | < 50 ms |
| `dif check`, 10 000 Zeilen | < 100 ms |
| `dif run hello.dif` (gcc) | < 300 ms |
| Compiler-Binary `dif` | ≤ 5 MB |
| `llms.txt` | ≤ 24 KB |
| Alltags-Benchmark (Listen, Strings, Maps: Wortzählung über 10 MB Text) | ≤ 2× C, ≤ ½ Python |
| Agenten-pass@1 im Aufgabenkatalog | Ziel ≥ Python |

**Versionen und Meilensteine** (Kern zuerst)

| Version | Meilenstein | Inhalt |
|---|---|---|
| v0 | M0 Spec | Spezifikation entlang R1–R31, ADRs akzeptiert |
| v0 | M1 „Hallo, nativ“ ✓ | `int`, `bool`, `str`-Literale, Funktionen, `const`, `if`/`while`/`for`, `print` → hello und fib als native Binaries, Größe und Laufzeit gemessen (`docs/measurements.md`) |
| v0 | M2 „Kino“ | in fünf Scheiben, jede lauffähig: **a** `struct`, Methoden, `mut`, `enum`, `switch`, `if` als Ausdruck · **b** `?T`, `T!E`, `try`, `??`, `test`/`expect`/`dif test` · **c** Runtime mit RC und Copy-on-Write, `str`, `rune`, feste Ganzzahlbreiten, `f64`, `[]T`, `[N]T`, `map`, `io`, `os`, Formatangaben · **d** Funktionsliterale an Aufrufen und Block-Argument (R26) und die allgemeine std v0 (Text, Zahlen, Listen mit `filter`/`map`/`sort_by`/`any`/`all`, Maps, Dateien, Argumente), eingebaut vom Compiler, bevor es eigene Generics gibt · **e** `kino.dif` aus §5 läuft, Tests grün; **erste Agenten-Messung** (Haiku und Sonnet, danach Opus) |
| v0 | M3 „Ausdruck“ | benannte Parameter, Funktionswerte als eigene Werte, eigene Generics, std in difficulty selbst geschrieben → Aufgabenliste aus §4.11 ohne UI läuft |
| v0 | M4 „Werkzeug-Kern“ | `extern` und Linken, `defer`, `dif check --json`, `fmt`, `fix` |
| v0.1 | M5 „C-Nachbar“ | `use c`, `bindgen`, Laufzeit-Laden, C-Quellen, Callbacks mit Kontext |
| v0.2 | M6 „Werkzeugkasten“ | Coverage, Sanitizer, Doc-Tests, Prozess-Isolation, `doc`, `llms`, `explain` |
| v0.3 | M7 „UI“ | rekta-Modul, Editoren, `--lang de` |
| v0.4 | – | Export als `.so`/`.dll`, JSON |
| v1.x | – | Nebenläufigkeit, Benchmarks in der Sprache |

**Agenten-Messung** (ab M2, bei jedem Meilenstein): Katalog aus Kino, Wortzählung, Zustandsautomat, Textparser, Datei mit Fehlern, ab v0.1 eine C-Bibliothek. Gemessen werden pass@1 (erster Versuch kompiliert und Tests grün, ohne Hilfe), pass@5 (mit `dif check`/`dif test`-Schleife, höchstens 5 Runden), Runden bis grün, Tokens und Regelverstöße je Regel. Verglichen wird mit denselben Aufgaben in Python. Modelle: Claude Opus, Sonnet, Haiku und ein lokales Modell (Ollama); Kontext nur `llms.txt` und die Aufgabe. Häufige Regelverstöße führen zu besseren Diagnosen oder zu einer Regeländerung.

Repo-Layout, sobald Code entsteht (nicht vorab anlegen): `src/` (Compiler-Crate), `rt/dif_rt.h`, `std/*.dif`, `tests/ok`, `tests/err`, `docs/spec.md`, `docs/llms.txt`.

---

## 11. Entscheidungen

| Frage | Entscheidung |
|---|---|
| Leitbild | Python-Einfachheit, C-Klarheit; keine Rust-Symbolik, kein Java-Boilerplate; keine Objekte, aber Funktionen an structs |
| Signaturen | C-Stil, Typ vorn: `int add(int a, int b)`, struct-Felder `int x` |
| Variablen | immer expliziter Typ (`int n = 0`), `const` für Unveränderliches, keine Inferenz |
| Methoden | `Rückgabetyp Typ.name(…)`; `self` ist in jeder Methode der Ort, an dem sie aufgerufen wird; ändert sie ihn, steht `mut self` als erster Parameter |
| Übergabe | `mut` in Signatur und Aufruf (`void shift(mut Point p, int dx)`, `shift(mut p, 5)`); Parameter ohne `mut` sind lokale Kopien wie in C; eine nie gelesene Änderung ist ein Fehler (R6) |
| Methodenaufruf | `hall.book(3, 5)` ohne `mut` (auch bei `mut self`): `self` ist der Ort vor dem Punkt; ein Aufruf ohne Wirkung ist ein Fehler (R13) |
| Regeln | Design-Grundsätze und Regeln R1–R31 ohne Sonderfälle (§3); Fehlercodes = Regelnummern |
| Fehler | `int!AgeErr parse_age(str s)`, `void!BookErr`, `?str`; `try`, `err(...)` |
| Verzweigung | `switch`/`case`, kein Fallthrough, vollständig |
| Schleifen | `for int i in range(n)`, `range(a, b)`, `for T x in xs`, `while` |
| enum-Varianten | immer qualifiziert: `Seat.Taken` |
| Klammern um Bedingungen | keine (`if x > 0 {`) |
| struct-Literale | `Point{x: 1, y: 2}` |
| Komfort-Features in v0 | nur `??` und `if` als Ausdruck (keine `is`-Muster, keine Textmuster) |
| Compiler-Meldungen | Englisch, `--lang de` umschaltbar |
| Plattformen | v0 Linux x86_64 + arm64, dann macOS, später Windows/WASM |
| `int` | immer 64 Bit, Überlauf = Panic, `+%` für Wrapping |
| Float-Ausgabe | kürzeste Form, die beim Zurücklesen denselben Wert ergibt; `{x:g}` für %g |
| C-Anbindung v0 | Header-Import `use c`, Laufzeit-Laden, C-Quellen mitbauen, `dif bindgen` (plus `extern`, Linken per Name/Pfad/pkg-config) |
| Export als `.so`/`.dll` | nach v0 |
| Test-Werkzeuge v0 | Coverage, Sanitizer, Doc-Tests (plus Prozess-Isolation, `panics`-Tests); Benchmarks nach v0 |
| Nebenläufigkeit | nach v0: `spawn`/`task[T]`/`wait`, Funktion wartet auf ihre Tasks; `chan[T]`; `parallel for … yield`; Timeouts und kooperativer Abbruch; Deadlock-Erkennung; reproduzierbare Tests; kein `async`/`await` |
| Nebenläufigkeits-Runtime | erst OS-Threads, später leichte Tasks (M:N) mit gleichem Code |
| Mehrere Quellen | Inbox-Muster (`enum` + ein Kanal), kein `select`; dazu `time.every`, `time.after`, `try_recv` |
| Große Daten teilen | `shared[T]` über `share(x)`, unveränderlich, atomare Zählung nur dort (R22) |
| Abbruch und IO | blockierende std-Aufrufe kehren nach `cancel` mit Fehler zurück |
| GUI-Anbindung | `main` immer im OS-Hauptthread; `ch.fd()` als Weck-Deskriptor für C-Eventloops |
| Name | `difficulty`, Kommando `dif`, Dateien `.dif` |
| Remote | öffentliches GitHub-Repo, angelegt über `gh` (per nix-shell) |
| Lizenz | MIT |
| Callbacks mit Zustand | Kontext-Variable, gebunden an den laufenden Aufruf, in den sie mit `mut` ging; Callbacks außerhalb oder aus fremden Threads brechen ab (R24) |
| Fehler mischen | `try x or E.V(…)` übersetzt, kein Sammeltyp `error` (R12) |
| Zahl-Literale | passen sich ihrer Stelle an, auch Ausdrücke nur aus Literalen (`f64 h = 7 / 2` ist 3.5) (R8) |
| Aufräumen | `defer X`: Code wandert ans Blockende, Argumente werden dann ausgewertet; `T!E` braucht `_ =`; kein `defer` bei Abbruch (R23) |
| Optionen an Funktionen | benannte Parameter mit Default, beim Aufruf per Name (R25) |
| Funktionswerte | Literal `bool(Person p) { … }`, nur während des Aufrufs gültig, fängt Werte ein (Momentaufnahme) und ändert nie etwas ringsum (R26); keine impliziten Parameter wie `it` |
| Blöcke | Literal ohne Parameter als `{ … }` direkt hinter `)`: `ui.tiles() { … }`; Kopfzeile von `if`/`while`/`for`/`switch` endet am ersten `{` (R3, R26) |
| Generics | vollständig in v0: Typparameter `[A, B]`, Funktionstypen `R(T)`, Einschränkung vor dem Parameter `[ordered T]` (R27); Fähigkeiten automatisch nach fester Regel (R28) |
| v0-Umfang | Kern zuerst: v0 = M0–M4, danach v0.1 C-Import, v0.2 Test-Werkzeuge, v0.3 UI/Editoren, v0.4 Export/JSON, v1.x Nebenläufigkeit (§10) |
| Agenten-Messung | ab M2 bei jedem Meilenstein, festes Protokoll (§10) |
| Literale | Ausdrücke nur aus Literalen rechnen im Typ ihrer Stelle, ohne Stelle `int`/`f64` (R8) |
| Zahlen | C-Semantik streng: `/` Richtung 0, `%` mit Vorzeichen des Dividenden, IEEE-Floats, Abbruch bei int/0 und zu weiten Shifts, links nach rechts (R29) |
| Maps | `m[k]` liefert `?V`, Literal `{k: v}`, Schleifen `for K k, V v in m` und `for int i, T x in xs` (R11, R30) |
| JSON | typisiert, nach v0 |
| C-Ausgabe | ein C-File pro Programm in v0 (§6.1) |
| Griffe in Tasks | atomar gezählt; C-Griffe nur mit `send T` beim Import (R20) |
| `mut` | nimmt Orte (Variable, Feld, Element), Überlappung = Fehler bzw. Abbruch (R6) |
| UI-Muster | Elm-Architektur: Nachrichten + `update(mut App, Msg)` + `view(App, ui.Builder)`; Builder als Parameter und Griff, kein globaler Zustand |
| C-Sicherheit | sichere Standards: Parameter nicht optional, Strings nur für den Aufruf, Griffe mit Freigeber gezählt, NUL im String bricht ab (R17) |
| Tasks beim Verlassen | erst abbrechen, dann abwarten; `continue` im `switch` erlaubt, `break` nicht (R11, R19) |
| Schleifen-Kopien | Regel „nie gelesene Änderung ist ein Fehler“ statt `const`-Schleifenvariablen (R6) |
| Editoren | Syntax-Highlighting für Vim/Neovim und Emacs, später tree-sitter und LSP |
| C-Import-Details | C-Pointer sind `?T`, opake Pointer sind Griffe, C-struct-Felder ohne Angabe sind 0, `c.sizeof` (R17) |
| Backend | C11 an gcc/clang (§6.2) |
| Compiler-Sprache | Rust, anfangs ohne Crates (§6.3) |
| Speicher | Wertsemantik + Referenzzählung + Copy-on-Write, kein GC (§6.4) |
| C-Import-Technik | `cc -E` + eigener C-Deklarationsparser, kein libclang (§6.6) |
