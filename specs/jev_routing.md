# Jev interactive routing

`Auto (Jev)` is a local TUI routing choice, distinct from upstream's server Auto
models. It is not sent as a model slug. `JevAutoState` lives in each ChatWidget.
New and resumed widgets start with Auto enabled. Manual selection events disable
it for that widget; selecting Auto in `/model` enables it again.
Explicit launch model/provider/effort settings, including selected profiles, take
precedence at startup and widget replacement. Inherited ordinary user defaults do
not disable Auto. Custom-provider and Reserve sessions disable Auto when attached;
an acknowledged backend usage fallback cancels pending classification, preserves
its draft and disables Auto so Jev cannot override that account-authorized model.
Queued Auto picker actions cannot re-enable routing while Reserve is forced or
the task is waiting for its Reserve switch.
When the current model is absent from the catalog, picker focus falls back to a
catalog model rather than implicitly selecting Jev.

Submission retains the complete UserMessage, history record, shell policy,
source and prepared images until success. Prompt submissions during an Auto
agent turn queue. Structured question answers, shell commands and noninteractive
execution follow upstream behavior. Snapshot images prepare before routing.

The Python helper reads the ask through stdin, never argv. It sends the task,
bounded conversation context and public policy to the fixed HTTPS Jev endpoint.
It reads credentials from environment or an explicit external file. Redirects are rejected. It emits only
model/effort with bounded numeric routing metadata, or a safe error; raw provider
bodies and keys never reach UI logs.

Bounds: task 24 KB, serialized provider request 60 KB, provider response 128 KB,
helper stdout 4096 bytes, provider timeout 30 seconds, outer timeout 35 seconds.
An empty or whitespace-only task is accepted only with validated image presence;
the original text stays unchanged. Empty text without images is rejected before
credentials or transport are used.
Context has a 24 KB serialized UTF-8 limit. The helper bounds stdin to 288512 bytes,
allowing worst-case JSON escaping of task/context plus the request envelope, and
rejects oversized input before any request.
No provider retries or fallback models. Strict typed answer validation requires complete allowed
probabilities, finite values, consistent choice and nonnegative integer usage.
The V6 pool is Luna 6, Terra 6.1, Sol 6.1 and Astra 6; elevated Luna effort stops
for manual selection. Policy model and effort criteria feed their Choices directly
and must match the supported pools. Effort descriptions must be nonempty strings;
malformed effort policy stops before credentials or transport. Model and effort are independent judgments over shared
state; neither question consumes the other's answer. Code validates the pair.

V6 distinguishes established execution, writing/visual/GUI work with a clear
brief, bounded technical judgment, and discovering/reconsidering the approach.
Sol low remains valid for complex work with a known method. Effort reflects
uncertainty and reconsideration, not duration. Immediate deliverables determine
scope; future TODOs and project names do not inflate it. V6.1 adds one image rule:
when `context.images_attached` is true, otherwise-Luna tasks use Terra. Tasks
requiring Sol/Astra keep those roles, and images alone do not raise effort.
Image requests exclude Luna from the execution-model Choice criteria. Response
validation requires the complete distribution for that three-model pool, so a
provider response cannot silently select Luna against the image policy.

V6.3 judges effort from the reasoning still needed for the immediate deliverable.
Failed attempts alone do not justify max. Reframing coupled unresolved failures
normally uses high/xhigh; a known remedy can return to low. Max requires evidence
of exceptional reasoning depth and novelty together. `policy.json` owns the effort
criteria used by the helper, eliminating a separate hardcoded set of descriptions.

The separate [effort mode and weekly budget](jev_effort_budget.md) keeps a model fixed and routes effort between sampling steps. Both modes share the Jev token budget.

## Follow-up context

`JevContext` in `jev_route.rs` retains up to three chronological visible requests
and the latest available completed response. Each excerpt is a UTF-8-safe prefix
of at most 768 bytes with an explicit truncation flag. A new request clears the
previous response/status; turn notifications record completed/failed/interrupted
status without claiming task success. On submission the TUI adds the active goal
and attachment presence, with no image contents or extra identifiers.

Before classification, `jev_context.rs` reads one descending page of at most 32
items through the active app-server request handle. This uses the server's
current thread history, including recent items not loaded by the widget, and
therefore follows server rollback and remote-session state. Like Guardian's
transcript collector (`core/src/guardian/prompt.rs`), it retains chronological
user/assistant messages, plans, tool calls and tool results, including command
exit codes. It excludes reasoning, hook prompts, review scaffolding and media
payloads. It projects public thread items rather than depending on Guardian's
private approval-session machinery or copying approval policy into routing.

