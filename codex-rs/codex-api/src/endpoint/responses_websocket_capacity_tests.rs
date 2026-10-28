use super::*;
use crate::api_bridge::map_api_error;
use codex_protocol::error::CodexErrorDetails;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(start_paused = true)]
async fn capacity_errors_keep_retry_advice_without_requiring_http_status() {
    for status in [None, Some(503)] {
        for (nested, outer, expected) in [
            (
                json!({"Retry-After": "120"}),
                json!({"Retry-After": "60"}),
                Some(120),
            ),
            (
                json!({"Retry-After": "invalid"}),
                json!({"Retry-After": 60}),
                Some(60),
            ),
            (Value::Null, json!({"Retry-After": "0"}), Some(0)),
            (Value::Null, json!({}), None),
        ] {
            let mut payload = json!({
                "type": "error",
                "error": {"code": "server_is_overloaded", "headers": nested},
                "headers": outer,
            });
            if let Some(status) = status {
                payload["status"] = json!(status);
            }
            let payload = payload.to_string();
            let event = parse_wrapped_websocket_error_event(&payload).expect("error event");
            let error = map_api_error(
                map_wrapped_websocket_error_event(event, payload).expect("capacity error"),
            );
            assert!(matches!(
                error.details(),
                CodexErrorDetails::ServerOverloaded
            ));
            assert_eq!(
                error.server_retry_delay(),
                expected.map(Duration::from_secs)
            );
            assert!(error.retry_delay(/*retry_count*/ 1).is_some());
        }
    }
}

#[tokio::test(start_paused = true)]
async fn rejected_websocket_handshake_keeps_capacity_retry_deadline() {
    let response = http::Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .header("Retry-After", "120")
        .body(Some(
            br#"{"error":{"code":"server_is_overloaded"}}"#.to_vec(),
        ))
        .expect("HTTP error response");
    let error = map_api_error(map_ws_error(
        WsError::Http(Box::new(response)),
        &Url::parse("wss://example.test/responses").expect("URL"),
    ));
    assert!(matches!(
        error.details(),
        CodexErrorDetails::ServerOverloaded
    ));
    assert_eq!(error.server_retry_delay(), Some(Duration::from_secs(120)));
    tokio::time::advance(Duration::from_secs(20)).await;
    assert_eq!(error.server_retry_delay(), Some(Duration::from_secs(100)));
}
