"""Exercise supervisor ownership, argv preservation and cleanup with inert peers."""

import json
import os
from pathlib import Path
import subprocess
import shutil
import sys
import tempfile
import unittest


class SessionLifecycleTests(unittest.TestCase):
    def test_noninteractive_command_keeps_argv_without_starting_a_session(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in ("jev_launcher.py", "jev_session.py", "jev_update.py"):
                shutil.copy2(Path(__file__).parent / name, root / name)
            native = root / "codex-jev-native"
            native.write_text(
                "#!/usr/bin/env python3\nimport json,sys\nprint(json.dumps(sys.argv[1:]))\n"
            )
            native.chmod(0o755)
            args = ["--no-daemon", "--profile", "saved", "exec", "a literal prompt"]
            result = subprocess.run(
                [sys.executable, str(root / "jev_launcher.py"), *args],
                capture_output=True,
                text=True,
                timeout=10,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), args)

    def test_foreground_server_is_reaped_and_prompt_argv_is_preserved(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            native = root / "fake-native"
            native.write_text("""#!/usr/bin/env python3
import json, os, signal, socket, sys, time
from pathlib import Path
args = sys.argv[1:]
if "app-server" in args:
    endpoint = args[args.index("--listen") + 1].removeprefix("unix://")
    server = socket.socket(socket.AF_UNIX)
    server.bind(endpoint)
    server.listen()
    while True:
        connection, _ = server.accept()
        connection.close()
else:
    Path(os.environ["JEV_TEST_RESULT"]).write_text(json.dumps({"args": args, "ignored_sigint": signal.getsignal(signal.SIGINT) == signal.SIG_IGN}))
    time.sleep(.2)
    raise SystemExit(7)
""")
            native.chmod(0o755)
            driver = "from pathlib import Path; from jev_session import run_session; import sys; sys.exit(run_session(Path(sys.argv[1]), sys.argv[2:]))"
            completed = subprocess.run(
                [
                    sys.executable,
                    "-c",
                    driver,
                    str(native),
                    "--no-daemon",
                    "--yolo",
                    "--",
                    "--no-daemon",
                ],
                env={
                    **os.environ,
                    "PYTHONPATH": str(Path(__file__).resolve().parent),
                    "XDG_RUNTIME_DIR": str(root),
                    "JEV_TEST_RESULT": str(root / "result.json"),
                },
                capture_output=True,
                text=True,
                timeout=15,
            )
            self.assertEqual(completed.returncode, 7, completed.stderr)
            data = json.loads((root / "result.json").read_text())
            self.assertEqual(data["args"][0], "--remote")
            self.assertTrue(data["args"][1].startswith("unix://"))
            self.assertEqual(data["args"][2:], ["--yolo", "--", "--no-daemon"])
            self.assertFalse(data["ignored_sigint"])
            self.assertEqual(list((root / "codex-jev/sessions").iterdir()), [])


if __name__ == "__main__":
    unittest.main()
