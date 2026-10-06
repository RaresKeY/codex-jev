# Codex Jev

A personal, open source fork of [OpenAI Codex](https://github.com/openai/codex),
with an **Auto (Jev)** model picker option. Installed as **`codex-jev`** alongside
an ordinary Codex installation. This fork is independent of OpenAI.

Based on upstream `rust-v0.160.0` (`a956835d020762cb2b570053af06f643a11c0ecc`).
Fork version: `0.160.0-jev.1`. Upstream Apache 2.0 license and notices are preserved;
fork additions are published under the same license.

## Install the Linux build

The initial x86_64 Linux build is available in
[Releases](https://github.com/RaresKeY/codex-jev/releases/tag/v0.160.0-jev.1).
Download `codex-jev-linux-x86_64.tar.gz` and `SHA256SUMS`, then:

```bash
sha256sum -c SHA256SUMS
tar -xzf codex-jev-linux-x86_64.tar.gz
bash codex-jev-linux-x86_64/install.sh
codex-jev
```

The release is an initial prerelease. Python 3 is needed for Jev Auto; a separate
Jev API key is required. Source build instructions follow.

## Build and install

Linux build prerequisites: rootless Podman, Git, Bash and Python 3. The build script
uses a digest-pinned Rust 1.95 container and a shared build lock. Build outputs stay
in ignored `.build/`. Start the build from this checkout:

```bash
bash tools/fork/build.sh
bash tools/fork/install.sh
codex-jev
```

The container build produces the CLI and its code mode host. The installer copies
both plus the standalone Jev helper/policy into a versioned user installation.
It installs only `~/.local/bin/codex-jev`; it does not replace `codex`.
The build profile is `dev-small`, with `opt-level = 3` and no debug symbols.
The prerelease archive was updated from the original `opt-level = 0` build;
the optimization change leaves other profile settings unchanged.
Native Cargo builds are possible with the pinned toolchain, but the installer
expects the container wrapper's output directory.

## Use Auto

Run `codex-jev`, open `/model`, and select **Auto (Jev)**. Every new text turn then:

1. Sends the exact current ask and the bundled V5 policy to Jev.
2. Validates the chosen model and effort against the allowed pool and Codex catalog.
3. Waits for Codex's thread settings acknowledgement.
4. Submits the original message with its attachments and mentions in the same chat.

The footer's model-with-reasoning field and a transcript notice show the chosen
model and effort. Selecting an explicit model disables Auto. Auto selection is
local to the current widget/session; select it again after resuming or switching
chats. Follow-up prompts during Auto execution queue for the next turn. Answers
to Codex's structured questions retain the running turn's model. Shell commands,
noninteractive `exec`, app-server clients and voice are outside this routing path.

Supply your own Typesafe/Jev key through `TYPESAFE_API_KEY` or `JEV_API`, or point
`CODEX_JEV_KEY_FILE` at an external dotenv file. The installer can save a file
**path**, without copying the key:

```bash
bash tools/fork/install.sh --key-file /absolute/path/to/external.env
```

The helper uses `jev-1.13.0` at `https://api.typesafe.ai/v1/systemone`. The pool is
`gpt-6.1-sol` and `gpt-6-luna`, with low/medium/high/xhigh/max effort. Luna at
high/xhigh/max requires choosing a manual model, matching the existing V5 route.
Model access still depends on your Codex account. Jev inference has its own API
usage; Auto sends the current ask to that provider. History, repository files,
image bytes are not sent to Jev; no extra thread/workspace identifiers are attached. Short follow-ups can be
ambiguous because routing has no conversation context.

Failures preserve the draft and stop submission; there are no hidden retries or
fallback models. Esc/Ctrl+C cancels pending routing. Switching threads cancels
routing and saves the draft through the existing thread input state.

The CLI uses the ordinary Codex home/auth/settings by default, so existing sign-in
and chats are available. Set `CODEX_HOME` yourself for a separate profile.
Upstream automatic update offers are disabled for fork versions; update this fork
from source using the documented process.

## Maintenance

Read [specs/_readme.md](specs/_readme.md). Upstream workflows are retained under
`.github/workflows.disabled/`; this fork does not run them automatically. To update:

```bash
bash tools/fork/upstream-update.sh rust-vVERSION
# Reconcile conflicts, fork version, lockfile and specs.
bash tools/fork/build.sh
bash tools/fork/install.sh
```

The script integrates only the explicitly named tag and requires a clean checkout.
Do not push keys, external dotenv files, private session data or build caches.
Current verification limits are recorded in [specs/jev_routing.md](specs/jev_routing.md).

---

## Upstream README

<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
<p align="center">
  <img src="https://github.com/openai/codex/blob/main/.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>
If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE.</a>
</br>If you want the desktop app experience, run <code>codex app</code> or visit <a href="https://chatgpt.com/codex?app-landing-page=true">the Codex App page</a>.
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

---

## Quickstart

### Installing and running Codex CLI

Run the following on Mac or Linux to install Codex CLI:

```shell
curl -fsSL https://chatgpt.com/codex/install.sh | sh
```

Run the following on Windows to install Codex CLI:

```shell
powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 | iex"
```

The standalone installers download from `https://releases.openai.com/codex` by default and fall back to GitHub Releases if a metadata or asset download is unavailable. To force GitHub Releases, set `CODEX_INSTALLER_USE_RELEASES_OPENAI_COM` to `false` (`0` and `no` are also accepted):

```shell
curl -fsSL https://chatgpt.com/codex/install.sh | CODEX_INSTALLER_USE_RELEASES_OPENAI_COM=false sh
```

```powershell
$env:CODEX_INSTALLER_USE_RELEASES_OPENAI_COM='false'; irm https://chatgpt.com/codex/install.ps1 | iex
```

Codex CLI can also be installed via the following package managers:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Business, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**Open source fund**](./docs/open-source-fund.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).
