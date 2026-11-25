# difficulty

Kompilierte Programmiersprache für KI-Agenten: C-artige Syntax, statische Typen, Summentypen,
Wertsemantik ohne GC, eingebaute Tests, FFI zu C, native Binaries über ein C11-Backend.

Status: erster Sprachumfang („Hallo, nativ“): `dif check` prüft Syntax, Namen und Typen; `dif run` und `dif build` übersetzen über C11 in ein natives Programm.
Idee, Grundsätze, Regeln, Beispielcode und Fahrplan: [PLAN.md](PLAN.md).
Verbindliche Sprachbeschreibung: [docs/spec.md](docs/spec.md).

## Entwickeln

```sh
nix develop          # rustc, cargo, clippy, rustfmt, gcc
cargo test
cargo run -- run examples/fib.dif
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

Lizenz: [MIT](LICENSE).

## Agenten-Messung

`eval/` enthält das Messgerüst: Aufgaben, versteckte Tests, Python-Referenzen und den Runner. Siehe [eval/README.md](eval/README.md).
