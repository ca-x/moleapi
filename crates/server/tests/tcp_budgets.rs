mod common;
use common::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::io::AsyncReadExt;
async fn setup(url: &str) -> (tempfile::TempDir, Router, String) {
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("budgets.db")).await.unwrap();
    let mut data = example_data();
    let r = &mut data["collections"][0]["requests"][0];
    r["protocol"] = json!({"kind":"tcp"});
    r["url"] = json!(url);
    r["examples"] = json!([]);
    let (status, w) = call(
        &router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"TCP budgets","data":data})),
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
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, s) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if s["state"] == "open" {
                break;
            }
            assert_ne!(s["state"], "error", "{s}");
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    (temp, router, id)
}
async fn send(router: &Router, id: &str, source: &str) -> (StatusCode, Value) {
    call(
        router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(json!({"kind":"tcp_send","message":{"encoding":"base64","payload_source":source}})),
    )
    .await
}
#[tokio::test]
async fn zero_byte_commands_have_independent_512_command_budget() {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let fixture = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let mut b = [0; 1];
        assert_eq!(s.read(&mut b).await.unwrap(), 0);
    });
    let (_temp, router, id) = setup(&url).await;
    for _ in 0..512 {
        let (status, value) = send(&router, &id, "").await;
        assert_eq!(status, StatusCode::OK, "{value}");
    }
    let (status, value) = send(&router, &id, "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("send-command limit")
    );
    let (status, _) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    fixture.await.unwrap();
}
#[tokio::test]
async fn aggregate_input_and_actual_wire_are_bounded_at_twenty_mib() {
    use base64::Engine;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let received = count.clone();
    let fixture = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        let mut b = [0; 8192];
        loop {
            let n = s.read(&mut b).await.unwrap();
            if n == 0 {
                break;
            }
            assert!(b[..n].iter().all(|byte| *byte == 0x41));
            received.fetch_add(n, Ordering::SeqCst);
        }
    });
    let (_temp, router, id) = setup(&url).await;
    let payload = base64::engine::general_purpose::STANDARD.encode(vec![0x41; 1048576]);
    for _ in 0..20 {
        let (status, value) = send(&router, &id, &payload).await;
        assert_eq!(status, StatusCode::OK, "{value}");
    }
    let (status, value) = send(&router, &id, "QQ==").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(value["error"].as_str().unwrap().contains("input limit"));
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while count.load(Ordering::SeqCst) != 20 * 1048576 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let (_, summary) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
    assert_eq!(summary["sent_bytes"], 20 * 1048576);
    let (status, _) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/close"),
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    fixture.await.unwrap();
}
#[tokio::test]
async fn full_command_queue_and_blocked_writer_can_be_cancelled() {
    use base64::Engine;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let (release, wait) = tokio::sync::oneshot::channel();
    let fixture = tokio::spawn(async move {
        let (_stream, _) = l.accept().await.unwrap();
        let _ = wait.await;
    });
    let (_temp, router, id) = setup(&url).await;
    let payload = base64::engine::general_purpose::STANDARD.encode(vec![0x41; 1048576]);
    for _ in 0..6 {
        let (status, value) = send(&router, &id, &payload).await;
        assert_eq!(status, StatusCode::OK, "{value}");
    }
    let mut full = false;
    for _ in 0..40 {
        let (status, value) = send(&router, &id, "").await;
        if status != StatusCode::OK {
            assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
            assert!(value["error"].as_str().unwrap().contains("queue"));
            full = true;
            break;
        }
    }
    assert!(full, "Blocked sender must expose finite command admission");
    let (status, summary) = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        call(
            &router,
            "POST",
            &format!("/api/sessions/{id}/close"),
            None,
            Some(json!({})),
        ),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(summary["state"], "closed");
    let _ = release.send(());
    fixture.await.unwrap();
}

#[tokio::test]
async fn incoming_wire_budget_stops_large_raw_stream() {
    use tokio::io::AsyncWriteExt;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", l.local_addr().unwrap());
    let fixture = tokio::spawn(async move {
        let (mut s, _) = l.accept().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let payload = vec![0x41; 1048576];
        for _ in 0..21 {
            if s.write_all(&payload).await.is_err() {
                break;
            }
        }
    });
    let (_temp, router, id) = setup(&url).await;
    let summary = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, s) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if s["state"] == "error" {
                return s;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        summary["reason"]
            .as_str()
            .unwrap()
            .contains("received-wire limit"),
        "{summary}"
    );
    assert!(summary["received_bytes"].as_u64().unwrap() <= 20 * 1048576 + 65536);
    fixture.await.unwrap();
}
