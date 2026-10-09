mod common;
use common::*;
use std::sync::{Arc, Mutex};
fn settings(kind: &str) -> Value {
    json!({"name":"Channel","kind":kind,"enabled":true,"statuses":["passed","failed","cancelled","interrupted"],"changes_only":false,"language":"en"})
}
async fn workspace(router: &Router, token: Option<&str>) {
    let mut data = example_data();
    data["collections"][0]["requests"][0]["pre_request_script"] =
        "pm.execution.skipRequest();".into();
    assert_eq!(
        call(
            router,
            "POST",
            "/api/workspaces",
            token,
            Some(json!({"id":"w","name":"Notifications","data":data}))
        )
        .await
        .0,
        StatusCode::OK
    );
}
async fn scheduled(router: &Router, target: &str, token: Option<&str>) -> String {
    let(status,schedule)=call(router,"POST","/api/workspaces/w/schedules",token,Some(json!({"name":"channel-private","cron":"0 9 * * *","timezone":"UTC","enabled":false,"collection_id":"c","notification_ids":[target]}))).await;
    assert_eq!(status, StatusCode::OK, "{schedule}");
    let id = schedule["id"].as_str().unwrap().to_string();
    assert_eq!(
        call(
            router,
            "POST",
            &format!("/api/workspaces/w/schedules/{id}/run"),
            token,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    id
}
async fn delivery(router: &Router, token: Option<&str>) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let (_, rows) = call(
                router,
                "GET",
                "/api/workspaces/w/notification-deliveries",
                token,
                None,
            )
            .await;
            let finished = rows.as_array().unwrap().iter().find(|row| {
                matches!(
                    row["status"].as_str(),
                    Some("sent" | "failed" | "cancelled")
                )
            });
            if let Some(row) = finished {
                return row.clone();
            }
            tokio::time::sleep(std::time::Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn signed_webhooks_retry_transient_failure_with_stable_delivery_identity_and_no_secret_copies()
 {
    use hmac::{Hmac, Mac};
    let attempts = Arc::new(Mutex::new(Vec::<(String, Vec<u8>)>::new()));
    let observed = attempts.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::post(
            move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                let attempts = observed.clone();
                async move {
                    let mut hmac =
                        Hmac::<sha1::Sha1>::new_from_slice(b"private-signature-key").unwrap();
                    hmac.update(&body);
                    assert_eq!(
                        headers.get("x-hub-signature").unwrap().to_str().unwrap(),
                        format!("sha1={}", hex::encode(hmac.finalize().into_bytes()))
                    );
                    let mut calls = attempts.lock().unwrap();
                    calls.push((
                        headers
                            .get("x-moleapi-notification-id")
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .into(),
                        body.to_vec(),
                    ));
                    if calls.len() == 1 {
                        StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        StatusCode::NO_CONTENT
                    }
                }
            },
        ),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("notify.db")).await.unwrap();
    workspace(&router, None).await;
    let(status,target)=call(&router,"POST","/api/workspaces/w/notifications",None,Some(json!({"settings":settings("webhook"),"credentials":{"endpoint":format!("{url}/?access_token=channel-private"),"signing_secret":"private-signature-key"}}))).await;
    assert_eq!(status, StatusCode::OK, "{target}");
    assert!(!target.to_string().contains("channel-private"));
    assert!(!target.to_string().contains("private-signature-key"));
    let target_id = target["id"].as_str().unwrap();
    scheduled(&router, target_id, None).await;
    let sent = delivery(&router, None).await;
    assert_eq!(sent["status"], "sent", "{sent}");
    assert_eq!(sent["attempts"], 2);
    assert!(!sent.to_string().contains("channel-private"));
    let calls = attempts.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, calls[1].0);
    let body: Value = serde_json::from_slice(&calls[1].1).unwrap();
    assert_eq!(body["event"], "SCHEDULE_RUN_COMPLETED");
    assert!(body["data"]["report_id"].is_string());
    assert!(!body.to_string().contains("channel-private"));
    drop(calls);
    fixture.abort();
}
#[tokio::test]
async fn robot_receipts_reject_http_success_when_provider_reports_an_error_and_manual_retry_is_explicit()
 {
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::post(|| async {
            axum::Json(json!({"errcode":40001,"errmsg":"private-provider-debug"}))
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("receipt.db")).await.unwrap();
    workspace(&router, None).await;
    let (_, target) = call(
        &router,
        "POST",
        "/api/workspaces/w/notifications",
        None,
        Some(json!({"settings":settings("wecom"),"credentials":{"endpoint":url}})),
    )
    .await;
    scheduled(&router, target["id"].as_str().unwrap(), None).await;
    let failed = delivery(&router, None).await;
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["attempts"], 1);
    assert_eq!(failed["last_http_status"], 200);
    assert!(!failed.to_string().contains("private-provider-debug"));
    let id = failed["id"].as_str().unwrap();
    let (status, retry) = call(
        &router,
        "POST",
        &format!("/api/workspaces/w/notification-deliveries/{id}/retry"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry["status"], "pending");
    fixture.abort();
}
#[tokio::test]
async fn owned_targets_keep_omitted_secrets_and_private_network_policy_blocks_delivery() {
    let temp = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temp.path().join("owned.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "notification-owner").await;
    let other = register(&router, "notification-other").await;
    workspace(&router, Some(&owner)).await;
    let(status,target)=call(&router,"POST","/api/workspaces/w/notifications",Some(&owner),Some(json!({"settings":settings("webhook"),"credentials":{"endpoint":"http://127.0.0.1:9/?token=private-url","signing_secret":"private-key"}}))).await;
    assert_eq!(status, StatusCode::OK);
    let id = target["id"].as_str().unwrap();
    let (status, changed) = call(
        &router,
        "PUT",
        &format!("/api/workspaces/w/notifications/{id}"),
        Some(&owner),
        Some(json!({"settings":settings("webhook"),"expected_revision":1})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(changed["has_signing_secret"], true);
    assert_eq!(changed["revision"], 2);
    assert!(!changed.to_string().contains("private-url"));
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/workspaces/w/notifications",
            Some(&other),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/notification-deliveries/unknown/retry",
            Some(&other),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    scheduled(&router, id, Some(&owner)).await;
    let failed = delivery(&router, Some(&owner)).await;
    assert_eq!(failed["status"], "failed");
    assert!(failed["last_error"].as_str().unwrap().contains("blocked"));
    assert_eq!(
        call(
            &router,
            "DELETE",
            "/api/workspaces/w",
            Some(&owner),
            Some(json!({"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    workspace(&router, Some(&owner)).await;
    let (_, targets) = call(
        &router,
        "GET",
        "/api/workspaces/w/notifications",
        Some(&owner),
        None,
    )
    .await;
    assert!(targets.as_array().unwrap().is_empty());
}
#[tokio::test]
async fn lettre_sends_a_real_smtp_message_to_pinned_loopback_without_custom_production_protocol_code()
 {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let captured = Arc::new(Mutex::new(String::new()));
    let output = captured.clone();
    let smtp = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let (read, mut write) = socket.into_split();
        let mut reader = BufReader::new(read);
        write.write_all(b"220 fixture ESMTP\r\n").await.unwrap();
        let mut data = false;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.unwrap() == 0 {
                break;
            }
            if data {
                if line == ".\r\n" {
                    data = false;
                    write.write_all(b"250 queued\r\n").await.unwrap();
                } else {
                    output.lock().unwrap().push_str(&line);
                }
                continue;
            }
            let upper = line.to_ascii_uppercase();
            if upper.starts_with("EHLO") {
                write
                    .write_all(b"250-fixture\r\n250 8BITMIME\r\n")
                    .await
                    .unwrap();
            } else if upper.starts_with("MAIL FROM") || upper.starts_with("RCPT TO") {
                write.write_all(b"250 ok\r\n").await.unwrap();
            } else if upper.starts_with("DATA") {
                data = true;
                write.write_all(b"354 send body\r\n").await.unwrap();
            } else if upper.starts_with("QUIT") {
                write.write_all(b"221 bye\r\n").await.unwrap();
                break;
            } else {
                write.write_all(b"250 ok\r\n").await.unwrap();
            }
        }
    });
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("smtp.db")).await.unwrap();
    workspace(&router, None).await;
    let(status,target)=call(&router,"POST","/api/workspaces/w/notifications",None,Some(json!({"settings":settings("email"),"credentials":{"smtp":{"host":"localhost","port":port,"tls":"none","from":"mole@example.test","to":["recipient@example.test"]}}}))).await;
    assert_eq!(status, StatusCode::OK, "{target}");
    scheduled(&router, target["id"].as_str().unwrap(), None).await;
    let sent = delivery(&router, None).await;
    assert_eq!(sent["status"], "sent", "{sent}");
    let message = captured.lock().unwrap().clone();
    assert!(message.contains("From: mole@example.test"));
    assert!(message.contains("To: recipient@example.test"));
    assert!(message.contains("Subject: MoleAPI task result"));
    assert!(message.contains("Notifications"));
    smtp.abort();
}
#[tokio::test]
async fn target_version_changes_cancel_pending_retries_and_outcome_change_filters_suppress_repeats()
{
    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls = attempts.clone();
    let (url, fixture) = serve(Router::new().route(
        "/",
        axum::routing::post(move || {
            let calls = calls.clone();
            async move {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                StatusCode::SERVICE_UNAVAILABLE
            }
        }),
    ))
    .await;
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("fence.db")).await.unwrap();
    workspace(&router, None).await;
    let mut channel = settings("webhook");
    channel["changes_only"] = true.into();
    let (_, target) = call(
        &router,
        "POST",
        "/api/workspaces/w/notifications",
        None,
        Some(json!({"settings":channel,"credentials":{"endpoint":url}})),
    )
    .await;
    let target_id = target["id"].as_str().unwrap();
    let schedule = scheduled(&router, target_id, None).await;
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let (_, rows) = call(
                &router,
                "GET",
                "/api/workspaces/w/notification-deliveries",
                None,
                None,
            )
            .await;
            if rows
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["status"] == "pending" && row["attempts"] == 1)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    channel["enabled"] = false.into();
    assert_eq!(
        call(
            &router,
            "PUT",
            &format!("/api/workspaces/w/notifications/{target_id}"),
            None,
            Some(json!({"settings":channel,"expected_revision":1}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let cancelled = delivery(&router, None).await;
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    channel["enabled"] = true.into();
    assert_eq!(
        call(
            &router,
            "PUT",
            &format!("/api/workspaces/w/notifications/{target_id}"),
            None,
            Some(json!({"settings":channel,"expected_revision":2}))
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("/api/workspaces/w/schedules/{schedule}/run"),
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let (_, runs) = call(
                &router,
                "GET",
                &format!("/api/workspaces/w/schedules/{schedule}/runs"),
                None,
                None,
            )
            .await;
            if runs
                .as_array()
                .unwrap()
                .iter()
                .filter(|run| run["status"] == "passed")
                .count()
                >= 2
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let (_, deliveries) = call(
        &router,
        "GET",
        "/api/workspaces/w/notification-deliveries",
        None,
        None,
    )
    .await;
    assert_eq!(deliveries.as_array().unwrap().len(), 1);
    fixture.abort();
}
#[tokio::test]
async fn smtp_public_field_edits_preserve_withheld_passwords_and_explicit_empty_values_clear_them()
{
    let temp = tempfile::tempdir().unwrap();
    let router = local(&temp.path().join("credentials.db")).await.unwrap();
    workspace(&router, None).await;
    let(status,target)=call(&router,"POST","/api/workspaces/w/notifications",None,Some(json!({"settings":settings("email"),"credentials":{"smtp":{"host":"localhost","port":2525,"tls":"none","from":"from@example.test","to":["first@example.test"],"username":"private-smtp-user","password":"private-smtp-password"}}}))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!target.to_string().contains("private-smtp-password"));
    let id = target["id"].as_str().unwrap();
    let(status,updated)=call(&router,"PUT",&format!("/api/workspaces/w/notifications/{id}"),None,Some(json!({"settings":settings("email"),"expected_revision":1,"credentials":{"smtp":{"host":"localhost","port":2525,"tls":"none","from":"from@example.test","to":["second@example.test"]}}}))).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["smtp"]["has_username"], true);
    assert_eq!(updated["smtp"]["has_password"], true);
    assert_eq!(updated["smtp"]["to"], json!(["second@example.test"]));
    let(status,cleared)=call(&router,"PUT",&format!("/api/workspaces/w/notifications/{id}"),None,Some(json!({"settings":settings("email"),"expected_revision":2,"credentials":{"smtp":{"host":"localhost","port":2525,"tls":"none","from":"from@example.test","to":["second@example.test"],"username":"","password":""}}}))).await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert_eq!(cleared["smtp"]["has_password"], false);
}
