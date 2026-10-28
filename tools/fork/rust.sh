#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
exec 9>"${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/podman-build-retention.lock"
flock -s 9
image=${CODEX_JEV_BUILDER_IMAGE:-localhost/codex-jev-builder:rust-1.99.1-ca}
podman image exists "$image" || { echo "Missing reviewed toolchain image; prepare the CA-reviewed base with tools/fork/build-toolchain.sh, then run tools/fork/upgrade-toolchain.sh with verified artifacts." >&2; exit 1; }
mkdir -p "$root/.build/cargo" "$root/.build/target" "$root/.build/home"
build_env=()
if [[ ${1:-} == cargo && ${2:-} =~ ^(build|check|clippy|nextest)$ ]] \
  || [[ ${1:-} == just && ${2:-} =~ ^(test|fix)$ ]]; then
  v8_lines=$(python3 "$root/tools/fork/v8-env.py")
  while IFS= read -r setting; do build_env+=(-e "$setting"); done <<< "$v8_lines"
fi
podman run --rm --name "codex-jev-build-$$" --userns=keep-id --user "$(id -u):$(id -g)" \
  --passwd-entry '$USERNAME:*:$UID:$GID:$NAME:/source/.build/home:/bin/sh' \
  --label io.rareskey.project=codex-jev --label io.rareskey.purpose=build \
  -v "$root:/source:rw" -w /source/codex-rs \
  -e HOME=/source/.build/home \
  -e CARGO_HOME=/source/.build/cargo -e CARGO_TARGET_DIR=/source/.build/target \
  -e CARGO_BUILD_JOBS="${CODEX_JEV_BUILD_JOBS:-2}" -e CARGO_INCREMENTAL=0 \
  -e RUSTUP_TOOLCHAIN=1.99.1 "${build_env[@]}" "$image" "$@"
