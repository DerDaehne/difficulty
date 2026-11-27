#!/bin/sh
# Builds, checks output equality, measures (median of 5, wall clock). Needs cargo, cc, python3 on PATH.
#   nix develop -c nix shell nixpkgs#python3 -c bench/run.sh
set -e
cd "$(dirname "$0")/.."
DIF=${DIF:-target/release/dif}
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
$DIF build bench/wc.dif -o "$T/wc_dif"
$DIF build bench/push.dif -o "$T/push_dif"
cc -std=c11 -O2 -o "$T/wc_c" bench/wc.c
python3 bench/gen.py 10000000 > "$T/text.txt"
echo "text: $(stat -c %s "$T/text.txt") bytes"

# median wall time in ms of 5 runs: median CMD [stdin-file]
median() {
    in=${2:-/dev/null}
    for i in 1 2 3 4 5; do
        s=$(date +%s%N); sh -c "$1" < "$in" > /dev/null; e=$(date +%s%N)
        echo $(( (e - s) / 1000 ))
    done | sort -n | sed -n 3p | awk '{printf "%.1f", $1 / 1000}'
}

"$T/wc_dif" < "$T/text.txt" > "$T/o_dif"
"$T/wc_c" < "$T/text.txt" > "$T/o_c"
python3 bench/wc.py < "$T/text.txt" > "$T/o_py"
cmp "$T/o_dif" "$T/o_c"; cmp "$T/o_c" "$T/o_py"
echo "outputs identical ($(wc -l < "$T/o_c") lines)"

cat "$T/text.txt" > /dev/null
p=$(median "$T/push_dif")
d=$(median "$T/wc_dif" "$T/text.txt")
c=$(median "$T/wc_c" "$T/text.txt")
y=$(median "python3 bench/wc.py" "$T/text.txt")
echo "push 1e6 (dif): $p ms"
echo "wordcount dif $d ms, C $c ms, Python $y ms"
awk -v d="$d" -v c="$c" -v y="$y" 'BEGIN{printf "dif/C %.2f  dif/Python %.2f\n", d/c, d/y}'
