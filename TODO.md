# Jev TODO and research

## Search and project context

Requested on 2026-10-06 for Codex WebUI 2 and research in this fork. Investigate these questions alongside model and effort selection, especially for Luna low and potentially for every turn. WebUI 2 implements the first three as advisory yes/no/unclear questions on every automatic classification; this fork has bounded V6 conversation context but still needs those diagnostics:

- [ ] Would this turn benefit from online search for external facts, documentation, or sources?
- [ ] Does it specifically need up-to-date information about prices, tools, libraries, APIs, versions, or compatibility?
- [ ] Would it benefit from project context supplied by a maintained summary and relevant specs?
- [ ] Is that summary current and sufficient, or should authoritative project files be read and the summary refreshed?

V6 now supplies bounded visible conversation excerpts, recorded turn status, an active goal and attachment presence. V6.2 also reads a bounded recent server transcript with tool calls and results, following Guardian's evidence selection. It does not supply a maintained project summary or search/freshness diagnostics. Search and project context can both be needed. Keep concise project summaries synchronized with behavior and durable decisions; include source references and freshness metadata. A selected project alone should not inflate a greeting or routine ask. Recording a future research item does not require executing that research.

Research typed answer schemas, when the classifier itself needs a brief, summary location and size limits, freshness detection for revisions and dirty files, synchronization triggers, supported search/context interfaces, and manual-selection behavior. Compare prepared Luna low turns with unprepared turns and examine other model/effort selections, measuring correctness, relevance, freshness, latency, and usage separately from confidence. Cover changing prices/tool/library questions, fixed-version docs, project follow-ups, stale or absent summaries, and turns needing both sources.

## Jev activity visibility

The optional `jev-stats` status-line item now shows the latest acknowledged route's
latency, model/effort confidence and classifier token usage. A decision timeline,
probability distributions and links to execution turns remain future work.

- [ ] Research a TUI Jev activity surface showing classification attempts, outcomes, model/effort probabilities and confidence, classifier usage, and links to actual execution turns. Codex WebUI 2 implements its own sidebar view with bounded decision metadata and native turn-status checks; this fork's TUI remains separate.
- [ ] Preserve exact user content, acknowledged settings changes, bounded metadata, safe errors, and the existing no-retry behavior while adding preparation or visibility.

## Gaps

- Search/project-summary preparation and synchronization are proposals; V6 supplies only the current ask, bounded visible conversation evidence and policy.
- Classifier benefit and paid end-to-end results remain unmeasured.
