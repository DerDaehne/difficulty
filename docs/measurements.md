# Messungen

Messwerte gegen die Zielwerte aus [PLAN.md §10](../PLAN.md). Jede Messung nennt Stand,
Rechner und Befehl, damit sie wiederholbar ist.

## Stand „Hallo, nativ“ (erster Sprachumfang)

Rechner: x86_64, AMD Ryzen 7 7800X3D, Linux, gcc 16.2.0, `dif` als Release-Build
(`cargo build --release`). Zeiten: Wanduhr, Bestwert aus drei Läufen.

| Größe | Ziel | Gemessen | Erfüllt |
|---|---|---|---|
| Hello-World-Binary (x86_64, dynamisch gegen libc, gestrippt) | ≤ 16 KB | 14 352 B (14,0 KB) | ja |
| `dif check`, 10 000 Zeilen | < 100 ms | 10 ms (11 003 Zeilen, 1 000 Funktionen) | ja |
| `dif run hello.dif` (gcc) | < 300 ms | 46 ms | ja |
| Compiler-Binary `dif` | ≤ 5 MB | 680 912 B (0,65 MB) | ja |
| fib(38) gegen C `-O2` (ohne Prüfung) | ≤ 1,5× | gcc 16.2: 2 ms gegen 37 ms (Rekursionsumformung); clang 21: 140–154 ms gegen 84–93 ms = 1,6–1,8× | gcc ja, clang nein |
| `kino`-Binary (`examples/kino.dif`, x86_64, dynamisch gegen libc, gestrippt) | ≤ 64 KB | 59 784 B (58,4 KB) | ja |
| `llms.txt` | ≤ 24 KB | 21214 B | ja |
| Agenten-pass@1 (Abschnitt „Agenten-Messung 1“) | ≥ Python | Haiku 4/5 gegen 4/5, Sonnet 4/5 gegen 5/5 | Haiku ja, Sonnet nein |
| 1 000 000 × `push` auf `[]int` | < 50 ms | 7,2 ms (Abschnitt „M2-Zielwerte“) | ja |
| Alltags-Benchmark: Wortzählung über 10 MB | ≤ 2× C, ≤ ½ Python | 4,5× C, 0,45× Python (121 ms gegen 27 ms und 270 ms) | ½ Python ja, 2× C nein |

Befehle:

```sh
dif build examples/hello.dif -o hello && stat -c %s hello
dif build examples/kino.dif -o kino && strip -s kino && stat -c %s kino
dif build fib.dif -o fib_dif                  # fib(38), rekursiv wie examples/fib.dif
gcc -std=c11 -O2 -s -o fib_c fib.c            # dieselbe Funktion in C, int64_t
time ./fib_dif; time ./fib_c
time dif run examples/hello.dif
time dif check big.dif                        # 1 000 generierte Funktionen à 11 Zeilen
```

fib(38) statt fib(30), weil fib(30) unter 10 ms bleibt und sich so nicht vergleichen lässt.

### fib: Ursache und Abhilfe

Vorher: 131 ms gegen 40 ms (3,3×). Das erzeugte C prüft jedes `+` und `-` (R14) und
legt wegen R29 die Ergebnisse beider Aufrufe in Temporäre. Gemessen wird fib(38) mit `n`
als Laufzeit-Eingabe (`argv`, nicht als Konstante, die der Compiler vorab rechnen
könnte), `-O2`, Wanduhr, kleinster und größter Wert aus 7 Läufen, ms:

| Fassung | gcc 16.2 | clang 21 |
|---|---|---|
| C ohne Prüfung | 37–72 | 84–93 |
| C mit `__builtin_*_overflow` an `+`, `n - 1`, `n - 2`, Panic `cold`/`noinline` | 124–141 | 127–150 |
| C mit Prüfung nur am `+` (`n - c` ungeprüft) | 2–2 | 124–142 |
| erzeugtes C vorher (alles geprüft) | 127–139 | 139–148 |
| erzeugtes C jetzt | 2–2 | 140–154 |

Ursache unter gcc: nicht die Temporären und nicht der Panic-Pfad (handgeschriebenes C
mit kaltem Panic ist so langsam wie das erzeugte), sondern die Prüfungen an `n - 1` und
`n - 2` zwischen den rekursiven Aufrufen; sie lassen gcc die Rekursion nicht umformen.
Hinter `if n < 2 { return … }` ist `n ≥ 2`, die Subtraktionen können nicht überlaufen.

