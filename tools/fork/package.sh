#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
package="$root/.build/release/codex-jev-linux-x86_64"
[[ -x "$root/.build/target/dev-small/codex-code-mode-host" ]] || { printf 'Build both binaries first.\n' >&2; exit 1; }
mkdir -p "$package"
install -m 755 "$root/.build/target/dev-small/codex" "$root/.build/target/dev-small/codex-code-mode-host" "$package/"
install -m 644 "$root/jev/route.py" "$package/jev-route.py"
install -m 644 "$root/jev/policy.json" "$package/policy.json"
install -m 644 "$root/LICENSE" "$root/NOTICE" "$package/"
install -m 755 "$root/tools/fork/install.sh" "$package/install-source.sh"
git -C "$root" rev-parse --short=12 HEAD > "$package/VERSION"
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
