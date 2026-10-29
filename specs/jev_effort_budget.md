# Jev effort mode and weekly budget

## Effort only

Select a supported OpenAI model in `/model`, then select **Effort only (Jev)**. The model stays selected; the backend asks Jev for effort before each model sampling step, including the first step and steps after tool results. Existing **Auto (Jev)** continues to choose model and effort once per user prompt.

The mode uses the existing custom reasoning value `jev` in thread settings. The TUI waits for a settings acknowledgement before showing the mode as active, rejects a picker action from another thread or model, and does not write global defaults. A resumed thread carrying this value keeps effort mode instead of enabling Auto. The mode follows the thread across Default and Plan modes. Selecting a concrete reasoning effort clears it from both modes. Switching to Auto also waits for backend acknowledgement and restores the current model's default effort until the next user prompt is routed.

The backend resolves the local marker to a supported concrete effort before executor inference. Jev receives bounded recent messages, text tool evidence and plaintext worker findings, not hidden reasoning or image bytes. The task comes from the latest actual user request; injected skill instructions and compaction summaries do not replace it. It judges the next step's remaining uncertainty using the existing effort rubric. The fixed model is never a classifier choice. Only the intersection of the model catalog and the routing policy's efforts is eligible; Luna remains limited to low/medium in automatic routing.

Per-step routing runs only in the primary thread, not subagents, internal sessions or Guardian sessions. Children inherit a concrete effort through the existing child-settings path; enabling this mode does not add a classifier call to every child step. Review delegates resolve the Jev marker to the parent's effective or default effort, then apply the review model's existing compatibility rules.

Routing is cancellable and has no provider retries. Executor transport retries reuse the already selected effort. A helper failure or exhausted weekly budget stops the step before executor inference and presents a safe error. The user can choose a manual effort, change the budget, or retry after the reset. Completed tools are not undone.

The selected effort goes through the fork's existing cache-preserving configuration-update and request-baseline logic. This is not a guarantee of a particular cache hit rate or quota saving. Every classified step consumes separate Jev API tokens.

Use the updated fork backend, normally through `codex-jev --no-daemon`. An older or upstream backend does not implement this local mode. The optional `jev-stats` footer labels effort mode; existing per-invocation context records the concrete selected effort. Auto's model-confidence display does not apply to a fixed-model decision.

## Weekly budget

The installed launcher exposes `codex-jev budget` for local budget control. It does not launch Codex or contact either inference provider.

| Command | Effect |
| --- | --- |
| `codex-jev budget status` | Show enablement, limit, usage and reset time |
| `codex-jev budget set 10000000` | Set the weekly token limit |
| `codex-jev budget reset` | Start a fresh seven-day window |
| `codex-jev budget off` | Disable enforcement |
| `codex-jev budget on` | Enable enforcement |

The initial limit is **10,000,000 tokens**, enabled. Usage counts Jev's reported input plus output tokens, separately from Codex executor tokens and subscription quotas. Auto and effort mode share the same persisted local budget across sessions. Turning enforcement off retains accounting; turning it back on does not reset usage. Changing the limit also preserves usage and the enforcement setting.

The shared ledger lives at `${XDG_STATE_HOME:-~/.local/state}/codex-jev/budget.sqlite3`. It is shared across local sessions and `CODEX_HOME` profiles for the same user, not across machines. Set `CODEX_JEV_BUDGET_FILE` to use a different ledger. Add `--json` to any budget command for machine-readable status.

A week is seven days from initialization or an explicit reset, tracked in UTC. The window rolls forward automatically when it expires. The ledger contains token accounting only, without prompts, tool results, API keys or raw provider responses.

Admission and accounting are coordinated across helper processes. Each call reserves an estimate based on serialized request bytes plus a 4,096-token output allowance; known input and output usage replaces that estimate. Unknown usage after interruption or failure retains the estimate. Pending reservations older than five minutes become unreported estimates. Reset and weekly rollover clear reported and unreported usage while retaining recent in-flight reservations, so a reset during active routing may still show reserved tokens. This local control cannot revoke a request already accepted by the provider or establish an exact provider billing ceiling.

## Source ownership

- TUI selection and acknowledgement: `tui/src/chatwidget/jev_effort.rs`, `tui/src/app/jev_effort.rs`.
- Per-step selection and concrete request effort: `core/src/session/`.
- Provider request and budget ledger: `jev/route.py`, `jev/jev_budget.py`.
- Budget launcher command and bundled payload: `tools/fork/`.

Deterministic coverage belongs beside these owners. No live routing-quality or executor-cost improvement is established by offline regression checks.
