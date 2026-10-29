"""Offline installer transaction checks using temporary homes and fake payloads."""

import json
import os
from pathlib import Path
import runpy
import shlex
import shutil
import subprocess
import tempfile
import unittest


INSTALLER = Path(__file__).with_name("install.sh")


class InstallTransactionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="codex-jev-install-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.home = self.root / "home"
        self.dist = self.root / "dist"
        self.dist.mkdir()
        for name in ("codex", "codex-code-mode-host"):
            path = self.dist / name
            path.write_text("#!/bin/sh\nexit 0\n")
            path.chmod(0o755)
        for name in ("jev-route.py", "policy.json", "LICENSE", "NOTICE"):
            (self.dist / name).write_text("offline fixture\n")
        shutil.copy2(
            INSTALLER.parents[2] / "jev/jev_budget.py", self.dist / "jev_budget.py"
        )
        for name in ("jev_launcher.py", "jev_session.py", "jev_update.py"):
            shutil.copy2(INSTALLER.parent / name, self.dist / name)
        for name in ("verify_trust.py", "incident-policy.json"):
            shutil.copy2(INSTALLER.parent / "ca-trust" / name, self.dist / name)
        (self.dist / "RELEASE.json").write_text(
            '{"version":"0.165.0-jev.1","profile":"dev-small","target":"linux-x86_64"}\n'
        )
        (self.dist / "VERSION").write_text("f5cd3fc5466a\n")
        self.data = self.home / ".local/share/codex-jev"
        self.config = self.home / ".config/codex-jev"
        self.launcher = self.home / ".local/bin/codex-jev"
        self.env = {
            **os.environ,
            "HOME": str(self.home),
            "XDG_DATA_HOME": str(self.home / ".local/share"),
            "XDG_CONFIG_HOME": str(self.home / ".config"),
            "CODEX_JEV_DIST": str(self.dist),
        }

    def install(self, *args, env=None):
        return subprocess.run(
            ["bash", str(INSTALLER), *map(str, args)],
            env=env or self.env,
            capture_output=True,
            text=True,
            timeout=15,
        )

    def test_newline_at_end_of_resolved_key_path_is_rejected_without_publication(self):
        key = self.root / "key.env\n"
        key.touch()
        link = self.root / "key-link.env"
        link.symlink_to(key)
        for supplied in (key, link):
            with self.subTest(supplied=supplied):
                rejected = self.install("--key-file", supplied)
                self.assertNotEqual(rejected.returncode, 0)
                self.assertIn("cannot contain newlines", rejected.stderr)
                self.assertFalse((self.data / "current").exists())
                self.assertFalse((self.config / "key-file").exists())

    def test_failed_release_switch_restores_launcher_key_and_previous_release(self):
        old_key = self.root / "old-key.env"
        new_key = self.root / "new-key.env"
        old_key.touch()
        new_key.touch()
        first = self.install("--key-file", old_key)
        self.assertEqual(first.returncode, 0, first.stderr)
        old_release = (self.data / "current").resolve()
        # An existing launcher can differ from the new template.
        with self.launcher.open("a") as stream:
            stream.write("# prior launcher marker\n")
        previous_launcher = self.launcher.read_bytes()
        previous_key = (self.config / "key-file").read_bytes()
        fake_bin = self.root / "fake-bin"
        fake_bin.mkdir()
        wrapper = fake_bin / "mv"
        real_mv = shlex.quote(shutil.which("mv"))
        wrapper.write_text(
            "#!/usr/bin/env bash\n"
            'if [[ ${!#} == "$JEV_TEST_FAIL_CURRENT" ]]; then exit 73; fi\n'
            f'exec {real_mv} "$@"\n'
        )
        wrapper.chmod(0o755)
        failed = self.install(
            "--key-file",
            new_key,
            env={
                **self.env,
                "PATH": str(fake_bin) + os.pathsep + self.env["PATH"],
                "JEV_TEST_FAIL_CURRENT": str(self.data / "current"),
            },
        )
        self.assertNotEqual(failed.returncode, 0)
        self.assertEqual((self.data / "current").resolve(), old_release)
        self.assertEqual(self.launcher.read_bytes(), previous_launcher)
        self.assertEqual((self.config / "key-file").read_bytes(), previous_key)
        self.assertEqual(sorted((self.data / "releases").iterdir()), [old_release])
        self.assertEqual(sorted(self.launcher.parent.iterdir()), [self.launcher])
        self.assertEqual(sorted(self.config.iterdir()), [self.config / "key-file"])

    def test_installed_budget_commands_share_accounting_and_preserve_controls(self):
        installed = self.install()
        self.assertEqual(installed.returncode, 0, installed.stderr)
        state_file = self.root / "isolated-budget.sqlite3"
        environment = {**self.env, "CODEX_JEV_BUDGET_FILE": str(state_file)}

        def command(*arguments):
            result = subprocess.run(
                [str(self.launcher), "budget", *arguments, "--json"],
                env=environment,
                capture_output=True,
                text=True,
                timeout=10,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            state = json.loads(result.stdout)
            return {key: state[key] for key in ("enabled", "limit_tokens", "used_tokens")}

        self.assertEqual(command("status"), {
            "enabled": True, "limit_tokens": 10_000_000, "used_tokens": 0,
        })
        installed_budget = runpy.run_path(str(self.data / "current/jev_budget.py"))
        budget = installed_budget["Budget"](state_file)
        budget.settle(budget.reserve(500), 123)
        self.assertEqual(command("off"), {
            "enabled": False, "limit_tokens": 10_000_000, "used_tokens": 123,
        })
        self.assertEqual(command("set", "1000"), {
            "enabled": False, "limit_tokens": 1000, "used_tokens": 123,
        })
        self.assertEqual(command("on"), {
            "enabled": True, "limit_tokens": 1000, "used_tokens": 123,
        })
        self.assertEqual(command("reset"), {
            "enabled": True, "limit_tokens": 1000, "used_tokens": 0,
        })


if __name__ == "__main__":
    unittest.main()
