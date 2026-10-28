#!/usr/bin/env bash
# Extend the reviewed local builder without pulling or replacing its shared base.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
[[ $# == 2 && $1 == --artifact-directory ]] || { printf 'Usage: %s --artifact-directory PATH\n' "$0" >&2; exit 2; }
stage=$(mktemp -d "$root/.build/toolchain-upgrade.XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
python3 -I - "$root" "$2" "$stage" <<'PY'
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tarfile
root, artifacts, stage = map(Path, sys.argv[1:])
pins = json.loads((root / "tools/fork/build-tools.json").read_text())
name = "bazel-9.2.0-linux-x86_64"
with (artifacts / name).open("rb") as stream:
    if hashlib.file_digest(stream, "sha256").hexdigest() != pins[name]["sha256"]:
        raise ValueError("Bazel checksum mismatch")
(stage / "build-tools").mkdir()
shutil.copyfile(artifacts / name, stage / "build-tools/bazel")
name = "dotslash-linux-musl.x86_64.v0.5.9.tar.gz"
with (artifacts / name).open("rb") as stream:
    if hashlib.file_digest(stream, "sha256").hexdigest() != pins[name]["sha256"]:
        raise ValueError("DotSlash checksum mismatch")
with tarfile.open(artifacts / name) as archive:
    member = archive.getmember(pins[name]["member"])
    if not member.isfile() or member.size > 32 * 1024 * 1024:
        raise ValueError("Unexpected DotSlash executable")
    with archive.extractfile(member) as reader, (stage / "build-tools/dotslash").open("wb") as writer:
        shutil.copyfileobj(reader, writer)
shutil.copyfile(root / "tools/fork/Containerfile.toolchain", stage / "Containerfile")
PY
base=$(podman image inspect localhost/codex-jev-builder:rust-1.95-ca --format '{{.Id}}')
[[ $base =~ ^[a-f0-9]{64}$ ]]
exec 9>"${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/podman-build-retention.lock"
flock --shared 9
podman build --pull=never --network private --build-arg "REVIEWED_BASE_IMAGE=$base" \
  --label io.rareskey.retention=retain --label io.rareskey.project=codex-jev --label io.rareskey.purpose=rust-build-toolchain \
  --layer-label io.rareskey.retention=ephemeral --layer-label io.rareskey.project=codex-jev \
  -t localhost/codex-jev-builder:rust-1.99.1-ca -f "$stage/Containerfile" "$stage"
