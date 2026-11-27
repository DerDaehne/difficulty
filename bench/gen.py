# Deterministic text generator: python3 gen.py SIZE_BYTES > text.txt
import random, sys
rnd = random.Random(42)
letters = "abcdefghijklmnopqrstuvwxyz"
vocab = ["".join(rnd.choice(letters) for _ in range(rnd.randint(2, 10))) for _ in range(5000)]
vocab += ["über", "Köln", "schön", "Mädchen", "Straße", "Größe", "für", "Ärger", "Öl", "grün"]
rnd.shuffle(vocab)
cum, s = [], 0.0
for i in range(len(vocab)):
    s += 1 / (i + 1)  # Zipf: weight 1/rank
    cum.append(s)
size, out, n = int(sys.argv[1]), [], 0
while n < size:
    sent = rnd.choices(vocab, cum_weights=cum, k=rnd.randint(4, 16))
    sent = [w.upper() if rnd.random() < 0.02 else w for w in sent]
    sent[0] = sent[0].capitalize()
    line = " ".join(sent) + rnd.choice([".", ".", "!", "?"])
    if rnd.random() < 0.3:
        line = line.replace(" ", ", ", 1)
    out.append(line + "\n")
    n += len(line.encode()) + 1
sys.stdout.write("".join(out))
