import re
import sys
from collections import Counter

words = re.findall(rb"[A-Za-z]+", sys.stdin.buffer.read())
c = Counter(w.lower().decode() for w in words)
for w, n in sorted(c.items(), key=lambda kv: (-kv[1], kv[0]))[:10]:
    print(w, n)
print(f"total {len(words)} distinct {len(c)}")
