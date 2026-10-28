# Updates and fork CI

## Update behavior

The current source base is upstream `rust-v0.165.0`. The installed runtime is
identified separately in `build_maintenance.md`; rebasing source does not change
an already installed or running executable.

Upstream release builds refresh cached latest-version metadata in the background
when it is absent or older than 20 hours. Startup uses the previous cache, so the
next launch can show the update banner or popup. A dismissed version is suppressed
for that popup. The detected install method selects the upgrade action. Source
and debug builds skip the release update flow.

The native fork suppresses upstream upgrade offers for `-jev.` versions in
`codex-rs/tui/src/updates.rs`. The installed Python launcher owns fork updates,
independently of upstream `codex`. Interactive startup checks cached fork release
metadata and refreshes it in a background process after 20 hours. A newly fetched
release is offered on the next launch. The prompt offers Update now, Skip this
time and Skip this version; installation requires an explicit selection. Help,
version, noninteractive commands and app-server processes never prompt.

`codex-jev update --check` checks immediately; `codex-jev update` asks before
installing and `--yes` explicitly authorizes installation. The launcher honors
`check_for_update_on_startup = false` and `CODEX_JEV_CHECK_UPDATES=0`. Its XDG cache
is separate from upstream version metadata. This updater supports Linux x86_64
and requires Python 3.11+, OpenSSL and the existing installer prerequisites.

The feed is the public `RaresKeY/codex-jev` GitHub releases API, including
prereleases. Only complete, numerically newer `vX.Y.Z-jev.N` releases with the
canonical archive/checksum assets qualify. Before HTTPS, the bundled incident
trust guard inspects the active system/Python stores; failures block downloads
and are recorded for display on the next launch. TLS verification remains on,
and redirects must retain HTTPS. An update verifies SHA-256 before extraction,
rejects unknown, duplicate, linked or escaping members, checks release metadata
for the platform and release profile, and checks the binary's actual version.
Only then does it invoke the transactional installer. Failed updates preserve
the current installation. No local Rust build is part of updating.

Publishing a new update still requires a reviewed release with a unique fork
version and optimized artifacts. CI candidate artifacts alone do not enter the
update feed. Existing installations acquire the launcher once through the
updated installer; future published releases carry it in their payload.

## Foreground sessions and Workspaces

The installed launcher's `--no-daemon` path owns a foreground fork app server on
a private Unix socket and connects a matching native TUI to it. The supervisor
reaps both processes when the TUI exits; it does not register, replace or restart
the shared daemon. Configuration/feature overrides reach the server and prompt
arguments remain literal. Native direct execution retains upstream's embedded
`--no-daemon` behavior.

Session discovery uses `$XDG_RUNTIME_DIR/codex-jev/sessions/s-*/session.json`.
The directories are private and manifests contain only PID/start-time identity,
socket path and cwd. Workspaces verifies ownership, modes, socket type, matching
native executable identity and all three process start times before connecting.
It merges loaded root-thread metadata from these servers with the existing
shared catalog and sends explicit renames to the owning connection. No transcript
subscription, prompt submission, daemon restart or terminal-input injection is
needed. Stale manifests cannot select a reused PID or an arbitrary socket.

## Maintenance

Integrate selected stable upstream tags with local review of Jev startup,
submission, model picker, settings acknowledgements, capacity retries and update
handling. Keep the paired Cargo/Bazel locks, Rust toolchain and specs consistent.
The explicit `tools/fork/upstream-update.sh rust-vVERSION` helper performs a merge
and requires a clean tree. A requested rebase can instead replay the fork commits;
published history then requires deliberate reconciliation before any push.

Build and regression automation does not establish live routing quality. Live
Jev checks are separate, explicitly authorized operations with recorded token
usage; CI has no inference credentials or live provider calls.

## Fork workflow

Native compilation and native regressions belong on standard GitHub-hosted
Actions runners by default, leaving the workstation CPU available. Heavy local
compilation requires an explicit user request; an unavailable workflow does not
authorize a local fallback. Lightweight source checks can remain local, subject
to the existing test authorization rule. CI must build the revision containing
the intended changes; an older remote revision cannot validate a dirty checkout.

`.github/workflows/jev-ci.yml` defines Rust formatting and offline helper and
installer checks for pushes to `main`, pull requests and manual dispatch. Its
optional `native` input and `profile` choice (`dev-small` or `release`)
adds Linux Rust regressions, both native binaries and a candidate archive retained
for seven days. Native validation is manual and has a six-hour timeout to bound
cold-build cost.

The workflow has read-only repository permissions. Upstream workflows remain in
`workflows.disabled`. It does not rebase, push branches, open pull requests,
publish releases or install candidates. The manual button becomes available
after the workflow is on the default branch; adding a local file does not activate
hosted CI.

Development candidates use `dev-small` (`opt-level = 1`, no debug symbols);
`ci-test` (`opt-level = 0`) is available for faster test iteration. Published
release builds must use `--profile release`, with Cargo's default optimization
level 3, thin LTO and four codegen units. Packaging must select the matching
release outputs. `CODEX_JEV_BUILD_PROFILE` selects matching build/package inputs;
it defaults to `dev-small`. The workflow produces candidates from the selected
profile and does not publish releases. Archives include `RELEASE.json` and the
launcher, updater, foreground supervisor and trust guard alongside both binaries.

Standard GitHub-hosted runner compute is free for public repositories. Larger
runners are billed, and artifact/cache storage has separate allowances. Keep
the current seven-day candidate retention and check storage usage as builds
accumulate. See [GitHub Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions)
and [runner resources](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

## Toolchain

`tools/fork/rust.sh` selects the reviewed local Rust 1.99.1 builder and prepares
checksum-pinned V8 inputs for checks, builds, Clippy and focused tests. The default
remains two workers. `upgrade-toolchain.sh --artifact-directory PATH` extends the
reviewed Rust 1.95 CA-repaired builder with Rust 1.99.1 and a hash-verified Bazel
9.2.0 input and DotSlash 0.5.9. It preserves shared base layers and validates trust
before downloads. DotSlash delegates HTTPS to curl using the selected system CA
store; the formatter can reuse cached Python tools with `UV_OFFLINE=1`.
The bootstrap security-image path remains available for rebuilding the reviewed
base from its independently verified wheel/tool inputs.
