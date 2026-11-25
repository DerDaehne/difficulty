import re
import sys

seen = set()
ok = bad = total = 0  # total in cents
for n, line in enumerate(sys.stdin, 1):
    line = line.rstrip("\n")
    if line.strip(" \t\r") == "" or line[0] == "#":
        continue
    f = [x.strip(" \t\r") for x in line.split(",")]
    if len(f) != 3:
        why = "expected 3 fields"
    elif not re.fullmatch(r"[0-9]{1,9}", f[0]) or int(f[0]) < 1:
        why = "bad id"
    elif f[1] == "":
        why = "empty name"
    elif not re.fullmatch(r"-?[0-9]+(\.[0-9]{1,2})?", f[2]):
        why = "bad amount"
    elif int(f[0]) in seen:
        why = "duplicate id"
    else:
        why = None
    if why:
        bad += 1
        print(f"line {n}: {why}")
        continue
    seen.add(int(f[0]))
    ok += 1
    whole, _, frac = f[2].partition(".")
    c = abs(int(whole)) * 100 + int((frac + "0")[:2]) if frac else abs(int(whole)) * 100
    total += -c if f[2].startswith("-") else c
sign = "-" if total < 0 else ""
print(f"ok {ok}, bad {bad}, total {sign}{abs(total) // 100}.{abs(total) % 100:02d}")
