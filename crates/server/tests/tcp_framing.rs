mod common;
use common::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
async fn setup(url: &str, framing: &str, idle: u64) -> (tempfile::TempDir, Router, String) {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("tcp.db")).await.unwrap();
    let mut data = example_data();
    let r = &mut data["collections"][0]["requests"][0];
    r["url"] = json!(url);
    r["protocol"] = json!({"kind":"tcp","framing":framing,"idle_timeout_ms":idle});
    r["examples"] = json!([]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"TCP","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, s) = call(
        &router,
        "POST",
        "/api/sessions",
        None,
        Some(json!({"workspace_id":"w","request":w["data"]["collections"][0]["requests"][0]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    let id = s["id"].as_str().unwrap().to_owned();
    state(&router, &id, "open").await;
    (temp, router, id)
}
async fn state(router: &Router, id: &str, target: &str) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, s) = call(router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if s["state"] == target {
                return s;
            }
            if target == "open" {
                assert_ne!(s["state"], "error", "{s}");
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
async fn send(router: &Router, id: &str, source: &str, secret: bool) {
    let(status,v)=call(router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"tcp_send","message":{"encoding":"text","payload_source":source,"secret":secret}}))).await;
    assert_eq!(status, StatusCode::OK, "{v}");
}
#[tokio::test]
async fn length_codecs_handle_fragmented_and_coalesced_frames_byte_exactly() {
    for framing in ["length_be", "length_le"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("tcp://{}", listener.local_addr().unwrap());
        let fixture = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut b = [0; 6];
            s.read_exact(&mut b).await.unwrap();
            let expected = if framing == "length_be" {
                [0, 0, 0, 2, b'h', b'i']
            } else {
                [2, 0, 0, 0, b'h', b'i']
            };
            assert_eq!(b, expected);
            let response: &[u8] = if framing == "length_be" {
                b"\x00\x00\x00\x03one\x00\x00\x00\x03two"
            } else {
                b"\x03\x00\x00\x00one\x03\x00\x00\x00two"
            };
            s.write_all(&response[..2]).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            s.write_all(&response[2..]).await.unwrap();
        });
        let (_temp, router, id) = setup(&url, framing, 0).await;
        send(&router, &id, "hi", false).await;
        fixture.await.unwrap();
        let summary = state(&router, &id, "closed").await;
        assert_eq!(summary["received_bytes"], 14);
        assert_eq!(summary["sent_bytes"], 6);
        let (_, batch) = call(
            &router,
            "GET",
            &format!("/api/sessions/{id}/events"),
            None,
            None,
        )
        .await;
        let texts: Vec<_> = batch["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["direction"] == "incoming")
            .map(|e| e["message"]["text"].as_str().unwrap())
            .collect();
        assert_eq!(texts, ["one", "two"]);
    }
}
#[tokio::test]
async fn line_codec_handles_crlf_and_actual_wire_counters() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let fixture = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let mut b = [0; 7];
        s.read_exact(&mut b).await.unwrap();
        assert_eq!(&b, "鼹鼠\n".as_bytes());
        s.write_all(b"on").await.unwrap();
        s.write_all(b"e\r\nsecond\n").await.unwrap();
    });
    let (_temp, router, id) = setup(&url, "lines", 0).await;
    send(&router, &id, "鼹鼠", false).await;
    fixture.await.unwrap();
    let summary = state(&router, &id, "closed").await;
    assert_eq!(summary["received_bytes"], 12);
    assert_eq!(summary["sent_bytes"], 7);
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    let texts: Vec<_> = batch["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["direction"] == "incoming")
        .map(|e| e["message"]["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["one", "second"]);
}
#[tokio::test]
async fn secret_send_hides_fragmented_echo_without_losing_byte_counts() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let fixture = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let mut b = [0; 12];
        s.read_exact(&mut b).await.unwrap();
        assert_eq!(&b, b"private-seed");
        for part in b.chunks(3) {
            s.write_all(part).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    });
    let (_temp, router, id) = setup(&url, "raw", 0).await;
    send(&router, &id, "private-seed", true).await;
    fixture.await.unwrap();
    let summary = state(&router, &id, "closed").await;
    assert_eq!(summary["received_bytes"], 12);
    let (_, batch) = call(
        &router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await;
    for event in batch["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["message"]["kind"] == "tcp_data")
    {
        assert_eq!(event["message"]["redacted"], true);
        assert_eq!(event["message"]["base64"], "");
        assert!(event["message"]["text"].is_null());
    }
    assert!(!batch.to_string().contains("private-seed"));
}
#[tokio::test]
async fn idle_limit_and_explicit_close_reap_waiting_peer() {
    for idle in [100, 0] {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("tcp://{}", l.local_addr().unwrap());
        let fixture = tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut b = [0; 1];
            assert_eq!(s.read(&mut b).await.unwrap(), 0);
        });
        let (_temp, router, id) = setup(&url, "raw", idle).await;
        if idle == 0 {
            let (status, _) = call(
                &router,
                "POST",
                &format!("/api/sessions/{id}/close"),
                None,
                Some(json!({})),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        } else {
            let summary = state(&router, &id, "error").await;
            assert!(summary["reason"].as_str().unwrap().contains("idle timeout"));
        }
        tokio::time::timeout(std::time::Duration::from_secs(2), fixture)
            .await
            .unwrap()
            .unwrap();
    }
}
#[tokio::test]
async fn declared_oversize_length_is_rejected_before_payload_arrives() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let fixture = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        s.write_all(b"\x00\x10\x00\x01").await.unwrap();
        let mut b = [0; 1];
        assert_eq!(s.read(&mut b).await.unwrap(), 0);
    });
    let (_temp, router, id) = setup(&url, "length_be", 0).await;
    let summary = state(&router, &id, "error").await;
    assert!(
        summary["reason"].as_str().unwrap().contains("frame size"),
        "{summary}"
    );
    fixture.await.unwrap();
}

#[tokio::test]
async fn empty_frame_flood_is_bounded_by_receive_count_not_only_bytes() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let fixture = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let _ = s.write_all(&vec![0; 4 * 5000]).await;
    });
    let (_temp, router, id) = setup(&url, "length_be", 0).await;
    let summary = state(&router, &id, "error").await;
    assert!(
        summary["reason"]
            .as_str()
            .unwrap()
            .contains("received-frame limit"),
        "{summary}"
    );
    assert!(summary["event_count"].as_u64().unwrap() < 4200);
    fixture.await.unwrap();
}
