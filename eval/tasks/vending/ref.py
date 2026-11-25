import sys

PRICE = {"A": 60, "B": 120, "C": 150}
credit = strikes = 0
locked = False

for line in sys.stdin:
    w = line.strip(" \n").split(" ")
    if w == [""]:
        continue
    c, n = w[0], len(w)
    if c == "unlock" and n == 1:
        if locked:
            locked, strikes = False, 0
            print("unlocked")
        else:
            print("error: not locked")
    elif c == "status" and n == 1:
        print(f"state {'locked' if locked else 'active' if credit > 0 else 'idle'} credit {credit}")
    elif c in ("coin", "select") and n == 2 or c == "refund" and n == 1:
        if locked:
            print("error: locked")
        elif c == "coin":
            if w[1] in ("10", "20", "50", "100"):
                credit += int(w[1])
                print(f"credit {credit}")
            else:
                print("rejected")
                strikes += 1
                if strikes == 3:
                    locked = True
                    print("locked")
        elif c == "refund":
            print(f"refund {credit}")
            credit = 0
        elif w[1] not in PRICE:
            print(f"error: unknown item {w[1]}")
        elif credit < PRICE[w[1]]:
            print(f"error: need {PRICE[w[1]] - credit} more")
        else:
            print(f"vend {w[1]} change {credit - PRICE[w[1]]}")
            credit = 0
    else:
        print("error: bad command")
