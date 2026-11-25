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
| fib(38) gegen C `-O2` (ohne Prüfung) | ≤ 1,5× | gcc: 1 ms gegen 38 ms; clang: 122 ms gegen 82 ms = 1,5× | ja |
| `kino`-Binary, `push`, Alltags-Benchmark, `llms.txt`, Agenten-pass@1 | siehe PLAN.md | noch nicht messbar | – |

Befehle:

```sh
dif build examples/hello.dif -o hello && stat -c %s hello
dif build fib.dif -o fib_dif                  # fib(38), rekursiv wie examples/fib.dif
gcc -std=c11 -O2 -s -o fib_c fib.c            # dieselbe Funktion in C, int64_t
time ./fib_dif; time ./fib_c
time dif run examples/hello.dif
time dif check big.dif                        # 1 000 generierte Funktionen à 11 Zeilen
```

fib(38) statt fib(30), weil fib(30) unter 10 ms bleibt und sich so nicht vergleichen lässt.

### fib: Ursache und Abhilfe

Vorher: 131 ms gegen 40 ms (3,3×). Das erzeugte C prüft jedes `+` und `-` (R14) und
legt wegen R29 die Ergebnisse beider Aufrufe in Temporäre. Drei C-Fassungen derselben
Funktion, gcc 16.2.0 `-O2`, `perf stat` (Instruktionen, Wanduhr, fib(38), `n` zur Laufzeit):

| Fassung | Instruktionen | Zeit |
|---|---|---|
| C ohne Prüfung | 919 M | 37 ms |
| C mit `__builtin_*_overflow` an `+`, `n - 1`, `n - 2`, Panic als `cold`/`noinline` | 1 392 M | 135 ms |
| C mit Prüfung nur am `+` (`n - 1`, `n - 2` nach `n < 2` bewiesen sicher) | 2,9 M | 0,5 ms |
| erzeugtes C vorher (Prüfung überall, Panic inline) | – | 126 ms |
| erzeugtes C jetzt | 2,9 M | 0,5 ms |

Ursache: nicht die Temporären und nicht der Panic-Pfad (das handgeschriebene C mit
kaltem Panic ist genauso langsam wie das erzeugte), sondern die beiden Prüfungen an
`n - 1` und `n - 2`. Sie stehen zwischen den rekursiven Aufrufen und lassen gcc die
Rekursion nicht mehr umformen. Hinter `if n < 2 { return … }` ist `n ≥ 2`, die
Subtraktionen können nicht überlaufen.

Abhilfe im Backend (`emit_c.rs`):

- `dif_panic` ist `noinline, cold`: die Prüfungen bleiben ein Vergleich und ein Sprung
  auf eine kalte Stelle.
- Kleine Wertebereichsregel: nach `if p < K { …; return … }` (ohne `else`) gilt im Rest
  des Blocks `p ≥ K`, für `int`-Parameter ohne `mut`, die die Funktion nie ändert (keine
  Zuweisung, kein `mut`-Argument). `p - c` mit Literal `c ≥ 0` und darstellbarem `K - c`
  wird dann ohne Prüfung erzeugt. Alles andere bleibt geprüft; Panics, Positionen und
  Auswertungsreihenfolge sind unverändert (`range_facts_keep_checks` in `tests/backend.rs`).

Der gcc-Wert (0,5 ms) ist kein fairer Wert für „Rechenleistung“: gcc formt die Rekursion
so um, dass kaum noch Aufrufe bleiben; ohne Prüfung gelingt ihm das nicht (37 ms). Als
vergleichbarer Wert dient clang, der diese Umformung nicht macht: C ohne Prüfung 82 ms,
erzeugtes C 122 ms = 1,5× (clang 21, `-O2`). Das Ziel PLAN §10 ist damit erreicht, bei
gcc weit unterboten.
