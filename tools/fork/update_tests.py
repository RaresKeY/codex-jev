"""Offline checks for release selection, startup choices and install boundaries."""

import hashlib
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import jev_update as update


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
