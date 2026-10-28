#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
exec "$root/tools/fork/rust.sh" cargo build --locked --profile "${CODEX_JEV_BUILD_PROFILE:-dev-small}" --bin codex --bin codex-code-mode-host "$@"