The snapshot holds at most 24 entries, each at most 1536 UTF-8 bytes, and at most
16 KB of compact serialized UTF-8. Long entries retain a head and tail with an
omission marker, preserving final errors. Oldest entries are dropped to meet
count and serialized-byte limits. `omitted` reports skipped/evicted items or an
unread older page. A failed, unsupported or three-second-timed-out read produces
an explicit `availability: unavailable` snapshot and keeps the visible evidence.
The initial handoff to the active server is also bounded to three seconds;
handoff failure restores the draft without inference. There are no read retries.

Context is always labelled partial, source-labelled and strictly validated by the helper.
Thread changes and prompt rollback clear it. A newly observed request or turn
start cancels a pending classification so changed conversation evidence cannot
apply a stale decision. Rollback context is subsequently
repopulated by observed messages and the next server snapshot. Failed approaches
are known only where supplied messages or tool results describe them.
No extra summarizer, repository reads or provider calls are used. Recent text tool
evidence is sent to Jev along with conversation excerpts; it is not persisted by
the routing collector. Tool text is evidence, never classifier instructions.
The classifier snapshot never changes the original execution-model submission.

## Submission

Routing runs as a cancellable Tokio subprocess task. Each result carries a UUID
and thread identity; stale results are ignored. The app checks model/effort
against the Codex model catalog and awaits a native thread/settings/update
acknowledgement, including matching collaboration-mode settings. The widget
then applies that choice and submits unchanged original content once. Jev
settings are not persisted to global Codex defaults.

Failures/cancellation restore the draft and mark existing queued follow-ups as
recovered, pausing their autosend while allowing the restored draft to be retried.
They do not set the settings/disconnection suppression flag. The existing
thread input snapshot cancels routing before preserving the draft on chat switch.
If dispatch fails after a decision, the retained message returns to the composer
and queued follow-ups remain paused, including a disconnected command channel.
The bundled helper is installed beside the native executable. Python 3 is required.
Before the first acknowledged routing decision, the footer and terminal-title
model fields show only `Auto (Jev)` and omit the inherited reasoning effort.
After a decision, the model-with-reasoning field shows Auto plus the selected
model/effort; the transcript notice also records the decision. Cancelling or
failing the first route does not expose the inherited manual/default model.
Manual selection, thread changes and prompt rollback clear the Auto selection
indicator. Selecting Auto again while it is already enabled retains its decision.

## Optional routing stats

The `/statusline` picker includes `jev-stats`, disabled by default and persisted
with the existing status-line selection and order. `chatwidget/jev_status.rs`
formats the latest acknowledged route as, for example,
`Jev 0.8s M82% E91% 480/12 tok`: local subprocess-to-decision elapsed seconds,
model confidence, effort confidence and classifier input/output token counts.
Settings-acknowledgement and execution time are excluded. Confidence values come
from Jev, not empirical quality measurements. Token counts use the existing
compact token formatter; executor usage is separate.

The item shows `Jev ready`, `Jev routing…`, `Jev failed` or `Jev off` in other
states. Pending results replace previous stats; cancellation, manual selection,
thread changes and rollback clear them. Stale results cannot overwrite the
current route's status. No metrics are persisted with a chat or recovered from
history. Older helpers that omit stats still work and show timing alone.
The bundled helper emits only validated confidence and usage scalars, and the
native decoder checks their types and bounds again. No additional provider calls,
request logging or retry behavior are introduced. Install the helper and native
executable together through the owning installer.

Limits: no maintained project summary or image-content classification, and no
Jev hook for exec or voice. Manual selection does not persist across widget
reconstruction. The complete native routing-to-OpenAI execution path has not been
exercised live. A 48-call live helper comparison uses synthetic fixtures checked
against the production native projection; it measures routing-rubric agreement,
not executor quality. See [the comparison](../reports/jev-context-comparison-2026-10-28.md).
An additional 88-call comparison holds context identical between v6.2 and v6.3.
Effort-rubric agreement improves from 30/44 to 38/44; model selections are unchanged.
All three original max overshoots are removed, while six fresh-case effort misses
remain. See [the effort refinement](../reports/jev-effort-comparison-2026-10-28.md).

V6 has local deterministic regression coverage. The context comparison does not
establish executor model quality or confidence calibration.

## Provider version

As of October 15, 2026, the Python classifier pins `jev-1.14.0` and requires
that response identity. V6 changes the execution-model policy; native
acknowledgement barriers remain. Installed copies require the owning install workflow.
