"""Offline checks for release selection, startup choices and install boundaries."""

import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import jev_update as update


# Archive contract used by the updater before weekly budgets were introduced.
PRE_BUDGET_PAYLOAD = {
    "codex", "codex-code-mode-host", "jev-route.py", "policy.json", "LICENSE",
    "NOTICE", "install.sh", "install-source.sh", "VERSION", "RELEASE.json",
    "jev_launcher.py", "jev_session.py", "jev_update.py", "verify_trust.py",
    "incident-policy.json",
}


class UpdateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.environment = patch.dict(
            os.environ,
            {
                "XDG_CACHE_HOME": str(self.root / "cache"),
                "CODEX_HOME": str(self.root / "codex"),
            },
        )
        self.environment.start()
        self.addCleanup(self.environment.stop)
        (self.root / "RELEASE.json").write_text('{"version":"0.165.0-jev.1"}')
        self.selected = self.release("v0.165.0-jev.2")

    @staticmethod
    def release(tag, **extra):
        base = f"https://github.com/{update.REPOSITORY}/releases/download/{tag}/"
        return dict(
            tag_name=tag,
            assets=[
                {"name": name, "browser_download_url": base + name}
                for name in (update.ARCHIVE, "SHA256SUMS")
            ],
            **extra,
        )

    def package(self, *, profile="release", extra=None):
        metadata = json.dumps(
            {"version": "0.165.0-jev.2", "profile": profile, "target": "linux-x86_64"}
        ).encode()
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode="w:gz") as bundle:
            for name in sorted(update.PAYLOAD):
                body = metadata if name == "RELEASE.json" else b"fixture\n"
                item = tarfile.TarInfo("codex-jev-linux-x86_64/" + name)
                item.size = len(body)
                bundle.addfile(item, io.BytesIO(body))
            if extra:
                bundle.addfile(extra, io.BytesIO(b""))
        data = stream.getvalue()
        checksum = f"{hashlib.sha256(data).hexdigest()}  {update.ARCHIVE}\n".encode()
        return checksum, data

    def test_selects_latest_complete_fork_release_including_prereleases(self):
        malicious = self.release("v99.0.0-jev.1")
        malicious["assets"][0]["browser_download_url"] = (
            "https://example.invalid/install"
        )
        selected = update.select_release(
            [
                self.release("v0.160.0-jev.1"),
                self.selected,
                self.release("v1.0.0-jev.1", draft=True),
                self.release("rust-v9.0.0"),
                malicious,
            ]
        )
        self.assertEqual(selected["tag"], "v0.165.0-jev.2")
        self.assertGreater(
            update.version("0.165.0-jev.10"), update.version("0.165.0-jev.2")
        )

    def test_bad_checksum_never_executes_installer(self):
        with (
            patch.object(
                update,
                "fetch",
                side_effect=[
                    b"0" * 64 + b"  " + update.ARCHIVE.encode() + b"\n",
                    b"corrupt",
                ],
            ),
            patch.object(update.subprocess, "run") as install,
        ):
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                update.install_release(update.select_release([self.selected]))
            install.assert_not_called()

    def test_archive_paths_symlinks_and_development_builds_are_rejected(self):
        traversal = tarfile.TarInfo("codex-jev-linux-x86_64/../../outside")
        symlink = tarfile.TarInfo("codex-jev-linux-x86_64/evil")
        symlink.type = tarfile.SYMTYPE
        symlink.linkname = "/tmp/outside"
        for kwargs in (
            {"extra": traversal},
            {"extra": symlink},
            {"profile": "dev-small"},
        ):
            with (
                self.subTest(kwargs=kwargs),
                patch.object(update, "fetch", side_effect=self.package(**kwargs)),
                patch.object(update.subprocess, "run") as install,
            ):
                with self.assertRaises(ValueError):
                    update.install_release(update.select_release([self.selected]))
                install.assert_not_called()

    def test_complete_optimized_matching_package_runs_its_installer_once(self):
        with (
            patch.object(update, "fetch", side_effect=self.package()),
            patch.object(
                update.subprocess,
                "check_output",
                return_value="codex-cli 0.165.0-jev.2\n",
            ),
            patch.object(update.subprocess, "run") as install,
        ):
            update.install_release(update.select_release([self.selected]))
            install.assert_called_once()
            self.assertEqual(install.call_args.args[0][0], "bash")
            self.assertTrue(install.call_args.args[0][1].endswith("/install.sh"))

    def test_package_updates_through_previous_payload_contract_with_working_budget(self):
        source = Path(__file__).resolve().parents[2]
        checkout = self.root / "checkout"
        files = [
            "tools/fork/package.sh", "tools/fork/install.sh",
            "tools/fork/jev_launcher.py", "tools/fork/jev_session.py",
            "tools/fork/jev_update.py", "tools/fork/ca-trust/verify_trust.py",
            "tools/fork/ca-trust/incident-policy.json", "jev/route.py",
            "jev/jev_budget.py", "jev/policy.json",
        ]
        for name in files:
            target = checkout / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source / name, target)
        for name in ("LICENSE", "NOTICE"):
            (checkout / name).write_text("offline fixture\n")
        (checkout / "codex-rs").mkdir()
        (checkout / "codex-rs/Cargo.toml").write_text(
            '[workspace.package]\nversion = "0.165.0-jev.2"\n'
        )
        binaries = checkout / ".build/target/release"
        binaries.mkdir(parents=True)
        for name in ("codex", "codex-code-mode-host"):
            binary = binaries / name
            binary.write_text("#!/bin/sh\nprintf 'codex-cli 0.165.0-jev.2\\n'\n")
            binary.chmod(0o755)
        fake_bin = self.root / "fake-bin"
        fake_bin.mkdir()
        git = fake_bin / "git"
        git.write_text("#!/bin/sh\nprintf 'b997e49f41e4\\n'\n")
        git.chmod(0o755)
        home = self.root / "home"
        environment = {
            **os.environ,
            "HOME": str(home),
            "XDG_DATA_HOME": str(home / "data"),
            "XDG_CONFIG_HOME": str(home / "config"),
            "CODEX_JEV_BUDGET_FILE": str(home / "budget.sqlite3"),
            "CODEX_JEV_BUILD_PROFILE": "release",
            "PATH": str(fake_bin) + os.pathsep + os.environ["PATH"],
        }
        # Repackaging a prior candidate must also remove its separate budget member.
        package = checkout / ".build/release/codex-jev-linux-x86_64"
        package.mkdir(parents=True)
        (package / "jev_budget.py").write_text("stale candidate module\n")
        packaged = subprocess.run(
            ["bash", str(checkout / "tools/fork/package.sh")], env=environment,
            capture_output=True, text=True, timeout=15,
        )
        self.assertEqual(packaged.returncode, 0, packaged.stderr)
        release = checkout / ".build/release"
        downloads = [(release / "SHA256SUMS").read_bytes(),
                     (release / update.ARCHIVE).read_bytes()]
        # Neither extraction nor installation may need access to the source checkout.
        shutil.rmtree(checkout)
        with (
            patch.object(update, "PAYLOAD", PRE_BUDGET_PAYLOAD),
            patch.object(update, "fetch", side_effect=downloads),
            patch.dict(os.environ, environment),
        ):
            update.install_release(update.select_release([self.selected]))
        installed = home / "data/codex-jev/current"
        self.assertEqual((installed / "jev_budget.py").read_bytes(),
                         (source / "jev/jev_budget.py").read_bytes())
        status = subprocess.run(
            [str(home / ".local/bin/codex-jev"), "budget", "status", "--json"],
            env=environment, capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(status.returncode, 0, status.stderr)
        state = json.loads(status.stdout)
        self.assertEqual((state["enabled"], state["limit_tokens"], state["used_tokens"]),
                         (True, 10_000_000, 0))
        helper = subprocess.run(
            [sys.executable, "-I", str(installed / "jev-route.py")], input="{}",
            env=environment, capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(helper.returncode, 1)
        self.assertIn("error", json.loads(helper.stdout))
        self.assertEqual(helper.stderr, "")

    def test_native_version_mismatch_never_installs(self):
        with (
            patch.object(update, "fetch", side_effect=self.package()),
            patch.object(
                update.subprocess,
                "check_output",
                return_value="codex-cli 0.160.0-jev.1\n",
            ),
            patch.object(update.subprocess, "run") as install,
        ):
            with self.assertRaisesRegex(ValueError, "binary version"):
                update.install_release(update.select_release([self.selected]))
            install.assert_not_called()

    def test_refresh_checks_trust_before_any_https_and_reuses_fresh_metadata(self):
        with (
            patch.object(update, "check_trust", side_effect=ValueError("unsafe trust")),
            patch.object(update, "fetch") as fetch,
        ):
            with self.assertRaisesRegex(ValueError, "unsafe trust"):
                update.refresh(self.root)
            fetch.assert_not_called()
        with (
            patch.object(update, "check_trust"),
            patch.object(
                update, "fetch", return_value=json.dumps([self.selected]).encode()
            ) as fetch,
        ):
            first = update.refresh(self.root)
            self.assertEqual(update.refresh(self.root), first)
            fetch.assert_called_once()

    def test_skipped_version_is_persisted_and_startup_does_not_install(self):
        state = {
            "checkedAt": update.time.time(),
            "latest": update.select_release([self.selected]),
        }
        update.write_state(state)
        with (
            patch("builtins.input", return_value="3"),
            patch.object(update, "install_release") as install,
        ):
            update.maybe_offer_update(self.root, ["--no-daemon"])
            self.assertEqual(
                update.read_state()["dismissed"], self.selected["tag_name"]
            )
            with patch(
                "builtins.input",
                side_effect=AssertionError("dismissed version prompted"),
            ):
                update.maybe_offer_update(self.root, [])
            install.assert_not_called()

    def test_startup_setting_disables_release_checks(self):
        config = self.root / "codex/config.toml"
        config.parent.mkdir()
        config.write_text("check_for_update_on_startup = false\n")
        with patch.object(update.subprocess, "Popen") as background:
            update.maybe_offer_update(self.root, [])
            background.assert_not_called()


if __name__ == "__main__":
    unittest.main()
