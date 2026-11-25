import re
import sys

ROWS, SEATS = 5, 8
taken = [[False] * SEATS for _ in range(ROWS)]


def num(w):
    return int(w) if re.fullmatch(r"-?[0-9]+", w) else None


for line in sys.stdin:
    w = line.strip(" ").rstrip("\n").strip(" ").split(" ")
    if w == [""]:
        continue
    if w == ["q"]:
        break
    c, args = w[0], [num(x) for x in w[1:]]
    if c in ("b", "s") and len(args) == 2 and None not in args:
        r, s = args
        if not (1 <= r <= ROWS and 1 <= s <= SEATS):
            print(f"error: no such seat {r} {s}")
        elif c == "b":
            if taken[r - 1][s - 1]:
                print(f"error: seat {r} {s} is taken")
            else:
                taken[r - 1][s - 1] = True
                print(f"booked {r} {s}")
        elif not taken[r - 1][s - 1]:
            print(f"error: seat {r} {s} is not booked")
        else:
            taken[r - 1][s - 1] = False
            print(f"cancelled {r} {s}")
    elif c == "g" and len(args) == 1 and args[0] is not None:
        n = args[0]
        if not 1 <= n <= SEATS:
            print(f"error: bad group size {n}")
            continue
        for r in range(ROWS):
            p = next((p for p in range(SEATS - n + 1) if not any(taken[r][p:p + n])), None)
            if p is not None:
                taken[r][p:p + n] = [True] * n
                print(f"group {n} booked: row {r + 1}, seats {p + 1} to {p + n}")
                break
        else:
            print(f"error: no block of {n} free seats")
    elif c == "p" and len(args) == 0:
        print("  12345678")
        for r in range(ROWS):
            print(f"{r + 1} " + "".join("X" if t else "." for t in taken[r]))
    elif c == "t" and len(args) == 0:
        sold = sum(map(sum, taken))
        print(f"sold {sold} of {ROWS * SEATS}, occupancy {sold * 100 / (ROWS * SEATS):.1f}%")
    else:
        print("error: bad command")
