#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
exec 9>"${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/podman-build-retention.lock"
flock -s 9
image=localhost/codex-jev-builder:rust-1.95
podman image exists "$image" || podman build --layer-label io.rareskey.retention=ephemeral --layer-label io.rareskey.project=codex-jev -t "$image" -f "$root/tools/fork/Containerfile" "$root/tools/fork"
mkdir -p "$root/.build/cargo" "$root/.build/target"
build_env=()
if [[ ${1:-} == cargo && ${2:-} == build ]]; then
  v8_lines=$(python3 "$root/tools/fork/v8-env.py")
  while IFS= read -r setting; do build_env+=(-e "$setting"); done <<< "$v8_lines"
fi
podman run --rm --name "codex-jev-build-$$" --userns=keep-id --user "$(id -u):$(id -g)" \
  --label io.rareskey.project=codex-jev --label io.rareskey.purpose=build \
  -v "$root:/source:rw" -w /source/codex-rs \
  -e CARGO_HOME=/source/.build/cargo -e CARGO_TARGET_DIR=/source/.build/target \
  -e CARGO_BUILD_JOBS="${CODEX_JEV_BUILD_JOBS:-2}" -e CARGO_INCREMENTAL=0 \
  -e RUSTUP_TOOLCHAIN=1.95.0 "${build_env[@]}" "$image" "$@"
