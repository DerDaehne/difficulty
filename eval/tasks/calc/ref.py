import sys


def calc(s):
    """Return the value, or an error message."""
    pos = 0
    zero = False  # a division by zero happened; reported only if the syntax is fine

    def peek():
        nonlocal pos
        while pos < len(s) and s[pos] == " ":
            pos += 1
        return s[pos] if pos < len(s) else ""

    class Syntax(Exception):
        pass

    def fail():
        peek()
        raise Syntax

    def expr():
        v = term()
        while peek() in ("+", "-"):
            op = s[pos]
            advance()
            r = term()
            v = v + r if op == "+" else v - r
        return v

    def advance():
        nonlocal pos
        pos += 1

    def term():
        nonlocal zero
        v = unary()
        while peek() in ("*", "/", "%"):
            op = s[pos]
            advance()
            r = unary()
            if op == "*":
                v *= r
            elif r == 0:
                zero, v = True, 0
            else:
                q = abs(v) // abs(r) * (1 if (v < 0) == (r < 0) else -1)
                v = q if op == "/" else v - q * r
        return v

    def unary():
        nonlocal pos
        c = peek()
        if c == "-":
            advance()
            return -unary()
        if c == "(":
            advance()
            v = expr()
            if peek() != ")":
                fail()
            advance()
            return v
        if c in "0123456789" and c:
            start = pos
            while pos < len(s) and s[pos] in "0123456789":
                pos += 1
            return int(s[start:pos])
        fail()

    try:
        v = expr()
        if peek() != "":
            fail()
    except Syntax:
        return f"error: syntax at {pos + 1}"
    return "error: division by zero" if zero else str(v)


for line in sys.stdin:
    line = line.rstrip("\n")
    if line.strip(" ") != "":
        print(calc(line))
