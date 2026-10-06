# Jev interactive routing

`Auto (Jev)` is a local TUI routing choice, distinct from upstream's server Auto
models. It is not sent as a model slug. `JevAutoState` lives in each ChatWidget.
Manual selection events disable it; a resumed/new widget starts in manual mode.

Submission retains the complete UserMessage, history record, shell policy,
source and prepared images until success. Prompt submissions during an Auto
agent turn queue. Structured question answers, shell commands and noninteractive
execution follow upstream behavior. Snapshot images prepare before routing.

The Python helper reads the ask through stdin, never argv. It sends only task
and the public policy to the fixed HTTPS Jev endpoint. It reads credentials from
environment or an explicit external file. Redirects are rejected. It emits only
model/effort or a safe error; raw provider bodies and keys never reach UI logs.

Bounds: task 24 KB, serialized provider request 60 KB, provider response 128 KB,
helper stdout 4096 bytes, provider timeout 30 seconds, outer timeout 35 seconds.
No retries or fallback. Strict typed answer validation requires complete allowed
probabilities, finite values, consistent choice and nonnegative integer usage.
The V5 pool is Sol 6.1 and Luna 6; elevated Luna effort stops for manual selection.

Routing runs as a cancellable Tokio subprocess task. Each result carries a UUID
and thread identity; stale results are ignored. The app checks model/effort
against the Codex model catalog and awaits a native thread/settings/update
acknowledgement, including matching collaboration-mode settings. The widget
then applies that choice and submits unchanged original content once. Jev
settings are not persisted to global Codex defaults.

Failures/cancellation restore the draft and pause queue autosend. The existing
thread input snapshot cancels routing before preserving the draft on chat switch.
The bundled helper is installed beside the native executable. Python 3 is required.
The footer's model-with-reasoning field and transcript notice display decisions.

Limits: no historical/project context, images themselves are not classified,
Auto does not persist across widget reconstruction, no Jev hook for exec or voice,
no paid Jev/Codex end-to-end run or test suite has been performed for this change.
Building and installing are requested; additional tests were not requested.
