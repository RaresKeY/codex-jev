# Codex Jev specs

Current implementation contracts for the public Codex fork. Upstream source owns
the Codex harness; the fork owns Jev classification before interactive text turns.

| Spec | Scope and source owners |
| --- | --- |
| [jev_routing.md](jev_routing.md) | V6 policy, startup precedence, bounded server transcript and visible context, draft retention, settings acknowledgement and optional status-line stats; `codex-rs/tui/src/app/jev_startup.rs`, `chatwidget/jev_auto.rs`, `chatwidget/jev_status.rs`, `bottom_pane/status_line_setup.rs`, `jev_context.rs`, `jev_route.rs`, `jev/` |
| [jev_effort_budget.md](jev_effort_budget.md) | Fixed-model per-step effort selection, local weekly Jev budget and launcher controls; core session loop, TUI effort picker, `jev/`, `tools/fork/` |
| [build_maintenance.md](build_maintenance.md) | Upstream pin, Podman build, separate install and updates; `tools/fork/`, workspace version and lockfile |
| [updates_ci.md](updates_ci.md) | Upstream and fork update behavior, reviewed maintenance, fork-only validation and candidate builds; `tui/src/updates.rs`, `.github/workflows/jev-ci.yml` |
| [capacity_retries.md](capacity_retries.md) | Model capacity backoff, server delay advice and bounded retries; protocol errors, Responses API parsing and core retry loop |

Import provenance: [reports/imports/jev-routing.md](../reports/imports/jev-routing.md).

Future work: [Jev TODO and research](../TODO.md) tracks search/freshness questions,
maintained project summaries, and activity visibility. These are proposals, not
implemented routing contracts.
