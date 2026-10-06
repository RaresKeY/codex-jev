#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 0 && ( $# != 2 || ${1:-} != --key-file ) ]]; then
  printf 'Usage: %s [--key-file /path/to/external.env]\n' "$0" >&2
  exit 1
fi
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
source_bin="${CODEX_JEV_DIST:-$root/.build/target/dev-small}"
if [[ -n "${CODEX_JEV_DIST:-}" ]]; then
  revision=$(< "$source_bin/VERSION")
  helper="$source_bin/jev-route.py"
  policy="$source_bin/policy.json"
  notices="$source_bin"
else
  revision=$(git -C "$root" rev-parse --short=12 HEAD)
  helper="$root/jev/route.py"
  policy="$root/jev/policy.json"
  notices="$root"
fi
[[ $revision =~ ^[0-9a-fA-F]{7,40}$ ]] || { printf 'Invalid source revision.\n' >&2; exit 1; }
for binary in codex codex-code-mode-host; do
  [[ -x "$source_bin/$binary" ]] || { printf 'Build %s first with tools/fork/build.sh\n' "$binary" >&2; exit 1; }
done
install_root="${XDG_DATA_HOME:-$HOME/.local/share}/codex-jev/releases/$revision"
mkdir -p "$install_root" "$HOME/.local/bin"
install -m 755 "$source_bin/codex" "$install_root/codex-jev-native"
install -m 755 "$source_bin/codex-code-mode-host" "$install_root/codex-code-mode-host"
install -m 644 "$helper" "$install_root/jev-route.py"
install -m 644 "$policy" "$install_root/policy.json"
install -m 644 "$notices/LICENSE" "$notices/NOTICE" "$install_root/"
ln -sfnT "$install_root" "${XDG_DATA_HOME:-$HOME/.local/share}/codex-jev/current"
cat > "$HOME/.local/bin/codex-jev" <<'LAUNCHER'
#!/usr/bin/env bash
set -euo pipefail
key_pointer="${XDG_CONFIG_HOME:-$HOME/.config}/codex-jev/key-file"
if [[ -z "${CODEX_JEV_KEY_FILE:-}" && -f "$key_pointer" ]]; then
  IFS= read -r CODEX_JEV_KEY_FILE < "$key_pointer"
  export CODEX_JEV_KEY_FILE
fi
exec "${XDG_DATA_HOME:-$HOME/.local/share}/codex-jev/current/codex-jev-native" "$@"
LAUNCHER
chmod 755 "$HOME/.local/bin/codex-jev"
if [[ ${1:-} == --key-file ]]; then
  [[ $# == 2 && -f $2 ]] || { printf 'Supply an existing external key file.\n' >&2; exit 1; }
  config="${XDG_CONFIG_HOME:-$HOME/.config}/codex-jev"
  mkdir -p "$config"
  chmod 700 "$config"
  realpath -- "$2" > "$config/key-file"
  chmod 600 "$config/key-file"
fi
printf 'Installed codex-jev (%s) to %s\n' "$revision" "$HOME/.local/bin/codex-jev"
