# Jev effort refinement — 2026-10-28

The refinement removes all three previously observed `max` overshoots in a fresh live comparison. Across 22 synthetic cases, effort-rubric agreement rises from 30/44 to 38/44 decisions, with identical model selections. Six effort selections remain below the declared ranges; this is an improvement on this suite, not a claim of general routing accuracy.

## Change and comparison

The baseline is the installed `2026-10-28-v6.2-context` helper and policy. The candidate is `2026-10-28-v6.3-effort`. Both receive exactly the same bounded transcript and visible context; per-request input hashes verify this. The model-role criteria and context collector are unchanged.

The old policy contradicted itself: failed plausible approaches appeared both as an `xhigh` anchor and a direct `max` criterion. The new policy judges reasoning still required for the current deliverable. Resolved failures can return to low; unresolved coupled failures can warrant xhigh; max requires exceptional depth and novelty together. Effort criteria now live in the policy and feed the provider choice directly, with pool and nonempty-string validation before credentials or transport.

88 live `jev-1.14.0` calls: 22 cases × two variants × two repetitions. Twelve cases are reused from the context comparison, including the two cases that motivated this refinement. Ten fresh stress cases cover resolved failures, repeated operational failures, bounded debugging, new foundations, difficult correctness, coupled uncertainty, exceptional derivations, absent context and tool-text instructions. Expected labels were frozen before inference; there was one candidate and no post-result tuning or relabelling. The fresh cases are author-created synthetic checks, not an independent benchmark.

Jobs were shuffled with a fixed seed, two concurrent helper processes and no retries. Only safe decision/usage/timing scalars and source/input hashes are recorded. No private conversation content, raw provider responses or OpenAI executor calls were used.

| Metric | Installed v6.2 | Refined v6.3 |
| --- | ---: | ---: |
| Model matches | 42/44 | 42/44 |
| Effort matches | 30/44 (68.2%) | 38/44 (86.4%) |
| Model and effort matches | 29/44 | 36/44 |
| Errors | 0 | 0 |
| Median helper wall time | 0.436 s | 0.441 s |
| Provider input tokens | 99,716 | 112,124 |
| Provider output tokens | 5,639 | 5,640 |

Input usage rises 12.4%. Total reported usage is 223,119 Jev tokens. The 5 ms median difference is descriptive; this small run does not establish a latency change. Timing excludes native history reads, settings acknowledgement and executor work.

## Case results

| Case | Baseline effort, two runs | Refined effort, two runs | Declared range |
| --- | --- | --- | --- |
| Failed synchronization hypotheses | max, max | xhigh, xhigh | high/xhigh |
| Failure at end of long output | xhigh, max | xhigh, xhigh | high/xhigh |
| Repeated operational failure, service restored | low, low | low, low | low |
| Earlier failures, exact fix now established | low, low | low, low | low |
| Isolated parser debugging after failed guesses | medium, medium | high, high | medium/high |
| New reusable validation foundation | medium, medium | high, high | high |
| Bounded linearizability proof | low, low | medium, medium | high/xhigh |
| Coupled controller uncertainty | high, high | high, high | xhigh |
| Novel theorem with dependent proof gaps | xhigh, xhigh | max, max | max |
| Novel inverse-problem synthesis | xhigh, xhigh | xhigh, xhigh | max |
| Missing follow-up context | low, medium | medium, medium | medium |
| Tool text instructs classifier to use max | low, low | low, low | low |

All other reused cases retained their expected low effort. On the twelve reused cases, combined agreement improves from 21/24 to 24/24. On the ten fresh cases, effort agreement improves from 9/20 to 14/20, and combined agreement from 8/20 to 12/20. No previously matching effort becomes a miss.

The two max controls remain deliberately strict. The theorem case selects max in both candidate repetitions, so max has not been globally suppressed. The inverse-problem case remains xhigh in both variants and is counted as two misses. The bounded proof and coupled controller each contribute two further effort misses. For the context-free `continue`, both variants choose Luna while the declared model rubric expects Sol; these two model misses remain visible and the model policy was not retuned.

Labels express policy-based judgments, not measured executor quality. Repeated calls on the same examples are not independent tasks. The fresh cases expose sensitivity around proof difficulty, high/xhigh and xhigh/max boundaries. Confidence values are provider metadata, not calibrated accuracy estimates.

## Reproduction and validation

Safe scalar evidence and exact source hashes: [results JSON](jev-effort-results-2026-10-28.json). Inputs: [original production-projection fixtures](../codex-rs/tui/tests/fixtures/jev_context_cases.json) and [fresh effort fixtures](../jev/fixtures/effort_cases.json). The additional cases are constructed directly in the accepted helper context schema; only the original twelve are checked against native projection fixtures.

```bash
python3 jev/compare_context.py \
  --baseline-dir PATH_TO_V6_2_HELPER_AND_POLICY \
  --context-mode both-transcript \
  --extra-cases jev/fixtures/effort_cases.json \
  --repeats 2 --output NEW_PATH
```

All 13 offline Python routing tests passed, including policy-to-provider effort propagation and rejection of malformed effort policies before credentials/transport. All 22 inputs pass the production context validator. The unchanged native implementation retains the preceding release's 19 focused passing tests; native tests were not repeated for this Python/policy-only refinement. Previously checked system/Python CA stores were hash-verified unchanged before HTTPS.

The deployment reuses the verified native binaries from the v6.2 release and installs the matching refined helper/policy as a new complete payload. The previous release remains available; its receipt supplies native build provenance. `current/BUILD_RECEIPT.json` records the exact installed hashes and the inherited native source manifest. Full `just fmt`, semantic equivalence of the formatted helper to the live-tested source, and `git diff --check` are deployment checks.
