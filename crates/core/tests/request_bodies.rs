use axum::{Json, Router, body::Bytes, extract::Multipart, routing::post};
use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_core::*;
use serde_json::{Value, json};
fn request(url: String, kind: &str, body: Value) -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"body","method":"POST","url":url,"description":"","query":[],"headers":[],"body_kind":kind,"body":body.to_string(),"auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":3000,"verify_tls":true,"follow_redirects":true,"assertions":[],"examples":[]})).unwrap()
}
async fn multipart(mut parts: Multipart) -> Json<Value> {
    let mut rows = Vec::new();
    while let Some(part) = parts.next_field().await.unwrap() {
        let name = part.name().unwrap().to_owned();
        let file = part.file_name().map(str::to_owned);
        let mime = part.content_type().map(str::to_owned);
        let bytes = part.bytes().await.unwrap();
        rows.push(json!({"name":name,"file":file,"mime":mime,"bytes":STANDARD.encode(bytes)}));
    }
    Json(json!(rows))
}
async fn serve() -> (String, tokio::task::JoinHandle<()>) {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", socket.local_addr().unwrap());
    let router = Router::new()
        .route("/multipart", post(multipart))
        .route(
            "/binary",
            post(|bytes: Bytes| async move { Json(json!({"bytes":STANDARD.encode(bytes)})) }),
        )
        .route(
            "/redirect",
            post(|| async {
                (
                    axum::http::StatusCode::TEMPORARY_REDIRECT,
                    [("location", "/multipart")],
                )
            }),
        );
    (
        url,
        tokio::spawn(async move { axum::serve(socket, router).await.unwrap() }),
    )
}
#[tokio::test]
async fn mature_encoder_preserves_repeated_unicode_fields_file_bytes_and_307_replay() {
    let (url, server) = serve().await;
    let body = json!({"parts":[{"id":"a","name":"same","value":{"kind":"text","text":"{{message}}","mime":"text/plain"}},{"id":"b","name":"same","value":{"kind":"text","text":"second","mime":""}},{"id":"c","name":"upload","value":{"kind":"file","file":{"file_name":"literal-{{name}}.bin","mime":"application/octet-stream","base64":"AP9B"}}},{"id":"d","name":"ignored","enabled":false,"value":{"kind":"file","file":{"file_name":"missing.bin","mime":"","base64":null}}}]});
    let env:Environment=serde_json::from_value(json!({"id":"dev","name":"dev","variables":[{"id":"m","key":"message","value":"引号 \" & 雪","enabled":true}]})).unwrap();
    for path in ["/multipart", "/redirect"] {
        let result = execute(
            &request(format!("{url}{path}"), "multipart", body.clone()),
            Some(&env),
            NetworkPolicy {
                allow_private_network: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(result.status, 200);
        let rows: Value = serde_json::from_str(&result.body).unwrap();
        assert_eq!(rows[0]["bytes"], STANDARD.encode("引号 \" & 雪"));
        assert_eq!(rows[1]["name"], "same");
        assert_eq!(rows[2]["bytes"], "AP9B");
        assert_eq!(rows[2]["file"], "literal-{{name}}.bin");
        assert_eq!(rows.as_array().unwrap().len(), 3);
    }
    server.abort();
}
#[tokio::test]
async fn actual_binary_empty_bytes_and_missing_file_are_distinct() {
    let (url, server) = serve().await;
    for bytes in [vec![0, 255, 65], vec![]] {
        let body = json!({"file_name":"test.bin","mime":"application/octet-stream","base64":STANDARD.encode(&bytes)});
        let response = execute(
            &request(format!("{url}/binary"), "binary", body),
            None,
            NetworkPolicy {
                allow_private_network: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&response.body).unwrap()["bytes"],
            STANDARD.encode(&bytes)
        );
    }
    let r = request(
        format!("{url}/binary"),
        "binary",
        json!({"file_name":"missing","mime":"","base64":null}),
    );
    validate_request(&r, true).unwrap();
    assert!(
        execute(
            &r,
            None,
            NetworkPolicy {
                allow_private_network: true
            }
        )
        .await
        .is_err()
    );
    server.abort();
}
#[test]
fn invalid_metadata_manual_boundary_and_aggregate_budget_fail() {
    let mut r = request(
        "http://example.com".into(),
        "multipart",
        json!({"parts":[{"id":"one","name":"name","value":{"kind":"text","text":"ok","mime":""}}]}),
    );
    r.headers.push(Pair {
        id: "header".into(),
        key: "Content-Type".into(),
        value: "multipart/form-data; boundary=wrong".into(),
        enabled: true,
        secret: None,
        local_value: None,
    });
    assert!(validate_request(&r, true).is_err());
    r.headers.clear();
    r.body=json!({"parts":[{"id":"one","name":"bad\r\nheader","value":{"kind":"text","text":"ok","mime":""}}]}).to_string();
    assert!(validate_request(&r, true).is_err());
    r.body=json!({"parts":[{"id":"one","name":"one","value":{"kind":"text","text":"x".repeat(MAX_BODY),"mime":""}},{"id":"two","name":"two","value":{"kind":"text","text":"extra","mime":""}}]}).to_string();
    assert!(validate_request(&r, true).is_err());
}

#[test]
fn repeated_variables_reject_during_bounded_expansion_not_final_validation() {
    let r = request(
        "http://example.com".into(),
        "multipart",
        json!({"parts":(0..64).map(|n|json!({"id":n.to_string(),"name":"field","value":{"kind":"text","text":"{{large}}{{large}}{{large}}{{large}}{{large}}","mime":""}})).collect::<Vec<_>>() }),
    );
    validate_request(&r, true).unwrap();
    let env:Environment=serde_json::from_value(json!({"id":"e","name":"e","variables":[{"id":"v","key":"large","value":"x".repeat(900*1024),"enabled":true}]})).unwrap();
    let error = resolve_request(&r, Some(&env)).unwrap_err().to_string();
    assert!(error.contains("expansion limit"), "{error}");
}
#[tokio::test]
async fn disabled_unresolved_mime_is_not_sent_or_required() {
    let (url, server) = serve().await;
    let r = request(
        format!("{url}/multipart"),
        "multipart",
        json!({"parts":[{"id":"a","name":"active","value":{"kind":"text","text":"ok","mime":""}},{"id":"b","name":"inactive","enabled":false,"value":{"kind":"text","text":"{{unused}}","mime":"{{unused}}"}},{"id":"c","name":"missing","enabled":false,"value":{"kind":"file","file":{"file_name":"","mime":"{{unused}}","base64":null}}}]}),
    );
    let result = execute(
        &r,
        None,
        NetworkPolicy {
            allow_private_network: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&result.body)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    server.abort();
}
