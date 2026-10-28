# Build and fork maintenance

Upstream base: openai/codex rust-v0.165.0, commit
e374836b2728d9b7aa981c2902649e640a2edaa8. Fork version 0.165.0-jev.1.
Apache 2.0 LICENSE, NOTICE and existing third party material are retained.

Native builds and native regressions default to standard GitHub-hosted Actions
runners; heavy local compilation requires an explicit user request. Development
uses low optimization (`dev-small` level 1, or `ci-test` level 0 for test
iteration). Published releases require the release profile with level 3 and thin
LTO, plus packaging from the matching output directory. `CODEX_JEV_BUILD_PROFILE`
selects the matching build/package profile and defaults to `dev-small`.
See [updates_ci.md](updates_ci.md)
for CI activation, artifact retention and the distinction between candidates and
release builds.

`tools/fork/rust.sh` owns rootless Podman execution, a shared retention flock,
keep-id user mapping, task container labels, disposable containers, two default
build workers, ignored Cargo/target directories, and Rust 1.99.1. The builder image
is retained and recorded by image identity; intermediate layers are explicitly ephemeral.
`v8-env.py` reuses upstream codex_package.v8 to download the Codex sandbox V8
archive/binding pair and verify its checksum against the committed manifest.
`build.sh` uses a locked dev-small build for CLI and code mode host. The current
build sets `opt-level = 1` for moderate optimization and shorter local rebuilds;
the previous comparison build used 3. It inherits the dev profile, with no explicit
release LTO, no debug symbols and stripped binaries. No
dependencies were added by the routing patch. The upstream release tag's Cargo.lock
used local workspace version 0.0.0; fork lock entries are synchronized to the fork version.

`install.sh` installs only codex-jev under the user's .local/bin and versioned
XDG data directory, plus code-mode-host, route helper, policy and license notices.
The Bash entry point loads an optional external key-file pointer from XDG config
and invokes the installed Python launcher. The launcher offers opt-in fork
updates and supervises private foreground servers for `--no-daemon` sessions. It
shares Codex's default home unless CODEX_HOME is explicitly supplied. The install
is separate from upstream CLI package management. Fork update offers never point
to OpenAI's release/package install actions.

The installer validates all payload files and any supplied key-file path before
changing the installation. Each install uses a unique directory prefixed with the
source revision, so reinstalling a dirty build cannot overwrite a running binary.
It stages the complete payload before atomically replacing the launcher and
`current` symlink. Failed staging removes its candidate and preserves the active
release; prior installed releases remain available.
Every payload also carries the launcher/update/session modules, the incident
trust verifier and policy, and `RELEASE.json` with version/profile/target identity.
Installer publication is serialized with a project-local flock. Launcher and
optional key-pointer backups are staged before publication and restored if the
final `current` switch fails; failed candidates and temporary files are removed.
Canonical key paths are read with a NUL delimiter so trailing newlines, including
those in symlink targets, are preserved and rejected before publication.

`upstream-update.sh` fetches and merges one explicitly specified upstream tag;
it requires clean state and preserves conflicts for resolution. Reconcile the
fork version, local workspace lock versions, disabled workflow locations, helper
packaging, model picker/submit hooks and routing contract before building and
installing a new revision. Upstream workflows are preserved as disabled sources.
The installer retains current installed runtimes; build intermediates are reproducible.

The effort refinement uses policy `2026-10-28-v6.3-effort` with unchanged native
binaries reused by verified hash from `c00fed32d84f-trLnVwof`. The owning installer
stages a complete payload and switches to a new release; the prior context release,
upstream executable and external key pointer are preserved. The new receipt links
the inherited native build/source provenance and records the refined helper and
policy hashes. No native rebuild or native test repetition is needed for this
Python/policy-only change. All 13 offline Python tests pass; the 88-call live
comparison uses 223,119 reported Jev tokens, with effort matches improving from
30/44 to 38/44. Six effort misses and two unchanged model misses remain. Full
formatting, semantic equivalence to the live-tested helper and payload checks are
recorded with installation. See [the effort comparison](../reports/jev-effort-comparison-2026-10-28.md).

The preceding context-routing update uses policy `2026-10-28-v6.2-context` and the existing
reviewed Rust 1.99.1 builder, locked offline dependencies and four build workers.
`~/.local/share/codex-jev/current/BUILD_RECEIPT.json` identifies the selected
runtime and matching source/payload hashes; the preceding 0.165 runtime below
is retained. The rollout preserves the upstream executable and external key pointer.

