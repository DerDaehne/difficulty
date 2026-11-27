# Benchmarks

`run.sh` builds the three word-count programs (`wc.dif`, `wc.c`, `wc.py`) and `push.dif`,
generates a deterministic 10 MB text with `gen.py` (into a temp directory, never in the
repo), checks that the three outputs are byte-identical, and prints the median of 5 wall-clock runs.

```sh
nix develop -c nix shell nixpkgs#python3 -c bench/run.sh
```

A word is a run of ASCII letters, lower-cased; the top 20 are printed by count descending,
then alphabetically, followed by `total N distinct M`.
