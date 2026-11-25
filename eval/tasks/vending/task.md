# Vending machine

Simulate a vending machine. Read commands from standard input, one per line; leading and trailing spaces are ignored, empty lines are skipped, arguments are separated by single spaces. The program ends at end of input.

## State

The machine has a credit in cents (starts at 0), a strike counter (starts at 0) and a locked flag (starts off). Items: `A` costs 60, `B` costs 120, `C` costs 150.

## Commands

| Command | Behaviour |
|---|---|
| `coin V` | If locked: `error: locked`. If V is 10, 20, 50 or 100: add V to the credit, print `credit C` (the new credit). Any other V (including text that is not a number): print `rejected`, add one strike; on the third strike also print `locked` and lock the machine. |
| `select X` | If locked: `error: locked`. Unknown item: `error: unknown item X`. Credit below the price: `error: need P more` (P = missing cents). Otherwise the item is dispensed and all remaining credit is returned as change: print `vend X change C`; credit becomes 0. |
| `refund` | If locked: `error: locked`. Otherwise print `refund C` with the whole credit; credit becomes 0. |
| `unlock` | If locked: print `unlocked`, clear the lock and the strikes (credit is kept). Otherwise `error: not locked`. |
| `status` | Print `state S credit C` where S is `locked` if locked, else `active` if the credit is above 0, else `idle`. |

Anything else (unknown command, missing or extra arguments): `error: bad command`. Errors change nothing.

A rejected coin counts as a strike only while the machine is not locked. Strikes are only cleared by `unlock`.

## Example

Input:

```
coin 50
coin 20
select B
coin 100
select B
status
```

Output:

```
credit 50
credit 70
error: need 50 more
credit 170
vend B change 50
state idle credit 0
```