Abhilfe im Backend (`emit_c.rs`):

- `dif_panic` ist `noinline, cold`.
- Kleine Wertebereichsregel: nach `if p < K { …; return … }` (ohne `else`) gilt im Rest
  des Blocks `p ≥ K`, für `int`-Parameter ohne `mut`, die die Funktion nie ändert (keine
  Zuweisung, kein `mut`-Argument; die Suche kennt jede Ausdrucksart ausdrücklich, eine
  neue erzwingt einen Compilerfehler). `p - c` mit Literal `c ≥ 0` und darstellbarem
  `K - c` wird ohne Prüfung erzeugt. Alles andere bleibt geprüft; Panics, Positionen und
  Auswertungsreihenfolge sind unverändert (`range_facts_*` in `tests/backend.rs`).

Ergebnis: Das Ziel ≤ 1,5× ist mit gcc erreicht (2 ms gegen 37 ms), aber mustergebunden:
gcc formt die Rekursion nur ohne die `n - c`-Prüfungen um und rechnet dann kaum noch
Aufrufe; der Wert sagt nichts über allgemeine Rechenleistung. Mit clang ist es nicht
erreicht: 1,6–1,8× gegen C ohne Prüfung, und die Änderung bringt dort nichts, weil der
Verlust an den Prüfungen am `+` und den Aufrufen selbst liegt (auch C mit nur einer
Prüfung am `+` braucht 124–142 ms). Fairer Vergleich mit gleicher Semantik: C mit
`__builtin_*_overflow` überall liegt bei clang mit 127–150 ms auf dem Niveau des
erzeugten C. Weiter wäre nur eine allgemeine Wertebereichsanalyse möglich (außerhalb
dieses Tickets).

## Stand „Sammlungen“ (Teil 2c, Runtime `rt/dif_rt.h` eingebunden)

Rechner: x86_64, AMD Ryzen 7 7800X3D, Linux, gcc 16.2.0, `dif` als Release-Build
(`cargo build --release`), erzeugtes C mit `-O2 -s`. Zeiten: Wanduhr eines ganzen
Prozesses (mit Start), Bestwert aus sieben Läufen.

| Größe | Ziel | Gemessen | Erfüllt |
|---|---|---|---|
| Hello-World-Binary (gestrippt) mit eingebundener Runtime | ≤ 16 KB | 14 448 B (14,1 KB; vorher 14 352 B) | ja |
| 1 000 000 × `push` in dif (`xs.push(i)` in einer Schleife) | < 50 ms | 5,2 ms (5,2–6,9 ms über sieben Läufe) | ja |
| dasselbe Programm als Binary | – | 14 504 B | – |
| Binary mit Text-, Listen- und Map-Methoden (`tests/ok/24_text.dif`, `23_maps.dif`) | – | 63 816 B, 59 712 B | – |
| `dif run examples/hello.dif` (gcc) | < 300 ms | 74 ms (vorher 46 ms: das C-File trägt jetzt die Runtime) | ja |
| `rt/dif_rt.h` | ≤ 1500 Zeilen | 1497 Zeilen (plus `rt/dif_unicode.h`, erzeugt) | ja |

Die Runtime besteht nur aus `static inline`-Funktionen und `static const`-Tabellen; was
ein Programm nicht aufruft, erzeugt der C-Compiler nicht (die Unicode-Tabellen und die
Deskriptoren fallen mit weg). Darum bläht sie das Hello-World-Binary nicht auf: das
Binary wächst nur um 96 B (gemessen gegen den Stand „Hallo, nativ“).

Befehle:

```sh
dif build examples/hello.dif -o hello && stat -c %s hello
dif build push.dif -o push                    # 1 000 000 × xs.push(i), dann print(xs.len())
for i in 1 2 3 4 5 6 7; do s=$(date +%s%N); ./push > /dev/null; e=$(date +%s%N); echo $(( (e-s)/1000 )) us; done
```

`tests/run2c.rs` prüft die Grenze der Push-Messung bei jedem Testlauf (hier 10× großzügiger:
ein Lauf unter 50 ms gilt).

## Kopien sparen: Last-Use-Move

