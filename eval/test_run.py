import os, stat, tempfile, unittest
from pathlib import Path
import run


def fake(body):
    d = tempfile.mkdtemp()
    p = Path(d) / "claude"
    p.write_text("#!/bin/sh\ncat >/dev/null\n" + body + "\n")
    p.chmod(p.stat().st_mode | stat.S_IEXEC)
    return str(p)


class AskModel(unittest.TestCase):
    def ask(self, body):
        return run.ask_model("haiku", "x", ".", claude=fake(body))

    def test_ok(self):
        a, t, e = self.ask("""echo '{"result":"hi","usage":{"input_tokens":3,"output_tokens":2}}'""")
        self.assertEqual((a, t["input"], t["output"], e), ("hi", 3, 2, None))

    def test_failures_are_rounds_not_crashes(self):
        for body in ["echo not json", """echo '{"is_error":true,"result":"x"}'""", "exit 3", "echo '[1]'"]:
            self.assertIsNotNone(self.ask(body)[2], body)

    def test_missing_command_and_timeout(self):
        self.assertIsNotNone(run.ask_model("haiku", "x", ".", claude="/nonexistent/claude")[2])
        old, run.MODEL_TIMEOUT = run.MODEL_TIMEOUT, 0.2
        try:
            self.assertIn("Timeout", self.ask("sleep 5")[2])
        finally:
            run.MODEL_TIMEOUT = old


class Feedback(unittest.TestCase):
    def test_expected_output_hidden(self):
        task = next(p.parent.name for p in (run.EVAL / "tasks").glob("*/task.md"))
        t = sorted((run.EVAL / "tasks" / task / "tests").glob("*.in"))[0]
        want = t.with_suffix(".out").read_text().splitlines()
        wrong = Path(tempfile.mkdtemp()) / "w.py"
        wrong.write_text("print('WRONGANSWER')")
        _, _, fb = run.run_tests([run.sys.executable, "-I", str(wrong)], task)
        self.assertIn(t.stem, fb)
        self.assertIn("WRONGANSWER", fb)
        for line in want:
            if line.strip() and line != "WRONGANSWER":
                self.assertNotIn(line, fb)


if __name__ == "__main__":
    unittest.main()
