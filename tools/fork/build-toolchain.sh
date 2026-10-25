#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
[[ $# == 4 && $1 == --wheel-directory && $3 == --artifact-directory ]] || { echo 'Usage: build-toolchain.sh --wheel-directory PATH --artifact-directory PATH' >&2; exit 2; }
mkdir -p "$root/.build"
stage=$(mktemp -d "$root/.build/toolchain-stage.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
rmdir "$stage"
python3 -I "$root/tools/fork/stage-toolchain.py" --wheel-directory "$2" --artifact-directory "$4" --destination "$stage"
base=$(podman image inspect localhost/codex-jev-builder:rust-1.95 --format '{{.Id}}')
[[ "$base" =~ ^[a-f0-9]{64}$ ]] || exit 1
exec 9>"${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/podman-build-retention.lock"
flock --shared 9
podman build --pull=never --network private --build-arg "SECURITY_BASE_IMAGE=$base" \
 --label io.rareskey.retention=retain --label io.rareskey.project=codex-jev --label io.rareskey.purpose=rust-build-toolchain \
 --layer-label io.rareskey.retention=ephemeral --layer-label io.rareskey.project=codex-jev \
 -t localhost/codex-jev-builder:rust-1.95-ca -f "$stage/Containerfile" "$stage"
# Input identities live in build-tools.json and the reviewed wheel manifest.
# The image contains its inputs; the reproducible build context is temporary.
