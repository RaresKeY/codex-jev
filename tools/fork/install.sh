#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 0 && ( $# != 2 || ${1:-} != --key-file ) ]]; then
  printf 'Usage: %s [--key-file /path/to/external.env]\n' "$0" >&2
  exit 1
fi
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
profile="${CODEX_JEV_BUILD_PROFILE:-dev-small}"
source_bin="${CODEX_JEV_DIST:-$root/.build/target/$profile}"
runtime="$root/tools/fork"
if [[ -n "${CODEX_JEV_DIST:-}" ]]; then
  runtime="$source_bin"
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
for input in "$helper" "$policy" "$notices/LICENSE" "$notices/NOTICE"; do
  [[ -r "$input" && -f "$input" ]] || { printf 'Missing install input: %s\n' "$input" >&2; exit 1; }
done
for input in jev_launcher.py jev_session.py jev_update.py; do
  [[ -f "$runtime/$input" ]] || { printf 'Missing launcher input: %s\n' "$input" >&2; exit 1; }
done
trust="$runtime/ca-trust"
[[ -d "$trust" ]] || trust="$runtime"
for input in verify_trust.py incident-policy.json; do
  [[ -f "$trust/$input" ]] || { printf 'Missing trust input: %s\n' "$input" >&2; exit 1; }
done
key_file_path=
if [[ ${1:-} == --key-file ]]; then
  [[ -f $2 ]] || { printf 'Supply an existing external key file.\n' >&2; exit 1; }
  IFS= read -r -d '' key_file_path < <(realpath --zero -- "$2")
  [[ $key_file_path != *$'\n'* ]] || { printf 'Key file paths cannot contain newlines.\n' >&2; exit 1; }
fi
data_root="${XDG_DATA_HOME:-$HOME/.local/share}/codex-jev"
mkdir -p "$data_root/releases" "$HOME/.local/bin"
exec 9>"$data_root/install.lock"
flock 9
# A dirty rebuild can share HEAD with the running release. Never overwrite it.
install_root=$(mktemp -d "$data_root/releases/$revision-XXXXXXXX")
launcher_tmp=
launcher_backup=
launcher_published=false
current_tmp=
key_tmp=
key_backup=
key_published=false
published=false
cleanup() {
  # A signal can arrive between publishing the symlink and setting the flag.
  if [[ $published == false && $(readlink -- "$data_root/current" || true) != "$install_root" ]]; then
    if [[ $key_published == true ]]; then
      if [[ -n "$key_backup" ]]; then
        mv -fT -- "$key_backup" "$config/key-file"
      else
        rm -f -- "$config/key-file"
      fi
    fi
    if [[ $launcher_published == true ]]; then
      if [[ -n "$launcher_backup" ]]; then
        mv -fT -- "$launcher_backup" "$HOME/.local/bin/codex-jev"
      else
        rm -f -- "$HOME/.local/bin/codex-jev"
      fi
    fi
    rm -rf -- "$install_root"
  fi
  for temporary in "$launcher_tmp" "$launcher_backup" "$current_tmp" "$key_tmp" "$key_backup"; do
    [[ -z "$temporary" ]] || rm -f -- "$temporary"
  done
}
trap cleanup EXIT
chmod 755 "$install_root"
install -m 755 "$source_bin/codex" "$install_root/codex-jev-native"
install -m 755 "$source_bin/codex-code-mode-host" "$install_root/codex-code-mode-host"
install -m 644 "$helper" "$install_root/jev-route.py"
install -m 644 "$policy" "$install_root/policy.json"
install -m 644 "$notices/LICENSE" "$notices/NOTICE" "$install_root/"
install -m 644 "$runtime/jev_launcher.py" "$runtime/jev_session.py" "$runtime/jev_update.py" "$install_root/"
install -m 644 "$trust/verify_trust.py" "$trust/incident-policy.json" "$install_root/"
if [[ -f "$source_bin/RELEASE.json" ]]; then
  install -m 644 "$source_bin/RELEASE.json" "$install_root/"
else
  python3 - "$source_bin/codex" "$revision" "$install_root/RELEASE.json" "$profile" <<'METADATA'
import json
from pathlib import Path
import subprocess
import sys
version = subprocess.check_output([sys.argv[1], "--version"], text=True).strip().removeprefix("codex-cli ")
Path(sys.argv[3]).write_text(json.dumps({"version": version, "revision": sys.argv[2], "profile": sys.argv[4], "target": "linux-x86_64"}) + "\n")
METADATA
fi
launcher_tmp=$(mktemp "$HOME/.local/bin/.codex-jev.XXXXXXXX")
cat > "$launcher_tmp" <<'LAUNCHER'
#!/usr/bin/env bash
set -euo pipefail
key_pointer="${XDG_CONFIG_HOME:-$HOME/.config}/codex-jev/key-file"
if [[ -z "${CODEX_JEV_KEY_FILE:-}" && -f "$key_pointer" ]]; then
  IFS= read -r CODEX_JEV_KEY_FILE < "$key_pointer"
  export CODEX_JEV_KEY_FILE
fi
exec python3 "${XDG_DATA_HOME:-$HOME/.local/share}/codex-jev/current/jev_launcher.py" "$@"
LAUNCHER
chmod 755 "$launcher_tmp"
if [[ -e "$HOME/.local/bin/codex-jev" || -L "$HOME/.local/bin/codex-jev" ]]; then
  launcher_backup=$(mktemp "$HOME/.local/bin/.codex-jev-backup.XXXXXXXX")
  cp -pP -- "$HOME/.local/bin/codex-jev" "$launcher_backup"
fi
if [[ ${1:-} == --key-file ]]; then
  config="${XDG_CONFIG_HOME:-$HOME/.config}/codex-jev"
  mkdir -p "$config"
  chmod 700 "$config"
  key_tmp=$(mktemp "$config/.key-file.XXXXXXXX")
  printf '%s\n' "$key_file_path" > "$key_tmp"
  chmod 600 "$key_tmp"
  if [[ -e "$config/key-file" || -L "$config/key-file" ]]; then
    key_backup=$(mktemp "$config/.key-file-backup.XXXXXXXX")
    cp -pP -- "$config/key-file" "$key_backup"
  fi
fi
current_tmp=$(mktemp "$data_root/.current.XXXXXXXX")
ln -sfnT "$install_root" "$current_tmp"
if [[ -n "$key_tmp" ]]; then
  key_published=true
  mv -fT -- "$key_tmp" "$config/key-file"
fi
launcher_published=true
mv -fT -- "$launcher_tmp" "$HOME/.local/bin/codex-jev"
mv -fT -- "$current_tmp" "$data_root/current"
published=true
printf 'Installed codex-jev (%s) to %s\n' "$revision" "$HOME/.local/bin/codex-jev"
