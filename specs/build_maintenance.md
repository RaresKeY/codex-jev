# Build and fork maintenance

Upstream base: openai/codex rust-v0.160.0, commit
a956835d020762cb2b570053af06f643a11c0ecc. Fork version 0.160.0-jev.1.
Apache 2.0 LICENSE, NOTICE and existing third party material are retained.

`tools/fork/rust.sh` owns rootless Podman execution, a shared retention flock,
keep-id user mapping, task container labels, disposable containers, two default
build workers, ignored Cargo/target directories, and Rust 1.95. The builder image
is retained and digest-pinned; intermediate layers are explicitly ephemeral.
`v8-env.py` reuses upstream codex_package.v8 to download the Codex sandbox V8
archive/binding pair and verify its checksum against the committed manifest.
`build.sh` uses a locked dev-small build for CLI and code mode host. No dependencies
were added by the routing patch. The upstream release tag's Cargo.lock used local
workspace version 0.0.0; fork lock entries are synchronized to the fork version.

`install.sh` installs only codex-jev under the user's .local/bin and versioned
XDG data directory, plus code-mode-host, route helper, policy and license notices.
The launcher loads an optional external key-file pointer from XDG config. It
shares Codex's default home unless CODEX_HOME is explicitly supplied. The install
is separate from upstream CLI package management. Fork update offers never point
to OpenAI's release/package install actions.

`upstream-update.sh` fetches and merges one explicitly specified upstream tag;
it requires clean state and preserves conflicts for resolution. Reconcile the
fork version, local workspace lock versions, disabled workflow locations, helper
packaging, model picker/submit hooks and routing contract before building and
installing a new revision. Upstream workflows are preserved as disabled sources.
The installer retains current installed runtimes; build intermediates are reproducible.

Initial build completed successfully on Linux x86_64 with the pinned Podman
Rust 1.95 toolchain: `tools/fork/build.sh`, dev-small, both CLI and code mode
host. An upstream unused-import warning in codex-core is preserved. Tests and
paid routing were not run. `package.sh` creates one current release archive and
SHA256SUMS; publish the archive, verify remote asset digests and remove generated
local release/build outputs while retaining the installed runtime and build tools.