Rechner und Werkzeug wie oben (gcc 16.2, `dif` als Release-Build, Aufruf `dif build`, Wanduhr des
Programms einschließlich Start und `repeat`, fünf Läufe, ms). Programm: 1000 Aufrufe auf einer
Liste mit 1 000 000 `int`; `bump` ändert ein Element der übergebenen Liste und gibt sie zurück,
`bump_mut(mut xs, k)` ändert sie am Ort.

```dif
[]int bump([]int xs, int k) {
    xs[k] = xs[k] + 1
    return xs
}

void main() {
    []int xs = repeat(0, 1000000)
    for int i in range(1000) {
        xs = bump(xs, i)       // oder: bump_mut(mut xs, i)
    }
}
```

| Aufruf | vorher (02f3584) | nachher | Ziel |
|---|---|---|---|
| `xs = bump(xs, i)` | 155–161 | 5,0–6,6 | < 20 |
| `bump_mut(mut xs, i)` | 5,2–6,4 | 5,1–6,4 | – |

Ursache vorher: der Argumentwert war eine Kopie (ein Zähler mehr), also kopierte das erste
Schreiben in `bump` 8 MB; 1000 Aufrufe sind 8 GB Speicherverkehr. Jetzt ist die letzte Benutzung
der Variablen ein Move (`DIF_MOVE`), die Liste ist ungeteilt, und das Schreiben geschieht am Ort:
`bump` ist so schnell wie `bump_mut`. Der Rest der 5 ms ist Prozessstart und `repeat`. Gibt der
Aufrufer die Liste weiter, die er danach noch liest, kopiert `bump` weiter (`dif check --hints`
nennt den Parameter). Der Test `tests/copies.rs` hält beides fest (Bewegung unter 40 ms,
Kopie mehr als dreimal so langsam).

## Agenten-Messung 1 (Stand M2)

Erster Lauf des Messgerüsts aus `eval/` (Aufgaben, Protokoll und Kosten: [eval/README.md](../eval/README.md)).
Je Sprache, Modell und Aufgabe ein Lauf mit höchstens 5 Runden; der Kontext jedes Aufrufs ist nur
`llms.txt` (bei difficulty) und die Aufgabe, das Feedback nach einem Fehlversuch zeigt nie die erwartete
Ausgabe. Compiler auf dem Stand der Teile 1 bis 2d mit Standardbibliothek v0. Rohdaten:
`eval/results/2026-11-27-m2/`.

| Sprache | Modell | pass@1 | pass@5 | Runden im Mittel | Tokens (Eingabe+Cache / Ausgabe) |
|---|---|---|---|---|---|
| difficulty | Haiku | 4/5 | 5/5 | 1,2 | 64 623 / 56 115 |
| difficulty | Sonnet | 4/5 | 5/5 | 1,2 | 64 556 / 13 615 |
| Python | Haiku | 4/5 | 5/5 | 1,6 | 13 032 / 21 894 |
| Python | Sonnet | 5/5 | 5/5 | 1,0 | 6 096 / 5 883 |

Beobachtungen:

- Kein einziger difficulty-Lauf hat eine Meldung des Compilers bekommen: alle Programme übersetzten
  im ersten Versuch. Die Regelverstöße je Regel sind damit in diesem Lauf null.
- Der einzige Fehlversuch in difficulty ist bei beiden Modellen dieselbe Aufgabe (`vending`, Test
  `strikes`, erste Abweichung in Zeile 12) mit identischer Ausgabe; Haiku brauchte für dieselbe Aufgabe
  in Python vier Runden. Ursache ist der Aufgabentext: Er sagte nicht, ob `coin 010` und `coin +10`
  als 10 gelten; die Modelle nahmen 10 an, der Test erwartet `rejected`. Beide reparierten es in der
  zweiten Runde. Der Aufgabentext sagt es seit diesem Lauf ausdrücklich.
- Ziel aus PLAN §10 „pass@1 ≥ Python“: mit Haiku erreicht (4/5 gegen 4/5), mit Sonnet knapp nicht
  (4/5 gegen 5/5). Bei fünf Aufgaben ist ein Lauf der ganze Unterschied.
- Kosten: Ein difficulty-Aufruf trägt `llms.txt` mit (etwa 10 500 Eingabetokens gegen 1 200 bei Python).
  Haiku schreibt in difficulty deutlich mehr Ausgabe als Sonnet.
