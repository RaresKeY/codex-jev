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
`build.sh` uses a locked dev-small build for CLI and code mode host. The current
build sets `opt-level = 3`; the initial prerelease archive used 0 before replacement. No
dependencies were added by the routing patch. The upstream release tag's Cargo.lock
used local workspace version 0.0.0; fork lock entries are synchronized to the fork version.

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

The local optimization comparison changes only the dev-small compiler optimization
level, from 0 to 3. Application code, dependency versions, profile inheritance,
debug settings and LTO settings are unchanged. Source commit `668972fbd27add7d91cefe936323a645ce21e877` and its optimized binaries
are published. The existing v0.160.0-jev.1 Linux archive and SHA256SUMS were replaced
with this build; release notes identify its exact source revision. Streaming responsiveness is awaiting the
user's manual comparison, with the prior installed revision retained as a baseline.

## Incident build trust

`tools/fork/build-toolchain.sh` prepares an explicit candidate from the installed immutable Rust1.95 builder, using only reviewed offline pip/certifi/Bazel9.0.0/just1.59.0/uv0.13.4 input hashes. Signed APT refreshes CA/OpenSSL and supplies Java. Python stores and future pip seeds are checked before use. Bazel selects the regenerated distro JKS explicitly, rather than its embedded JRE's old trust file; actual native Java and Cargo TLS require positive/negative acceptance before registry operations. Rustup uses its documented curl backend against the repaired store and automatic toolchain installs are disabled. See https://blog.rust-lang.org/2025/03/02/Rustup-1.28.0/.

The builder removes its temporary input context on success or failure. Keep the
selected toolchain image and recorded input hashes; remove superseded candidates
by exact identity under the exclusive build-retention lock after acceptance.

The disposable builder's HOME and Java user.home point to the ignored
`.build/home` outside the Cargo workspace. Bazel repository caches must not be
created inside `codex-rs`: generated Rust tool manifests there inherit the wrong
workspace and break lock generation. Build caches remain disposable.

The normal Rust wrapper fails closed when the reviewed image is missing, performs no registry pull, preserves the existing owning checkout and runs two build workers. The candidate can be selected through CODEX_JEV_BUILDER_IMAGE before default-tag acceptance. No host packages or credentials are installed. Cargo and Bazel locks must be regenerated together; tests and paid inference remain outside this incident build operation.

The2026-10-25 rollout completed the locked optimized CLI/code-mode-host build in95m01s with two Cargo workers. Paired locks select rustls-webpki0.103.16 and webpki-roots1.0.10 with1470 other package entries preserved. Actual production doctor HEAD/GET transport rejected all14 live incident hosts; the PyPI HTTPS control completed with its expected HTTP404 route response. These were isolated credential-free diagnostics, with no inference or unit-test run.

Only codex-jev was installed through `tools/fork/install.sh` using a curated `CODEX_JEV_DIST`. The current release directory is `~/.local/share/codex-jev/releases/0e6ce6cd8aca`, identifying the source base plus the accepted security overlay; private install receipts bind exact source-file and binary hashes. Native CLI SHA256 is `5533938d73b7030d2bf909202a7d6ab881aaaabf6bef5e2d7d6904e2333ef925`; code-mode-host SHA256 is `ccbd7ef5b05bb98fea9d874f6b13ea0141c90795fa7ca9ef3aaf65d1355544d2`. The already installed route helper, policy and notices were copied byte-for-byte rather than deploying unrelated working changes. The external key pointer and upstream codex remain unchanged; prior installed release668972fbd27a remains for comparison. Task-owned build/package caches are disposable after installed hash acceptance; the selected builder is retained.