Qualification passed all 19 focused native Jev tests and 11 Python routing tests,
scoped Clippy with `-D warnings`, full `just fmt` and `git diff --check`.
Native cases cover bounded server RPC, production fixture projection, UTF-8 and
JSON expansion, disclosure, cancellation and draft recovery. The 48-call live
old/new helper comparison used 109,321 reported Jev tokens. Model-rubric matches
were 12/24 versus 24/24; combined model/effort matches were 12/24 versus 21/24.
Three candidate efforts overshot to `max`. Four controls retained expected
selections. These synthetic routing measurements do not establish executor
quality. A metadata-only real-history probe returned 32 items in 0.292 seconds;
no private content was saved or sent to Jev for that probe. See
[the comparison report](../reports/jev-context-comparison-2026-10-28.md).

The preceding 0.165 local runtime was
`~/.local/share/codex-jev/releases/c00fed32d84f-qQ1L8kJ9`: source base
`c00fed32d84f` on upstream `rust-v0.165.0`, plus the working-tree
snapshot in `SOURCE_MANIFEST.json`. All 9,103 source hashes and six installed
payload hashes were verified. `BUILD_RECEIPT.json` records the reviewed Rust
1.99.1 builder, locked `dev-small` profile, `opt-level = 1` and four workers.
Both native binaries built successfully; compatible build caches are retained.

Current qualification passed 11 offline Python tests, 57 focused Rust library
tests and 43 core retry integration tests. Full `just fmt`, scoped Clippy, TUI/core
typechecks and Bash syntax checks passed. External Cargo package versions and
checksums match upstream 0.165.0. Four model-picker snapshots were reviewed and
updated for the Jev row and upstream catalog. The capacity handshake regression
requires zero HTTP fallback requests after its retry budget is exhausted.

Eight live Jev helper requests used 16,262 provider-reported tokens
within the authorized five-million-token ceiling. They exposed and verified the
image-choice pool correction; the installed helper also selected Terra for an
image-only request. No live OpenAI executor inference was performed. The previous
0.160 runtime, upstream `codex` and external key pointer were preserved. New
launches use `codex-jev --no-daemon` to select the fork backend. Fork CI and update
ownership are documented in [updates_ci.md](updates_ci.md).

The previous 0.160 local runtime was
`~/.local/share/codex-jev/releases/f5cd3fc5466a-tY5DgfiM`: source base
`f5cd3fc5466a` plus the working-tree snapshot recorded in
`SOURCE_MANIFEST.json`. Its `BUILD_RECEIPT.json` binds the builder identity,
source manifest and exact installed payload hashes. It includes V6.1 routing,
bounded conversation context, Auto enabled by default, the subsequent launch
override/provider/Reserve and image-only fixes, installer publication fixes, and
the optional `jev-stats` status-line item.

The 2026-10-28 rollout rebuilt both native CLI and code-mode host through the
pinned Rust 1.95 rootless wrapper, locked offline dependencies, `dev-small`,
`opt-level = 1` and four workers. Compilation completed in 30m13s.
All 8,433 recorded source hashes matched after compilation. The staged and
installed payload hashes matched, the binary contains the new stats item, and
the launcher reported `codex-cli 0.160.0-jev.1`. The guarded installer deployed
the matching helper and policy through `CODEX_JEV_DIST`.

Upstream `codex`, the external key-file pointer and the previous
`f5cd3fc5466a-LJENeyZB` release were preserved. This rollout's temporary install payload
and `.build/target/dev-small` outputs were removed after installed verification;
retained tooling, V8/Cargo inputs, earlier debug cache and receipts remain. No
tests or paid inference ran for this rollout. Earlier Rust compile/format and
Python syntax checks passed; full formatting still lacks DotSlash and cached
Ruff packages. Existing processes keep their loaded build; use
`codex-jev --no-daemon` to launch the current frontend and server. Enable the
optional stats item through `/statusline`.

The 2026-10-27 review passed 43 focused Rust library tests and 11 capacity
integration tests through `just test`, six offline Python routing tests, and three
isolated installer checks (reinstall while the old executable runs, missing key
file, missing payload). Picker snapshots were reviewed and updated; the final run used normal
snapshot assertions. Rust formatting and `git diff --check` passed. Tests used
mock providers, with no paid inference or full workspace suite. The previously
recorded broad formatter tooling limits remain. Concurrent V6/context work was
preserved and its focused routing cases were included in the library test run.
Strict library Clippy checks for `codex-api`, `codex-protocol` and `codex-tui`
passed with `-D warnings`; three unused imports found during validation were
removed from core and its integration tests.

