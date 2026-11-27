import re, sys
from collections import Counter

data = sys.stdin.buffer.read().lower()  # bytes.lower: ASCII only
words = re.findall(rb"[a-z]+", data)
counts = Counter(words)
for w, c in sorted(counts.items(), key=lambda p: (-p[1], p[0]))[:20]:
    print(w.decode(), c)
print("total", len(words), "distinct", len(counts))
