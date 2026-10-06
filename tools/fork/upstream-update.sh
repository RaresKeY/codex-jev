#!/usr/bin/env bash
# Fetch and integrate one explicitly selected upstream tag; conflicts remain for review.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
[[ $# == 1 && $1 == rust-v* ]] || { printf 'Usage: %s rust-vVERSION\n' "$0" >&2; exit 1; }
[[ -z $(git -C "$root" status --porcelain) ]] || { printf 'Commit or preserve existing work first.\n' >&2; exit 1; }
git -C "$root" fetch https://github.com/openai/codex.git "refs/tags/$1:refs/tags/$1"
git -C "$root" merge --no-ff "$1" -m "Merge upstream $1"
printf 'Reconcile fork version, lockfile, Jev hooks and specs, then build before installing.\n'
