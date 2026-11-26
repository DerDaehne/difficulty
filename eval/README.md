# Agent measurement harness

Measures how well a language model writes working programs in difficulty (and, for comparison, in Python) when it sees only `llms.txt` and the task text.

## Layout

```
eval/
  run.py              runner, standard library only
  tasks/<name>/
    task.md           task text, language neutral (input, output, examples)
    tests/*.in, *.out hidden tests: stdin and expected stdout
    ref.py            Python reference, passes all tests
    ref.dif           difficulty reference (not there yet, see below)
  results/<time>/     one JSON per run and table.md
```

Tasks: `cinema` (hall with booking, cancel, group), `wordcount` (word frequencies), `vending` (state machine), `calc` (expression parser), `records` (input with broken lines).

A solution is a program that reads stdin and writes stdout. A test passes when stdout equals the `.out` file line by line and the exit code is 0.

## Procedure

For every language x model x task the runner:

1. builds the prompt: `llms.txt` (difficulty only) + `task.md` + the answer format (one fenced code block);
2. calls `claude -p --model <model> --output-format json` with a replaced system prompt (`--system-prompt`), without tools, session storage or slash commands, without locally configured MCP servers (`--strict-mcp-config`), user settings with their hooks and plugins (`--setting-sources project`) and dynamic system prompt sections, so the context is only `llms.txt` + task; in an empty directory; reads the token counts from the JSON;
3. extracts the last fenced block;
4. difficulty: `dif check --json`, then `dif build`, then all tests. Python: all tests. A failure produces feedback: the diagnostics, the build error, or the test name, exit code or the first 500 characters of the program's own output for the first failing test (expected output and test input stay hidden);
5. on failure, asks again with the previous solution and the feedback, up to 5 rounds in total.

Every round is a fresh call; nothing is carried over except the previous solution and its feedback.

Counted per run: **pass@1** (green in round 1), **pass@5** (green within 5 rounds), rounds used, tokens (input, cache creation, cache read, output) and the diagnostic codes `dif check` reported, summed over all rounds. Python runs have no codes.

## Usage

```sh
nix develop -c cargo build --release       # the runner uses target/release/dif
python3 eval/run.py --dry-run               # whole chain, no model, no cost
python3 eval/run.py --lang dif --model haiku --task cinema      # one real run
python3 eval/run.py --model haiku --model sonnet                # all tasks, both languages
```

Options: `--lang dif|python` (repeatable), `--model` (repeatable, default `haiku` and `sonnet`), `--task` (repeatable), `--rounds N`, `--dif PATH`, `--out DIR`.

`--dry-run` lets the Python reference answer. For difficulty it answers with `ref.dif` if present, otherwise with the empty program `void main() {}`, which compiles and fails the tests, so `check`, `build`, the test loop and the 5 rounds are all exercised.

Reference check: `for t in eval/tasks/*/; do for i in $t/tests/*.in; do python3 $t/ref.py < $i | cmp - ${i%.in}.out; done; done`

## difficulty references

`ref.dif` per task is added once the standard library exists. Until then `--dry-run --lang dif` uses the placeholder. A `ref.dif` must pass all tests of its task and, in dry-run mode, shows that the harness and the language agree.

## Cost estimate

One model call is about `prompt + answer` tokens. Rough sizes (about 3.5 characters per token):

| part | size |
|---|---|
| `llms.txt` (difficulty only) | about 16 KB, 4.5k tokens |
| `task.md` | 0.8 to 2 KB, 0.2 to 0.6k tokens |
| answer (program) | 0.5 to 2k tokens |
| feedback round | previous program and message, 1 to 2.5k tokens extra |
| fixed block of `claude -p` per call | about 3.5k tokens (measured below) |

Measured per call (2026-11-26, Haiku, prompt `llms.txt` + "Answer with ok.", 16 448 characters, empty directory; input tokens 2 in every case, the rest is cache creation):

| call | cache creation tokens |
|---|---|
| default `claude -p` flags of the first version | 63 670 |
| plus `--system-prompt` | 60 667 |
| plus `--strict-mcp-config --setting-sources project --exclude-dynamic-system-prompt-sections` (current) | 8 027 |

Without the last three flags, locally configured MCP servers, hooks and plugins add about 50k tokens to every call; the current call is about `llms.txt` plus 3.5k tokens.

Per task and round: difficulty about 4k input, Python about 1k input. With r rounds the input grows to roughly `r x (prompt + feedback)`.

Full run Haiku + Sonnet, both languages, 5 tasks (20 task runs):

- expected (average 2 rounds): difficulty 10 runs x about 10k input = 100k, Python 10 runs x about 3k = 30k, plus about 20 to 40k output, in total about 150k input and 30k output tokens;
- worst case (every run uses 5 rounds): difficulty 10 x 25k, Python 10 x 8k, in total about 330k input and 100k output tokens.

Add the fixed block of `claude -p` (about 3.5k tokens per call; 1 call per round, so up to 100 calls in the worst case). Cost in the subscription quota: a full run is small next to an interactive session, but Sonnet consumes noticeably more of the quota per token than Haiku, and Opus (later) more again. Check `claude -p "/usage"` before and after a run; use `--task` and `--model` to start with one cheap run. The exact numbers per run end up in the result JSON (`tokens`, `prompt_chars`).

## Failed calls and tests

A call that times out, exits non-zero, returns invalid JSON or `is_error` counts as a failed round (reason in the result `history`, field `error`); the run goes on with the same prompt. `python3 -m unittest discover eval` runs the tests (a stand-in for `claude` via `$EVAL_CLAUDE`, no model call).
