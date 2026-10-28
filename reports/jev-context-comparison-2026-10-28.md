# Jev context comparison — 2026-10-28

48 live calls to `jev-1.14.0`: 12 public synthetic follow-ups × two variants × two repetitions. The old helper and policy are byte-identical to the previously installed runtime. No private chat content or raw provider bodies are saved.

The candidate adds a bounded app-server transcript and its interpretation guidance. Model choices, effort choices and their criteria are unchanged. Each fixture includes a descending server page, the expected production transcript, and a model/effort rubric declared before inference. Native tests compare each fixture with the production collector. Requests were shuffled with a fixed seed, using two concurrent helper subprocesses and no retries.

| Metric | Old visible excerpts | New server transcript |
| --- | ---: | ---: |
| Model matches | 12/24 | 24/24 |
| Effort matches | 18/24 | 21/24 |
| Model and effort matches | 12/24 | 21/24 |
| Errors | 0 | 0 |
| Median helper wall time (seconds) | 0.418 | 0.401 |
| Provider input tokens | 47788 | 55392 |
| Provider output tokens | 3074 | 3067 |

| Case | Old, two runs | New, two runs | Expected |
| --- | --- | --- | --- |
| failed-hypotheses | sol medium, sol medium | astra max, astra max | astra high/xhigh |
| established-procedure | sol low, sol low | luna low, luna low | luna low |
| implementation-behind-visual | terra low, terra low | sol low, sol low | sol low/medium |
| visual-review-ready | sol low, sol low | terra low, terra low | terra low |
| technical-correctness | sol low, sol low | sol low, sol low | sol low/medium |
| long-output-failure-tail | sol medium, sol medium | astra max, astra xhigh | astra high/xhigh |
| unloaded-history | sol medium, sol medium | luna low, luna low | luna low |
| complex-known-method | sol low, sol low | sol low, sol low | sol low/medium |
| control-new-topic | luna low, luna low | luna low, luna low | luna low |
| control-writing | terra low, terra low | terra low, terra low | terra low |
| control-image | terra low, terra low | terra low, terra low | terra low |
| control-repeat | luna low, luna low | luna low, luna low | luna low |

The model changed appropriately when tool evidence exposed failed approaches, an established runbook, implementation behind a visual request, or remaining visual QA. All four controls kept their expected model and low effort in both repetitions. The candidate overshot the predeclared effort range in three decisions: `max` on both failed-hypotheses runs and one long-output-failure-tail run. These are retained as misses; no rubric or effort criteria were changed after seeing results.

This is a small, deliberately context-sensitive synthetic routing test, not a representative benchmark or executor-quality evaluation. The expected labels are policy-based judgments. Both variants see the same visible excerpts; only the candidate sees the extra server evidence. Candidate guidance also explains the new evidence schema, so this is a comparison of the complete context change, not a strict prompt-token ablation. The helper timing excludes the native history read, settings acknowledgement and executor inference. No live OpenAI executor call was made. Provider confidence was not treated as correctness.

Detailed safe scalar results and tested helper/policy hashes: [results JSON](jev-context-results-2026-10-28.json). Public inputs: [fixtures](../codex-rs/tui/tests/fixtures/jev_context_cases.json). Reproducer: `python3 jev/compare_context.py --baseline-dir PATH_TO_OLD_JEV_DIRECTORY --output NEW_PATH --repeats 2`.

A read-only probe against the actual app server returned 32 recent items (107,214 wire bytes) in 0.292 seconds, confirming the history API works on a real thread. Only item counts, types, timing and size were saved; no private thread content was persisted or sent to Jev for that probe. The native collector is separately checked against public fixtures.

All 19 focused native tests and 11 Python routing tests passed. Native coverage includes the real RPC shape through a mock server, server-error availability, the production projection of all 12 fixtures, Unicode/JSON byte caps, draft recovery, stale-result rejection and the updated disclosure snapshot.

The host system/Python CA stores passed the incident trust guard before HTTPS. Total reported Jev usage: 109,321 tokens. No retries or provider errors occurred.
