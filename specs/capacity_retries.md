# Model capacity retries

`server_is_overloaded` keeps the pending model request alive within the existing
provider `stream_max_retries` budget (five retries by default). When no server
delay is available, it waits approximately 5, 10, 20, 40, then 60 seconds. The
first four delays use the existing jitter; the local delay has a hard 60-second
cap. A zero retry budget still fails immediately.

`Retry-After` takes precedence, including values longer than the local cap or an
already elapsed deadline. HTTP errors, rejected WebSocket handshakes, wrapped
WebSocket errors and failed Responses events retain their advice; for failed
events it comes from `response.error.headers`. An HTTP 200 streaming handshake's
header does not describe a later event's retry deadline.
Wrapped WebSocket capacity events are recognized by their error code even without
an HTTP status. Valid nested retry advice takes precedence over outer advice.

Every capacity retry emits a visible message with its selected wait and attempt
count through the existing stream error notification. Waiting remains cancellable
through the sampling/compaction caller. Capacity exhaustion does not switch
transports and reset the retry budget. Other error categories retain their policy.

Owners: `protocol/src/error.rs`, `codex-api/src/sse/responses_error.rs`,
`codex-api/src/endpoint/responses_websocket.rs` and `core/src/responses_retry.rs`
under `codex-rs/`. Regression coverage belongs to their existing error tests and
`core/tests/suite/retry_after.rs`.

This behavior runs in the server owning the turn. Use the rebuilt fork with
`--no-daemon` for its embedded server, or connect to a server running the rebuilt
fork. Connecting a patched terminal to an existing upstream daemon retains that
daemon's retry behavior. Fork builds and installation never replace upstream Codex.
