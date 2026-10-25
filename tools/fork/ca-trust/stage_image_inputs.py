#!/usr/bin/env python3
"""Stage only reviewed public bootstrap wheels and their verified transport CA."""

import argparse
import hashlib
from pathlib import Path
import shutil
import zipfile

PIP = "pip-26.2.2-py3-none-any.whl"
CERTIFI = "certifi-2026.10.19-py3-none-any.whl"
HASHES = {
    PIP: "fd7f4dfa8190e014013b9f08f3e973f4e4c26ffd179a204895533a7254813a70",
    CERTIFI: "42d35340cc5b9ba78e818ddbff0c5b1c2b779de448a340fd1b989f5fa8670e1b",
}
CA_HASH = "50f79f98dac192b20dbf73f35c56f0b0c557ab267ff5199d1117cc1180b70785"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wheel-directory", type=Path)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--clean", action="store_true")
    args = parser.parse_args()
    dest = args.destination
    for parent in [dest, *dest.parents]:
        if parent.is_symlink():
            raise ValueError("Refusing symlink staging path")
    paths = {dest / "wheels" / name: value for name, value in HASHES.items()}
    paths[dest / "transport.crt"] = CA_HASH
    if args.clean:
        for path, expected in paths.items():
            if path.exists():
                if path.is_symlink() or sha(path) != expected:
                    raise ValueError("Preserve changed bootstrap input")
        for path in paths:
            if path.exists():
                path.unlink()
        for directory in [dest / "wheels", dest]:
            if directory.is_dir() and not any(directory.iterdir()):
                directory.rmdir()
        return
    if not args.wheel_directory:
        parser.error("--wheel-directory required")
    for name, expected in HASHES.items():
        if sha(args.wheel_directory / name) != expected:
            raise ValueError("Bootstrap wheel checksum mismatch")
    dest.mkdir(parents=True, exist_ok=True, mode=0o700)
    (dest / "wheels").mkdir(exist_ok=True, mode=0o700)
    for path, expected in paths.items():
        if path.exists() and (path.is_symlink() or sha(path) != expected):
            raise ValueError("Refusing unrelated staging input")
    for name in HASHES:
        shutil.copyfile(args.wheel_directory / name, dest / "wheels" / name)
    with zipfile.ZipFile(args.wheel_directory / CERTIFI) as archive:
        data = archive.read("certifi/cacert.pem")
    if hashlib.sha256(data).hexdigest() != CA_HASH:
        raise ValueError("Bootstrap transport CA identity mismatch")
    (dest / "transport.crt").write_bytes(data)
    print("Staged hash-reviewed public CA/installer inputs")


if __name__ == "__main__":
    main()
