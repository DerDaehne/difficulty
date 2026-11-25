# Records with errors

Standard input holds one record per line: `id,name,amount`. Some lines are broken. Report every broken line and summarise the good ones.

## Lines

- Lines are numbered from 1. Empty lines (nothing but spaces, tabs or `\r`) and lines whose first character is `#` are skipped, but they still count for numbering.
- A record has exactly three fields separated by `,`. Spaces, tabs and `\r` around each field are ignored.
- `id`: only ASCII digits, 1 to 9 of them, value at least 1.
- `name`: not empty.
- `amount`: an optional `-`, digits, and optionally `.` followed by one or two digits (`12`, `-3.5`, `0.07`).
- An `id` that was already used by an earlier valid record is a duplicate.

## Output

For each broken line, in input order, print `line N: REASON` with the first reason that applies, checked in this order:

1. `expected 3 fields`
2. `bad id`
3. `empty name`
4. `bad amount`
5. `duplicate id`

Then print one summary line: `ok R, bad B, total T` where R is the number of valid records, B the number of broken lines and T the sum of the amounts of valid records with exactly two decimals (`-0.50`, `12.00`; a sum of zero is `0.00`, never `-0.00`).

## Example

Input:

```
# id,name,amount
1,Ann,10.5
2,,3
x,Bob,1
3,Cy,7.123
1,Dee,2
4,Eve
5, Fay ,-2.25
```

Output:

```
line 4: empty name
line 5: bad id
line 6: bad amount
line 7: duplicate id
line 8: expected 3 fields
ok 2, bad 5, total 8.25
```
