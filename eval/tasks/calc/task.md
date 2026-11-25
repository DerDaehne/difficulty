# Expression calculator

Each line of standard input holds one integer expression. Print one output line per non-empty line: the value, or an error.

## Grammar

```
expr   = term { ("+" | "-") term }
term   = unary { ("*" | "/" | "%") unary }
unary  = "-" unary | "(" expr ")" | number
number = one or more ASCII digits
```

Spaces may appear between tokens (and at the start and end of the line) but not inside a number. Lines that are empty or contain only spaces are skipped. Operators are left-associative. Values and intermediate results fit in a signed 64-bit integer in all tests.

`/` truncates toward zero (`-7 / 2` is `-3`). `%` is the remainder with the sign of the left operand (`-7 % 2` is `-1`).

## Output

- Valid expression: its value in decimal.
- Not a valid expression: `error: syntax at COL`, where COL is the column (counting from 1, spaces included) of the first character at which the line stops being a valid expression: the first character that cannot continue it, or one past the last character of the line if it ends too early. Syntax errors take precedence over division errors.
- Valid expression whose evaluation divides or takes a remainder by zero: `error: division by zero`.

## Example

Input:

```
1 + 2 * 3
(1 + 2) * 3
-7 / 2
10 / (5 - 5)
2 * (3 + 
4 $ 4
```

Output:

```
7
9
-3
error: division by zero
error: syntax at 10
error: syntax at 3
```
