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
| `push`, Alltags-Benchmark, `llms.txt`, Agenten-pass@1 | siehe PLAN.md | noch nicht messbar | – |

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
