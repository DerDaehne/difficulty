#!/usr/bin/env python3
"""Agent measurement runner: language x model x task, pass@1 and pass@5.

Standard library only. See eval/README.md.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from collections import Counter
from pathlib import Path

EVAL = Path(__file__).resolve().parent
ROOT = EVAL.parent
EXT = {"dif": "dif", "python": "py"}
LANG_NAME = {"dif": "difficulty", "python": "Python 3"}
TEST_TIMEOUT = 10
MODEL_TIMEOUT = 600


def prompt_for(lang, task, feedback=None, last=None):
    parts = []
    if lang == "dif":
        parts.append("Below is the reference for the programming language difficulty.\n\n"
                     "<llms.txt>\n" + (ROOT / "llms.txt").read_text() + "\n</llms.txt>\n")
    parts.append("Task:\n\n" + (EVAL / "tasks" / task / "task.md").read_text())
    parts.append(f"Write the complete solution as one {LANG_NAME[lang]} program that reads standard input "
                 "and writes standard output. Answer with exactly one fenced code block containing the "
                 "whole program and no other code blocks.")
    if feedback:
        parts.append("Your previous solution:\n\n```\n" + last + "\n```\n\nIt failed:\n\n" + feedback +
                     "\n\nFix it and answer again with the complete program in one fenced code block.")
    return "\n\n".join(parts)


def extract(text):
    blocks = re.findall(r"```[^\n]*\n(.*?)```", text, re.S)
    return blocks[-1] if blocks else None


SYSTEM_PROMPT = "You are a careful programmer. Answer exactly as the task asks."
NO_TOKENS = dict.fromkeys(["input", "cache_creation", "cache_read", "output"], 0)


def ask_model(model, prompt, cwd, claude=None):
    """One model call. Returns (answer, tokens dict, error or None); never raises for a bad call.

    `claude` (or $EVAL_CLAUDE) replaces the command, for tests."""
    cmd = claude or os.environ.get("EVAL_CLAUDE", "claude")
    try:
        r = subprocess.run(
            [cmd, "-p", "--model", model, "--output-format", "json", "--tools", "",
             "--no-session-persistence", "--disable-slash-commands", "--system-prompt", SYSTEM_PROMPT],
            input=prompt, capture_output=True, text=True, cwd=cwd, timeout=MODEL_TIMEOUT)
    except (subprocess.TimeoutExpired, OSError) as e:
        return "", NO_TOKENS, f"model call failed: {type(e).__name__}"
    if r.returncode != 0:
        return "", NO_TOKENS, f"model call failed: exit code {r.returncode}"
    try:
        j = json.loads(r.stdout)
        if j.get("is_error"):
            return "", NO_TOKENS, "model call failed: is_error"
        u = j.get("usage", {})
        result = j.get("result", "")
    except (ValueError, AttributeError):
        return "", NO_TOKENS, "model call failed: invalid JSON"
    return result, {
        "input": u.get("input_tokens", 0),
        "cache_creation": u.get("cache_creation_input_tokens", 0),
        "cache_read": u.get("cache_read_input_tokens", 0),
        "output": u.get("output_tokens", 0),
    }, None


def ask_dry(lang, task):
    """Stand-in for the model: the reference solution, or a placeholder for dif."""
    ref = EVAL / "tasks" / task / f"ref.{EXT[lang]}"
    code = ref.read_text() if ref.exists() else "void main() {\n}\n"
    return "```\n" + code + "```", NO_TOKENS, None


def run_tests(cmd, task):
    """Run every tests/*.in; return (passed, total, feedback or None)."""
    ins = sorted((EVAL / "tasks" / task / "tests").glob("*.in"))
    for i, f in enumerate(ins):
        want = f.with_suffix(".out").read_text().splitlines()
        try:
            r = subprocess.run(cmd, stdin=f.open("rb"), capture_output=True, timeout=TEST_TIMEOUT)
        except subprocess.TimeoutExpired:
            return i, len(ins), f"test {f.stem}: no result after {TEST_TIMEOUT} seconds"
        got = r.stdout.decode("utf-8", "replace").splitlines()
        if r.returncode != 0:
            return i, len(ins), f"test {f.stem}: exit code {r.returncode}\n" + r.stderr.decode("utf-8", "replace")[-500:]
        if got != want:
            n = next((k for k in range(max(len(got), len(want)))
                      if k >= len(got) or k >= len(want) or got[k] != want[k]), 0)
            # the expected output stays hidden: show only the model's own output
            mine = "\n".join(got)[:500]
            return i, len(ins), f"test {f.stem}: output differs from the expected one (first difference at line {n + 1})\nyour output (first 500 characters):\n{mine}"
    return len(ins), len(ins), None


def evaluate(lang, code, task, work, dif):
    """Returns (ok, feedback, codes)."""
    if lang == "python":
        src = work / "main.py"
        src.write_text(code)
        _, _, fb = run_tests([sys.executable, "-I", str(src)], task)
        return fb is None, fb, []
    src, exe = work / "main.dif", work / "main"
    src.write_text(code)
    c = subprocess.run([dif, "check", "--json", str(src)], capture_output=True, text=True)
    lines = [ln for ln in c.stdout.splitlines() if ln.startswith("{")]
    if c.returncode != 0 or lines:
        codes = [json.loads(ln)["code"] for ln in lines]
        return False, "dif check:\n" + (c.stdout + c.stderr).replace(str(src), "main.dif"), codes
    b = subprocess.run([dif, "build", str(src), "-o", str(exe)], capture_output=True, text=True)
    if b.returncode != 0:
        return False, "dif build:\n" + (b.stdout + b.stderr).replace(str(src), "main.dif"), []
    _, _, fb = run_tests([str(exe)], task)
    return fb is None, fb, []


def one_run(lang, model, task, args):
    res = {"lang": lang, "model": model, "task": task, "dry_run": args.dry_run, "rounds": 0,
           "pass1": False, "pass5": False, "tokens": Counter(), "codes": Counter(),
           "prompt_chars": [], "history": []}
    feedback = last = None
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        (work / "model").mkdir()  # empty cwd: the model sees no project files
        for rnd in range(1, args.rounds + 1):
            p = prompt_for(lang, task, feedback, last)
            res["prompt_chars"].append(len(p))
            ans, tok, err = ask_dry(lang, task) if args.dry_run else ask_model(model, p, work / "model")
            res["tokens"].update(tok)
            code = extract(ans)
            res["rounds"] = rnd
            if err:  # failed round: keep the previous solution and feedback, ask again
                res["history"].append({"round": rnd, "ok": False, "codes": [], "error": err})
                continue
            if code is None:
                ok, feedback, codes, last = False, "No fenced code block found in the answer.", [], ans
            else:
                ok, feedback, codes = evaluate(lang, code, task, work, args.dif)
                last = code
            res["codes"].update(codes)
            res["history"].append({"round": rnd, "ok": ok, "codes": codes, "feedback": feedback})
            if ok:
                res["pass1"], res["pass5"] = rnd == 1, True
                break
    res["tokens"], res["codes"] = dict(res["tokens"]), dict(res["codes"])
    return res


def table(results):
    out = ["| language | model | task | pass@1 | pass@5 | rounds | tokens (in+cache/out) | codes |",
           "|---|---|---|---|---|---|---|---|"]
    for r in results:
        t = r["tokens"]
        tok = f"{t['input'] + t['cache_creation'] + t['cache_read']}/{t['output']}"
        codes = ", ".join(f"{c} x{n}" for c, n in sorted(r["codes"].items())) or "-"
        out.append(f"| {r['lang']} | {r['model']} | {r['task']} | {'yes' if r['pass1'] else 'no'} | "
                   f"{'yes' if r['pass5'] else 'no'} | {r['rounds']} | {tok} | {codes} |")
    out += ["", "| language | model | pass@1 | pass@5 | mean rounds | tokens (in+cache/out) |", "|---|---|---|---|---|---|"]
    for lang, model in sorted({(r["lang"], r["model"]) for r in results}):
        g = [r for r in results if (r["lang"], r["model"]) == (lang, model)]
        ti = sum(r["tokens"]["input"] + r["tokens"]["cache_creation"] + r["tokens"]["cache_read"] for r in g)
        to = sum(r["tokens"]["output"] for r in g)
        out.append(f"| {lang} | {model} | {sum(r['pass1'] for r in g)}/{len(g)} | {sum(r['pass5'] for r in g)}/{len(g)} | "
                   f"{sum(r['rounds'] for r in g) / len(g):.1f} | {ti}/{to} |")
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--lang", action="append", choices=list(EXT), help="repeatable; default: dif and python")
    ap.add_argument("--model", action="append", help="repeatable; default: haiku and sonnet")
    ap.add_argument("--task", action="append", help="task directory name, repeatable; default: all")
    ap.add_argument("--rounds", type=int, default=5, help="max rounds per run (pass@5)")
    ap.add_argument("--dry-run", action="store_true", help="no model call: the reference solution answers")
    ap.add_argument("--dif", default=str(ROOT / "target/release/dif"), help="path of the dif binary")
    ap.add_argument("--out", help="result directory; default eval/results/<timestamp>")
    args = ap.parse_args()
    langs = args.lang or ["dif", "python"]
    models = args.model or ["haiku", "sonnet"]
    tasks = args.task or sorted(p.parent.name for p in (EVAL / "tasks").glob("*/task.md"))
    out = Path(args.out or EVAL / "results" / time.strftime("%Y%m%d-%H%M%S"))
    out.mkdir(parents=True)
    results = []
    for lang in langs:
        for model in models:
            for task in tasks:
                print(f"{lang} {model} {task} ...", flush=True)
                r = one_run(lang, model, task, args)
                (out / f"{lang}-{model}-{task}.json").write_text(json.dumps(r, indent=2) + "\n")
                results.append(r)
    (out / "table.md").write_text(table(results))
    print(table(results))
    print(f"results in {out}")


if __name__ == "__main__":
    main()
