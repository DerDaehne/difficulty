# difficulty

Kompilierte Programmiersprache für KI-Agenten: C-artige Syntax, statische Typen, Summentypen,
Wertsemantik ohne GC, eingebaute Tests, FFI zu C, native Binaries über ein C11-Backend.

Status: erste Spezifikation; der Compiler `dif` prüft bisher die Syntax (`dif check`).
Idee, Grundsätze, Regeln, Beispielcode und Fahrplan: [PLAN.md](PLAN.md).
Verbindliche Sprachbeschreibung: [docs/spec.md](docs/spec.md).

## Entwickeln

```sh
nix develop          # rustc, cargo, clippy, rustfmt, gcc
cargo test
cargo run -- check examples/fib.dif
```

Lizenz: [MIT](LICENSE).
