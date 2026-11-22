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
| fib(38) gegen C `-O2` | ≤ 1,5× | 131 ms gegen 40 ms = 3,3× | nein |
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

### fib: warum 3,3×

Die C-Fassung rechnet ohne Überlaufprüfung; gcc formt die Rekursion dort teilweise in eine
Schleife um. In difficulty prüft jedes `+` und `-` auf Überlauf (R14), und diese Prüfungen
verhindern die Umformung. Ein fairer Vergleich mit derselben Semantik (C mit
`__builtin_add_overflow`) und mögliche Abhilfe — Wertebereich-Analyse, die Prüfungen
wie bei `n - 1` mit `n ≥ 2` weglässt — gehören zu den nächsten Schritten des Backends.
Das Ziel gilt laut PLAN.md ab M2.