- Opus: nach ADR 011 erst bei stabilen Ergebnissen. Ein Lauf mit fünf Aufgaben ist das noch nicht;
  zuerst kommt ein zweiter Lauf mit Haiku und Sonnet auf dem geklärten Aufgabentext.

## M2-Zielwerte: `push` und Wortzählung

Rechner: x86_64, AMD Ryzen 7 7800X3D (16 Threads), Linux, gcc 16.2.0 (`-O2` für C, Backend von `dif`),
Python 3.14.8, `dif` als Release-Build. Zeit: Wanduhr des fertigen Programms
(`dif build`), Median aus 5 Läufen, Eingabedatei vorher gelesen (Page Cache warm).

```sh
nix develop -c nix shell nixpkgs#python3 -c bench/run.sh
```

Das Skript (`bench/`) erzeugt den Text (10 000 019 Byte, fester Seed, Zipf-artig über
5 010 Wörter, Groß-/Kleinschreibung, Satzzeichen, einige Umlaute), baut, prüft, dass die
drei Ausgaben byte-gleich sind, und misst. Ein Wort ist eine Folge ASCII-Buchstaben
(so lassen sich C und Python ohne Unicode-Tabellen gleich halten; `rune.is_letter` in
difficulty kennt Unicode); Umlaute trennen Wörter, in allen drei Programmen gleich. Es sind 1 304 206 Wörter.

| Messung | Ziel | Gemessen | Erfüllt |
|---|---|---|---|
| 1 000 000 × `push` (`bench/push.dif`, ganzes Programm samt Start) | < 50 ms | 7,2 ms | ja |
| Wortzählung dif | ≤ 2× C | 248,6 ms = 9,4× C | nein |
| Wortzählung C (C11, eigene Hash-Tabelle, `-O2`) | – | 26,4 ms | – |
| Wortzählung Python (`Counter`) | dif ≤ ½ Python | 256,5 ms; dif = 0,97× | nein |

### Wortzählung: Zeitanteile (dif)

Varianten des Programms, Wanduhr, ms (ungefähr, je 3 Läufe):

| Stufe | Zeit |
|---|---|
| nur `io.lines()` und `line.len()` | 30 |
| plus Schleife über `line.runes()` mit Vergleich | 45 |
| plus Wort zeichenweise mit `word += str(r)` aufbauen | 125 |
| plus `word.lower()` je Wort | 165 |
| vollständig (Map-Zugriffe mit `counts.add(w, 1)`, Sortieren) | 249 |

Die Zeit liegt vor allem im zeichenweisen Aufbau der Wörter (rund 80 ms), in den
Allokationen von `lower()` (rund 40 ms) und in Map-Zugriffen und Sortieren (rund
85 ms). `perf` war auf dem Rechner nicht erlaubt (`perf_event_paranoid`), die Anteile
stammen aus den Programmvarianten. Beide Ziele sind verfehlt; Folgetickets liegen vor.

### Nach dem Umbau der Strings in der Runtime

Das Anhängen am Ort (`s += t` bei nicht geteiltem Text) gab es schon; 1 000 000 × `s += "a"`
und × `s += str(r)` brauchen je 3 ms. Der Aufbau war langsam, weil jedes Wort bei Kapazität
2, 4, 8 ... neu allokierte. Geändert: kleine Texte bekommen beim ersten Wachsen gleich 32 Byte,
`s += str(r)` hängt die UTF-8-Bytes ohne Zwischentext an, `lower()`/`upper()` auf reinem ASCII
machen einen Durchgang mit einer Allokation (sonst wie bisher, Unicode-Tabellen).

| Stufe (Varianten wie oben, ms) | vorher | nachher |
|---|---|---|
| Wort zeichenweise aufbauen | 81 | 73 |
| plus `word.lower()` je Wort | 131 | 82 |
| vollständig `bench/wc.dif` | 273 | 121 |

| Messung | Ziel | Gemessen | Erfüllt |
|---|---|---|---|
| Wortzählung dif | ≤ 2× C | 121,2 ms = 4,5× C | nein |
| Wortzählung C | – | 27,0 ms | – |
| Wortzählung Python | dif ≤ ½ Python | 269,8 ms; dif = 0,45× | ja |

Es bleiben Map-Zugriffe und Sortieren (rund 40 ms) sowie Allokation je Wort.
