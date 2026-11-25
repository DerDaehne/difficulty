# Cinema

Write a program that manages the seats of a single cinema hall and reads commands from standard input, one per line.

The hall has 5 rows and 8 seats per row. Rows and seats are numbered from 1. All seats start free.

## Commands

Leading and trailing spaces of a line are ignored, empty lines are skipped. Arguments are separated by single spaces.

| Command | Meaning | Output on success |
|---|---|---|
| `b R S` | book seat S in row R | `booked R S` |
| `s R S` | cancel the booking of seat S in row R | `cancelled R S` |
| `g N` | book a group of N seats next to each other in one row: the lowest row first, within it the leftmost block of N free seats | `group N booked: row R, seats A to B` |
| `p` | print the seating plan | see below |
| `t` | print the day total | `sold S of 40, occupancy P%` (P with one decimal) |
| `q` | stop reading, end the program | nothing |

The program also ends at end of input.

## Errors

An error prints one line and changes nothing. Checks apply in this order:

- unknown command, wrong number of arguments, or an argument that is not a decimal integer: `error: bad command`
- `b`/`s` with a row outside 1..5 or a seat outside 1..8: `error: no such seat R S`
- `b` on a taken seat: `error: seat R S is taken`
- `s` on a free seat: `error: seat R S is not booked`
- `g` with N below 1 or above 8: `error: bad group size N`
- `g` when no row has N free seats next to each other: `error: no block of N free seats`

Numbers in the tests fit in a signed 32-bit integer. `R`, `S`, `N` in messages are the numbers as given in the command, printed in decimal.

## Seating plan

The plan has 6 lines: a header `  12345678` (two spaces), then one line per row: the row number, a space, and 8 characters, `.` for free and `X` for taken.

## Example

Input:

```
b 1 1
b 1 1
g 3
p
t
q
```

Output:

```
booked 1 1
error: seat 1 1 is taken
group 3 booked: row 1, seats 2 to 4
  12345678
1 XXXX....
2 ........
3 ........
4 ........
5 ........
sold 4 of 40, occupancy 10.0%
```
