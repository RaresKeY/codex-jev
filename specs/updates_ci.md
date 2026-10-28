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

The fork additionally suppresses upstream upgrade offers for `-jev.` versions in
`codex-rs/tui/src/updates.rs`. There is currently no fork release-feed updater.
Fork upgrades use the project installer, independently of upstream `codex`.

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
optional `native` input
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
release outputs. The current workflow and `package.sh` produce development
candidates from `dev-small`; they do not implement release publication.

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
