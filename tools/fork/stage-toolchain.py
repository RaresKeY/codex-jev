#!/usr/bin/env python3
"""Stage only reviewed public image inputs; never download or run installers."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wheel-directory", type=Path, required=True)
    parser.add_argument("--artifact-directory", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    destination = args.destination.resolve()
    if destination.exists():
        raise ValueError("Use a fresh staging directory")
    destination.mkdir(parents=True, mode=0o700)
    shutil.copytree(ROOT / "ca-trust", destination / "ca-trust")
    subprocess.run(
        [
            sys.executable,
            "-I",
            str(ROOT / "ca-trust/stage_image_inputs.py"),
            "--wheel-directory",
            str(args.wheel_directory),
            "--destination",
            str(destination / "ca-trust"),
        ],
        check=True,
    )
    tools = destination / "build-tools"
    tools.mkdir()
    for name, pin in json.loads((ROOT / "build-tools.json").read_text()).items():
        source = args.artifact_directory / name
        if (
            hashlib.file_digest(source.open("rb"), "sha256").hexdigest()
            != pin["sha256"]
        ):
            raise ValueError("Build tool checksum mismatch: " + name)
        output = tools / (
            "bazel"
            if name.startswith("bazel-")
            else "just"
            if name.startswith("just-")
            else "uv"
        )
        if "member" in pin:
            with tarfile.open(source) as archive:
                member = archive.getmember(pin["member"])
                if not member.isfile() or member.size > 128 * 1024 * 1024:
                    raise ValueError("Unexpected executable member")
                with archive.extractfile(member) as reader, output.open("wb") as writer:
                    shutil.copyfileobj(reader, writer)
        else:
            shutil.copyfile(source, output)
        output.chmod(0o755)
    shutil.copyfile(ROOT / "Containerfile.security", destination / "Containerfile")


if __name__ == "__main__":
    main()
