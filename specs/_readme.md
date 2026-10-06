# Codex Jev specs

Current implementation contracts for the public Codex fork. Upstream source owns
the Codex harness; the fork owns Jev classification before interactive text turns.

| Spec | Scope and source owners |
| --- | --- |
| [jev_routing.md](jev_routing.md) | Model picker, draft retention, bounded classifier, settings acknowledgement; `codex-rs/tui/src/chatwidget/jev_auto.rs`, `jev_route.rs`, `jev/` |
| [build_maintenance.md](build_maintenance.md) | Upstream pin, Podman build, separate install and updates; `tools/fork/`, workspace version and lockfile |

Import provenance: [reports/imports/jev-routing.md](../reports/imports/jev-routing.md).
