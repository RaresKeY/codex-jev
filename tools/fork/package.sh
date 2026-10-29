#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
package="$root/.build/release/codex-jev-linux-x86_64"
profile="${CODEX_JEV_BUILD_PROFILE:-dev-small}"
case "$profile" in dev-small|release) ;; *) printf 'Unsupported packaging profile.\n' >&2; exit 1 ;; esac
source_bin="$root/.build/target/$profile"
[[ -x "$source_bin/codex-code-mode-host" && -x "$source_bin/codex" ]] || { printf 'Build both binaries first.\n' >&2; exit 1; }
mkdir -p "$package"
install -m 755 "$source_bin/codex" "$source_bin/codex-code-mode-host" "$package/"
install -m 644 "$root/jev/route.py" "$package/jev-route.py"
install -m 644 "$root/jev/jev_budget.py" "$package/jev_budget.py"
install -m 644 "$root/jev/policy.json" "$package/policy.json"
install -m 644 "$root/LICENSE" "$root/NOTICE" "$package/"
install -m 755 "$root/tools/fork/install.sh" "$package/install-source.sh"
install -m 644 "$root/tools/fork/jev_launcher.py" "$root/tools/fork/jev_session.py" "$root/tools/fork/jev_update.py" "$package/"
install -m 644 "$root/tools/fork/ca-trust/verify_trust.py" "$root/tools/fork/ca-trust/incident-policy.json" "$package/"
git -C "$root" rev-parse --short=12 HEAD > "$package/VERSION"
python3 - "$root" "$package" "$profile" <<'METADATA'
import json
from pathlib import Path
import subprocess
import sys
import tomllib
root, package = Path(sys.argv[1]), Path(sys.argv[2])
version = tomllib.loads((root / "codex-rs/Cargo.toml").read_text())["workspace"]["package"]["version"]
actual = subprocess.check_output([str(package / "codex"), "--version"], text=True).strip()
if actual != "codex-cli " + version:
    raise SystemExit("Built binary does not match the checkout version")
(package / "RELEASE.json").write_text(json.dumps({"version": version, "revision": (package / "VERSION").read_text().strip(), "profile": sys.argv[3], "target": "linux-x86_64"}) + "\n")
METADATA
cat > "$package/install.sh" <<'INSTALL'
#!/usr/bin/env bash
set -euo pipefail
package=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
export CODEX_JEV_DIST="$package"
exec "$package/install-source.sh" "$@"
INSTALL
chmod 755 "$package/install.sh"
tar -C "$root/.build/release" -czf "$root/.build/release/codex-jev-linux-x86_64.tar.gz" codex-jev-linux-x86_64
cd "$root/.build/release"
sha256sum codex-jev-linux-x86_64.tar.gz > SHA256SUMS