V6 routing (`2026-10-27-v6`) is implemented in the checkout, including the
four-model policy, independent effort and bounded conversation evidence. On
2026-10-27, all six Python helper tests passed (`python -m unittest -q route_tests`
from `jev/`). All nine focused Rust Jev tests passed through the pinned Rust 1.95
wrapper and `just test`, with locked offline dependencies and the filter
`package(codex-tui) & (test(jev_auto) | test(jev_route))`. Coverage includes context
bounds, disclosure, rollback, stale-context cancellation and submission recovery.
Rust formatting and `git diff --check` passed; the full `just fmt` command remains
blocked by missing DotSlash and uncached pinned Ruff packages. These are local
regression results, not live routing or executor-quality measurements. No paid
inference was performed. V6 was source-only at that review; the current rollout
above installs V6.1.

The subsequent V6.1 policy edit routes otherwise-Luna tasks with attached images
to Terra, preserving independent effort and Sol/Astra tasks. All six existing
Python helper tests and `git diff --check` passed after the edit. No native code
changed and no live classifier call was made. This edit was source-only at the
time and is included in the current installed build.

The subsequent Auto indicator fix shows only `Auto (Jev)` until a routing choice
is acknowledged, including custom model/reasoning footer items and terminal
titles. All 70 focused Jev, status-surface and terminal-title tests passed through
the same locked offline wrapper with normal snapshot assertions. The new rendered
footer snapshot was reviewed at 48- and 80-column widths before acceptance.
Rust formatting and `git diff --check` passed; the broader formatter retains the
tooling limits above. This UI correction is included in the current installed build.

The startup default now enables Auto (Jev) for new and resumed widgets; selecting
a model manually disables it for the current widget. The manual widget fixture
explicitly disables Auto. Locked offline `cargo check -p codex-tui --lib`, Rust
formatting and `git diff --check` passed. The full formatter still lacks DotSlash
and cached Ruff packages. No tests or paid inference were run for this edit.

The optional `jev-stats` footer item carries validated confidence and classifier
usage from the bundled helper and measures local routing latency. Locked offline
`cargo check -p codex-tui --tests` passed, including compilation of the new picker,
metadata-boundary and route-lifecycle cases. Rust formatting, Python syntax and
`git diff --check` passed; the full formatter retains the DotSlash/Ruff limits.
Tests were not executed for this edit and the rendered UI remains unverified.
This change is included in the current installed runtime described above,
with its matching native binary and helper.
The startup default change is included in the current installed build.

The subsequent source review fixed five issues: image-only prompts rejected as
empty text, explicit launch model/provider/effort overrides displaced by Auto,
custom-provider/Reserve and acknowledged usage fallback settings overridden by
Jev, partial installer publication changing the launcher/key pointer, and trailing
newlines lost during key-path canonicalization. Stale Auto picker actions cannot
bypass a forced Reserve state. The upstream initial-prompt fixture explicitly
uses manual submission; dedicated Jev cases exercise the real Auto constructor.

Locked offline `cargo check -p codex-tui --tests`, library Clippy with `-D warnings`,
Rust formatting, Python/shell syntax and `git diff --check` passed after the review.
The pinned builder's system and Python CA stores passed the incident guard before
adding its ephemeral Clippy component. The full formatter retains the known
DotSlash/Ruff tooling limits. New Rust startup/provider/fallback and Python
image-only/installer rollback/path regressions were added and compiled where
applicable, but no tests were executed because explicit opt-in is still pending.
No paid inference ran. These review fixes are included in the current installed
runtime above. The private source-review receipt is
`.build/review-auto-default-receipt.json`.

Initial build completed successfully on Linux x86_64 with the pinned Podman
Rust 1.95 toolchain: `tools/fork/build.sh`, dev-small, both CLI and code mode
host. That build reported an upstream unused-import warning in codex-core. Tests and
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

Only codex-jev was installed through `tools/fork/install.sh` using a curated `CODEX_JEV_DIST`. That incident build was installed under `~/.local/share/codex-jev/releases/0e6ce6cd8aca`, identifying the source base plus the accepted security overlay; private install receipts bind exact source-file and binary hashes. Native CLI SHA256 is `5533938d73b7030d2bf909202a7d6ab881aaaabf6bef5e2d7d6904e2333ef925`; code-mode-host SHA256 is `ccbd7ef5b05bb98fea9d874f6b13ea0141c90795fa7ca9ef3aaf65d1355544d2`. The already installed route helper, policy and notices were copied byte-for-byte rather than deploying unrelated working changes. The external key pointer and upstream codex remain unchanged; prior installed release668972fbd27a remains for comparison. Task-owned build/package caches are disposable after installed hash acceptance; the selected builder is retained.
