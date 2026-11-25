# Word count

Read all of standard input as text and count how often each word occurs.

A word is a maximal run of the ASCII letters `a`-`z` and `A`-`Z`. Every other character (digits, punctuation, spaces, newlines, bytes of non-ASCII characters) separates words. Words are compared in lower case.

## Output

The (up to) 10 most frequent words, one per line as `word count`, ordered by count descending, then by word ascending (byte order). Then one final line `total N distinct M` with the number of all words and the number of different words.

Empty input, or input without words, prints only the final line.

## Example

Input:

```
The cat and the hat.
THE END, and 42 more!
```

Output:

```
the 3
and 2
cat 1
end 1
hat 1
more 1
total 9 distinct 6
```
