mod common;
use common::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
async fn open(router: &Router, url: &str, framing: &str) -> String {
    let mut data = example_data();
    data["global_variables"] =
        json!([{"id":"empty","key":"empty_token","value":"","secret":true,"enabled":true}]);
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = json!(url);
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"tcp","framing":framing});
    let (status, w) = call(
        router,
        "POST",
        "/api/workspaces",
        None,
        Some(json!({"id":"w","name":"TCP","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{w}");
    let (status, s) = call(
        router,
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
            let (_, s) = call(router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if s["state"] == "open" {
                break;
            }
            assert_ne!(s["state"], "error", "{s}");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    id
}
async fn events(router: &Router, id: &str) -> Value {
    call(
        router,
        "GET",
        &format!("/api/sessions/{id}/events"),
        None,
        None,
    )
    .await
    .1
}
#[tokio::test]
async fn raw_binary_send_half_close_continues_reading_and_accounts_wire_bytes() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("tcp://{}", listener.local_addr().unwrap());
    let fixture = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        assert_eq!(bytes, [0, 1, 255, 65]);
        stream.write_all(b"after EOF").await.unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("tcp.db")).await.unwrap();
    let id = open(&router, &url, "raw").await;
    let(status,value)=call(&router,"POST",&format!("/api/sessions/{id}/send"),None,Some(json!({"kind":"tcp_send","message":{"encoding":"hex","payload_source":"00 01 ff 41","secret":false}}))).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let (status, value) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(json!({"kind":"tcp_half_close"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    fixture.await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, s) = call(&router, "GET", &format!("/api/sessions/{id}"), None, None).await;
            if s["state"] == "closed" {
                assert_eq!(s["sent_bytes"], 4);
                assert_eq!(s["received_bytes"], 9);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let batch = events(&router, &id).await;
    assert!(
        batch["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["direction"] == "incoming"
                && e["message"]["kind"] == "tcp_data"
                && e["message"]["text"] == "after EOF"),
        "{batch}"
    );
    let (status, _) = call(
        &router,
        "POST",
        &format!("/api/sessions/{id}/send"),
        None,
        Some(json!({"kind":"tcp_send","message":{"encoding":"text","payload_source":"late"}})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
