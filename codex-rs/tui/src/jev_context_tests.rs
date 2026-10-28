use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn live_comparison_cases_use_the_production_transcript_projection() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/jev_context_cases.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let page = serde_json::from_value(case["page"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(JevTranscript::from_page(page)).unwrap(),
            case["transcript"],
            "case {}",
            case["id"],
        );
    }
}

#[test]
fn transcript_bounds_unicode_json_expansion_and_preserves_failure_tail() {
    let mut transcript = JevTranscript {
        availability: "available",
        ..JevTranscript::unavailable()
    };
    for _ in 0..40 {
        transcript.push(
            "tool_output",
            &format!("{} decisive failure", "界\"\\\n".repeat(1_000)),
        );
    }
    let encoded = serde_json::to_vec(&transcript).unwrap();
    assert!(encoded.len() <= MAX_TRANSCRIPT_BYTES);
    assert!(transcript.omitted);
    assert!(transcript.entries.len() <= MAX_ENTRIES);
    for entry in transcript.entries {
        assert!(entry.text.len() <= MAX_ENTRY_BYTES);
        assert!(entry.truncated);
        assert!(entry.text.ends_with("decisive failure"));
    }
}

#[tokio::test]
async fn snapshot_reads_one_bounded_page_and_labels_server_failure() -> color_eyre::Result<()> {
    use codex_app_server_protocol::JSONRPCMessage;
    use futures::SinkExt;
    use futures::StreamExt;
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message;

    let thread_id = ThreadId::new();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut socket = tokio_tungstenite::accept_async(stream).await?;
        let mut pages = 0;
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                continue;
            };
            let mut reply = match request.method.as_str() {
                "initialize" => json!({"result": {"userAgent": "jev-context-test/1.0"}}),
                "thread/items/list" => {
                    assert_eq!(
                        request.params,
                        Some(json!({
                            "threadId": thread_id.to_string(), "turnId": null, "cursor": null,
                            "limit": 32, "sortDirection": "desc",
                        }))
                    );
                    pages += 1;
                    if pages == 1 {
                        json!({"result": {"data": [], "nextCursor": "older", "backwardsCursor": null}})
                    } else {
                        json!({"error": {"code": -32601, "message": "not supported"}})
                    }
                }
                method => panic!("unexpected method: {method}"),
            };
            reply["id"] = json!(request.id);
            socket.send(Message::Text(reply.to_string().into())).await?;
        }
        Ok::<_, color_eyre::Report>(pages)
    });
    let client = crate::connect_remote_app_server(endpoint).await?;
    let handle = client.request_handle();
    let first = snapshot(&handle, thread_id, uuid::Uuid::new_v4()).await;
    assert_eq!(
        serde_json::to_value(first)?,
        json!({
            "source": "app_server_thread_items", "availability": "available",
            "omitted": true, "entries": [],
        })
    );
    let second = snapshot(&handle, thread_id, uuid::Uuid::new_v4()).await;
    assert_eq!(
        serde_json::to_value(second)?,
        serde_json::to_value(JevTranscript::unavailable())?
    );
    drop(handle);
    client.shutdown().await?;
    assert_eq!(server.await??, 2);
    Ok(())
}
