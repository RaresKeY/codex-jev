"""Jev release checks and checksum-verified, opt-in installation. Linux only."""

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
import urllib.request


REPOSITORY = "RaresKeY/codex-jev"
ARCHIVE = "codex-jev-linux-x86_64.tar.gz"
RELEASES_URL = f"https://api.github.com/repos/{REPOSITORY}/releases?per_page=20"
MAX_ARCHIVE = 512 * 1024 * 1024
TTL = 20 * 3600
# Preserve the original updater contract; the installer embeds additional modules.
PAYLOAD = {
    "codex",
    "codex-code-mode-host",
    "jev-route.py",
    "policy.json",
    "LICENSE",
    "NOTICE",
    "install.sh",
    "install-source.sh",
    "VERSION",
    "RELEASE.json",
    "jev_launcher.py",
    "jev_session.py",
    "jev_update.py",
    "verify_trust.py",
    "incident-policy.json",
}


def version(text):
    match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)-jev\.(\d+)", text)
    if not match:
        raise ValueError("Invalid Jev release version")
    return tuple(map(int, match.groups()))


def state_file():
    root = (
        Path(os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache")))
        / "codex-jev"
    )
    root.mkdir(parents=True, mode=0o700, exist_ok=True)
    return root / "update.json"


def read_state():
    try:
        data = json.loads(state_file().read_text())
        return data if isinstance(data, dict) else {}
    except (OSError, ValueError):
        return {}


def write_state(data):
    path = state_file()
    fd, name = tempfile.mkstemp(prefix=".update-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as stream:
            json.dump(data, stream)
        os.replace(name, path)
    finally:
        Path(name).unlink(missing_ok=True)


def check_trust(release):
    result = subprocess.run(
        [
            sys.executable,
            "-I",
            str(release / "verify_trust.py"),
            "--system",
            "--python",
        ],
        capture_output=True,
        text=True,
        timeout=45,
    )
    if result.returncode:
        detail = result.stderr.strip().splitlines()
        raise ValueError(
            "Update HTTPS blocked by trust inspection: "
            + (detail[-1] if detail else f"exit {result.returncode}")
        )


class SecureRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, new_url):
        if not new_url.startswith("https://"):
            raise ValueError("Release download attempted a non-HTTPS redirect")
        return super().redirect_request(request, fp, code, message, headers, new_url)


def fetch(url, maximum):
    request = urllib.request.Request(
        url,
        headers={
            "User-Agent": "codex-jev-updater",
            "Accept": "application/vnd.github+json",
        },
    )
    with urllib.request.build_opener(SecureRedirect()).open(
        request, timeout=20
    ) as response:
        data = response.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError("Release response exceeds its size limit")
    return data


def select_release(releases):
    if not isinstance(releases, list):
        raise ValueError("Unexpected GitHub release response")
    choices = []
    for release in releases:
        if not isinstance(release, dict):
            continue
        tag = release.get("tag_name", "")
        try:
            number = version(tag)
        except (ValueError, TypeError):
            continue
        if release.get("draft"):
            continue
        assets = {
            asset.get("name"): asset.get("browser_download_url")
            for asset in release.get("assets", [])
        }
        base = f"https://github.com/{REPOSITORY}/releases/download/{tag}/"
        if all(assets.get(name) == base + name for name in (ARCHIVE, "SHA256SUMS")):
            choices.append(
                (
                    number,
                    {
                        "tag": tag,
                        "archive": base + ARCHIVE,
                        "checksums": base + "SHA256SUMS",
                    },
                )
            )
    return max(choices, key=lambda choice: choice[0])[1] if choices else None


def refresh(release, force=False):
    path = state_file()
    with path.with_suffix(".lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return read_state()
        state = read_state()
        if not force and time.time() - state.get("checkedAt", 0) < TTL:
            return state
        check_trust(release)
        latest = select_release(json.loads(fetch(RELEASES_URL, 2 * 1024 * 1024)))
        state.update(checkedAt=time.time(), latest=latest)
        state.pop("error", None)
        write_state(state)
        return state


def install_release(selected):
    # Revalidate cached metadata before making any network or install request.
    tag = selected["tag"]
    version(tag)
    base = f"https://github.com/{REPOSITORY}/releases/download/{tag}/"
    if selected != {
        "tag": tag,
        "archive": base + ARCHIVE,
        "checksums": base + "SHA256SUMS",
    }:
        raise ValueError("Unexpected release source")
    checksums = fetch(selected["checksums"], 65536).decode("ascii")
    matches = re.findall(
        r"^([0-9a-f]{64}) [ *]" + re.escape(ARCHIVE) + r"$", checksums, re.M
    )
    if len(matches) != 1:
        raise ValueError("Missing or ambiguous archive checksum")
    data = fetch(selected["archive"], MAX_ARCHIVE)
    if hashlib.sha256(data).hexdigest() != matches[0]:
        raise ValueError("Jev archive checksum mismatch; installation cancelled")
    with tempfile.TemporaryDirectory(prefix="codex-jev-update-") as temp:
        root = Path(temp)
        archive = root / "release.tar.gz"
        archive.write_bytes(data)
        package = root / "codex-jev-linux-x86_64"
        package.mkdir()
        seen = set()
        total = 0
        with tarfile.open(archive) as bundle:
            for member in bundle:
                if member.isdir() and member.name.rstrip("/") == package.name:
                    continue
                prefix, separator, name = member.name.partition("/")
                if (
                    prefix != package.name
                    or not separator
                    or name not in PAYLOAD
                    or name in seen
                    or not member.isfile()
                ):
                    raise ValueError("Unexpected archive member")
                seen.add(name)
                total += member.size
                if total > 2 * 1024**3:
                    raise ValueError("Expanded release exceeds its size limit")
                with (
                    bundle.extractfile(member) as source,
                    (package / name).open("wb") as target,
                ):
                    shutil.copyfileobj(source, target)
                (package / name).chmod(
                    0o755
                    if name
                    in {
                        "codex",
                        "codex-code-mode-host",
                        "install.sh",
                        "install-source.sh",
                    }
                    else 0o644
                )
        if seen != PAYLOAD:
            raise ValueError("Incomplete Jev release payload")
        metadata = json.loads((package / "RELEASE.json").read_text())
        if (
            version(metadata.get("version", "")) != version(tag)
            or metadata.get("profile") != "release"
            or metadata.get("target") != "linux-x86_64"
        ):
            raise ValueError("Release identity, optimization or platform mismatch")
        native_version = subprocess.check_output(
            [str(package / "codex"), "--version"], text=True, timeout=15
        ).strip()
        if native_version != "codex-cli " + metadata["version"]:
            raise ValueError("Native binary version does not match release metadata")
        subprocess.run(["bash", str(package / "install.sh")], check=True)


def enabled():
    if os.environ.get("CODEX_JEV_CHECK_UPDATES") == "0":
        return False
    try:
        config = (
            Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex")))
            / "config.toml"
        )
        return (
            tomllib.loads(config.read_text()).get("check_for_update_on_startup", True)
            is not False
        )
    except (OSError, ValueError):
        return True


def current_version(release):
    return json.loads((release / "RELEASE.json").read_text())["version"]


def maybe_offer_update(release, args):
    if (
        not enabled()
        or platform.system() != "Linux"
        or platform.machine() not in ("x86_64", "AMD64")
    ):
        return
    try:
        state = read_state()
        if state.get("error"):
            print("Jev update check failed: " + str(state["error"]), file=sys.stderr)
        if time.time() - state.get("checkedAt", 0) >= TTL:
            subprocess.Popen(
                [sys.executable, str(release / "jev_update.py"), "--refresh"],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
        latest = state.get("latest")
        if (
            not latest
            or state.get("dismissed") == latest["tag"]
            or version(latest["tag"]) <= version(current_version(release))
        ):
            return
        print(
            f"Codex Jev update available: {current_version(release)} → {latest['tag'].removeprefix('v')}"
        )
        print("1. Update now\n2. Skip this time\n3. Skip this version")
        choice = input("Select [2]: ").strip()
        if choice == "3":
            state["dismissed"] = latest["tag"]
            write_state(state)
        elif choice == "1":
            check_trust(release)
            install_release(latest)
            launcher = Path.home() / ".local/bin/codex-jev"
            os.execv(launcher, [str(launcher), *args])
    except (
        OSError,
        ValueError,
        KeyError,
        TypeError,
        EOFError,
        tarfile.TarError,
        subprocess.SubprocessError,
    ) as error:
        print(
            f"Jev update unavailable: {error}. Continuing with the installed build.",
            file=sys.stderr,
        )


def update_command(release, args):
    parser = argparse.ArgumentParser(prog="codex-jev update")
    parser.add_argument("--check", action="store_true", help="Check without installing")
    parser.add_argument(
        "--yes", action="store_true", help="Install the latest newer fork release"
    )
    options = parser.parse_args(args)
    try:
        if platform.system() != "Linux" or platform.machine() not in (
            "x86_64",
            "AMD64",
        ):
            raise ValueError("The Jev release updater currently supports Linux x86_64")
        state = refresh(release, force=True)
        latest = state.get("latest")
        if not latest or version(latest["tag"]) <= version(current_version(release)):
            print("Codex Jev is up to date.")
            return 0
        print("Available Jev release: " + latest["tag"])
        if options.check:
            return 0
        if (
            options.yes
            or input("Install this fork update? [y/N] ").strip().casefold() == "y"
        ):
            check_trust(release)
            install_release(latest)
        return 0
    except (
        OSError,
        ValueError,
        KeyError,
        TypeError,
        EOFError,
        tarfile.TarError,
        subprocess.SubprocessError,
    ) as error:
        print(f"Jev update failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    if sys.argv[1:] == ["--refresh"]:
        try:
            refresh(Path(__file__).resolve().parent)
        except (
            OSError,
            ValueError,
            KeyError,
            TypeError,
            subprocess.SubprocessError,
        ) as error:
            state = read_state()
            state.update(checkedAt=time.time(), error=str(error)[:500])
            write_state(state)
