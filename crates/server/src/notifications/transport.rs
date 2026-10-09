use super::models::{Credentials, Event, Settings};
use crate::AppState;
use base64::{Engine, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use std::time::Duration;
pub(super) enum Outcome {
    Accepted(Option<u16>),
    Retry(Option<u16>, &'static str),
    Rejected(Option<u16>, &'static str),
}
fn text(settings: &Settings, event: &Event) -> String {
    if settings.language == "zh-CN" {
        format!(
            "MoleAPI 任务结果\n工作区：{}\n任务：{}\n结果：{}\n通过：{}，失败：{}，跳过：{}\n完成时间：{}\n报告：{}",
            event.workspace_name,
            event.schedule_name,
            event.status,
            event.passed,
            event.failed,
            event.skipped,
            event.finished_at,
            event.report_id.as_deref().unwrap_or("--")
        )
    } else {
        format!(
            "MoleAPI task result\nWorkspace: {}\nTask: {}\nOutcome: {}\nPassed: {}, failed: {}, skipped: {}\nFinished: {}\nReport: {}",
            event.workspace_name,
            event.schedule_name,
            event.status,
            event.passed,
            event.failed,
            event.skipped,
            event.finished_at,
            event.report_id.as_deref().unwrap_or("--")
        )
    }
}
fn signature(secret: &str, message: &[u8]) -> String {
    let mut hmac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC supports any key size");
    hmac.update(message);
    STANDARD.encode(hmac.finalize().into_bytes())
}
pub(super) fn payload(settings: &Settings, credentials: &Credentials, event: &Event) -> Value {
    let content = text(settings, event);
    match settings.kind.as_str() {
        "slack" => json!({"text":content}),
        "teams" => {
            json!({"type":"message","attachments":[{"contentType":"application/vnd.microsoft.card.adaptive","contentUrl":null,"content":{"$schema":"http://adaptivecards.io/schemas/adaptive-card.json","type":"AdaptiveCard","version":"1.2","body":[{"type":"TextBlock","text":content,"wrap":true}]}}]})
        }
        "wecom" | "dingtalk" => json!({"msgtype":"text","text":{"content":content}}),
        "feishu" => json!({"msg_type":"text","content":{"text":content}}),
        "pagerduty" => {
            let action = if event.status == "passed" {
                "resolve"
            } else {
                "trigger"
            };
            let mut value = json!({"routing_key":credentials.routing_key,"event_action":action,"dedup_key":format!("moleapi:{}",event.schedule_id)});
            if action == "trigger" {
                value["payload"] = json!({"summary":format!("MoleAPI task {}: {}",event.schedule_name,event.status),"source":"MoleAPI","severity":"error","custom_details":{"report_id":event.report_id,"passed":event.passed,"failed":event.failed,"skipped":event.skipped}});
            }
            value
        }
        _ => {
            json!({"event":event.kind,"event_id":event.id,"title":"MoleAPI task result","content":content,"data":event})
        }
    }
}
pub(super) fn receipt(kind: &str, status: u16, body: &[u8]) -> Outcome {
    if status == 408 || status == 429 || status >= 500 {
        return Outcome::Retry(Some(status), "Temporary notification HTTP failure");
    }
    if !(200..300).contains(&status) {
        return Outcome::Rejected(Some(status), "Notification endpoint rejected the request");
    }
    let json = serde_json::from_slice::<Value>(body).ok();
    match kind {
        "slack" if body != b"ok" => {
            Outcome::Rejected(Some(status), "Slack did not acknowledge the message")
        }
        "wecom" | "dingtalk"
            if json.as_ref().and_then(|value| value["errcode"].as_i64()) != Some(0) =>
        {
            Outcome::Rejected(Some(status), "Robot API rejected the message")
        }
        "feishu"
            if json
                .as_ref()
                .and_then(|value| value.get("code").or_else(|| value.get("StatusCode")))
                .and_then(Value::as_i64)
                != Some(0) =>
        {
            Outcome::Rejected(Some(status), "Feishu rejected the message")
        }
        "pagerduty"
            if json.as_ref().and_then(|value| value["status"].as_str()) != Some("success") =>
        {
            Outcome::Rejected(Some(status), "PagerDuty rejected the event")
        }
        _ => Outcome::Accepted(Some(status)),
    }
}
pub(super) async fn send(
    state: &AppState,
    settings: &Settings,
    credentials: &Credentials,
    event: &Event,
    delivery_id: &str,
) -> Outcome {
    match tokio::time::timeout(
        Duration::from_secs(20),
        send_inner(state, settings, credentials, event, delivery_id),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Outcome::Retry(None, "Notification transport deadline exceeded"),
    }
}
async fn send_inner(
    state: &AppState,
    settings: &Settings,
    credentials: &Credentials,
    event: &Event,
    delivery_id: &str,
) -> Outcome {
    if settings.kind == "email" {
        return smtp(state, settings, credentials, event, delivery_id).await;
    }
    let Ok(mut url) = moleapi_core::valid_url(&credentials.endpoint) else {
        return Outcome::Rejected(None, "Notification endpoint configuration is invalid");
    };
    let mut body = payload(settings, credentials, event);
    if settings.kind == "dingtalk" && !credentials.signing_secret.is_empty() {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let sign = signature(
            &credentials.signing_secret,
            format!("{timestamp}\n{}", credentials.signing_secret).as_bytes(),
        );
        let pairs = url
            .query_pairs()
            .filter(|(key, _)| key != "timestamp" && key != "sign")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        url.set_query(None);
        url.query_pairs_mut()
            .extend_pairs(pairs)
            .append_pair("timestamp", &timestamp)
            .append_pair("sign", &sign);
    }
    if settings.kind == "feishu" && !credentials.signing_secret.is_empty() {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let sign = signature(&format!("{timestamp}\n{}", credentials.signing_secret), b"");
        body["timestamp"] = timestamp.into();
        body["sign"] = sign.into();
    }
    let Ok(bytes) = serde_json::to_vec(&body) else {
        return Outcome::Rejected(None, "Notification payload could not be serialized");
    };
    let Ok(client) = moleapi_core::checked_client(
        &url,
        moleapi_core::NetworkPolicy {
            allow_private_network: state.local || state.config.allow_private_network,
        },
        true,
    )
    .await
    else {
        return Outcome::Rejected(None, "Notification destination was blocked or unresolved");
    };
    let mut request = client
        .post(url)
        .timeout(Duration::from_secs(15))
        .header("content-type", "application/json")
        .header("x-moleapi-notification-id", delivery_id)
        .body(bytes.clone());
    if settings.kind == "webhook" && !credentials.signing_secret.is_empty() {
        let mut hmac = Hmac::<sha1::Sha1>::new_from_slice(credentials.signing_secret.as_bytes())
            .expect("HMAC supports any key size");
        hmac.update(&bytes);
        request = request.header(
            "x-hub-signature",
            format!("sha1={}", hex::encode(hmac.finalize().into_bytes())),
        );
    }
    if settings.kind == "jenkins" && !credentials.signing_secret.is_empty() {
        request = request.bearer_auth(&credentials.signing_secret);
    }
    let Ok(mut response) = request.send().await else {
        return Outcome::Retry(None, "Notification connection failed");
    };
    let status = response.status().as_u16();
    let mut content = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(bytes)) => {
                if content.len() + bytes.len() > 65536 {
                    return Outcome::Rejected(Some(status), "Notification receipt exceeds 64 KiB");
                }
                content.extend_from_slice(&bytes);
            }
            Ok(None) => break,
            Err(_) => {
                return Outcome::Retry(Some(status), "Notification receipt could not be read");
            }
        }
    }
    receipt(&settings.kind, status, &content)
}
async fn smtp(
    state: &AppState,
    settings: &Settings,
    credentials: &Credentials,
    event: &Event,
    delivery_id: &str,
) -> Outcome {
    use lettre::{
        AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
        transport::smtp::{
            authentication::Credentials as Login,
            client::{Tls, TlsParameters},
        },
    };
    let Some(config) = &credentials.smtp else {
        return Outcome::Rejected(None, "SMTP configuration is missing");
    };
    let mut url = url::Url::parse("https://smtp.invalid").unwrap();
    if url.set_host(Some(&config.host)).is_err() || url.set_port(Some(config.port)).is_err() {
        return Outcome::Rejected(None, "SMTP destination is invalid");
    }
    let Ok(addresses) = moleapi_core::checked_destination(
        &url,
        moleapi_core::NetworkPolicy {
            allow_private_network: state.local || state.config.allow_private_network,
        },
    )
    .await
    else {
        return Outcome::Rejected(None, "SMTP destination was blocked or unresolved");
    };
    if addresses.is_empty() {
        return Outcome::Rejected(None, "SMTP destination was unresolved");
    }
    let Ok(from) = config.from.parse() else {
        return Outcome::Rejected(None, "SMTP sender is invalid");
    };
    let mut message = Message::builder()
        .from(from)
        .message_id(Some(format!("<{delivery_id}@moleapi.local>")))
        .subject(if settings.language == "zh-CN" {
            "MoleAPI 任务结果"
        } else {
            "MoleAPI task result"
        });
    for recipient in &config.to {
        let Ok(recipient) = recipient.parse() else {
            return Outcome::Rejected(None, "SMTP recipient is invalid");
        };
        message = message.to(recipient);
    }
    let Ok(message) = message.body(text(settings, event)) else {
        return Outcome::Rejected(None, "SMTP message could not be encoded");
    };
    for address in addresses.iter().take(16) {
        let mut transport =
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(address.ip().to_string())
                .port(config.port)
                .timeout(Some(Duration::from_secs(15)));
        if config.tls != "none" {
            let Ok(parameters) = TlsParameters::new(url.host_str().unwrap_or(&config.host).into())
            else {
                return Outcome::Rejected(None, "SMTP TLS configuration is invalid");
            };
            transport = transport.tls(if config.tls == "tls" {
                Tls::Wrapper(parameters)
            } else {
                Tls::Required(parameters)
            });
        }
        if !config.username.is_empty() {
            transport =
                transport.credentials(Login::new(config.username.clone(), config.password.clone()));
        }
        match transport.build().send(message.clone()).await {
            Ok(_) => return Outcome::Accepted(None),
            Err(error) if error.is_permanent() => {
                return Outcome::Rejected(None, "SMTP server rejected the message");
            }
            Err(error) if connection_refused(&error) => continue,
            Err(_) => return Outcome::Retry(None, "SMTP connection or delivery failed"),
        }
    }
    Outcome::Retry(None, "SMTP connection or delivery failed")
}
fn connection_refused(error: &lettre::transport::smtp::Error) -> bool {
    use std::error::Error;
    let mut source = error.source();
    while let Some(value) = source {
        if let Some(io) = value.downcast_ref::<std::io::Error>() {
            return matches!(
                io.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::AddrNotAvailable
                    | std::io::ErrorKind::NetworkUnreachable
                    | std::io::ErrorKind::HostUnreachable
            );
        }
        source = value.source();
    }
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> Event {
        Event {
            kind: "SCHEDULE_RUN_COMPLETED".into(),
            id: "event".into(),
            schedule_id: "schedule".into(),
            job_id: "job".into(),
            status: "failed".into(),
            previous_status: Some("passed".into()),
            finished_at: "now".into(),
            report_id: Some("report".into()),
            passed: 1,
            failed: 1,
            skipped: 0,
            workspace_name: "Space".into(),
            schedule_name: "Test".into(),
        }
    }
    #[test]
    fn provider_payloads_and_receipts_follow_distinct_wire_formats() {
        let credentials = Credentials {
            routing_key: "routing".into(),
            ..Credentials::default()
        };
        for kind in [
            "webhook",
            "slack",
            "teams",
            "wecom",
            "dingtalk",
            "feishu",
            "jenkins",
            "pagerduty",
        ] {
            let settings = Settings {
                name: "Channel".into(),
                kind: kind.into(),
                enabled: true,
                statuses: vec!["failed".into()],
                changes_only: false,
                language: "en".into(),
            };
            let data = payload(&settings, &credentials, &event());
            match kind {
                "slack" => assert!(data["text"].is_string()),
                "teams" => assert_eq!(data["attachments"][0]["content"]["type"], "AdaptiveCard"),
                "wecom" | "dingtalk" => assert_eq!(data["msgtype"], "text"),
                "feishu" => assert_eq!(data["msg_type"], "text"),
                "pagerduty" => {
                    assert_eq!(data["event_action"], "trigger");
                    let mut recovered = event();
                    recovered.status = "passed".into();
                    assert_eq!(
                        payload(&settings, &credentials, &recovered)["event_action"],
                        "resolve"
                    );
                }
                _ => assert_eq!(data["event"], "SCHEDULE_RUN_COMPLETED"),
            }
        }
        assert!(matches!(receipt("slack", 200, b"ok"), Outcome::Accepted(_)));
        assert!(matches!(
            receipt("slack", 200, b"invalid_auth"),
            Outcome::Rejected(_, _)
        ));
        assert!(matches!(
            receipt("feishu", 200, b"{\"code\":0}"),
            Outcome::Accepted(_)
        ));
        assert!(matches!(
            receipt("pagerduty", 202, b"{\"status\":\"success\"}"),
            Outcome::Accepted(_)
        ));
        assert!(matches!(
            receipt("webhook", 429, b"retry"),
            Outcome::Retry(_, _)
        ));
    }
}
