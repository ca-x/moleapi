//! Real official Apache Dubbo provider + shaded worker acceptance, never a user request.
use anyhow::{Result, ensure};
use moleapi_core::{NetworkPolicy, Protocol, RequestSpec};
use moleapi_protocols::dubbo::DubboRuntime;
use serde_json::{Value, json};
use std::path::Path;

fn request(port: u16, method: &str, types: &[&str], sources: &[&str]) -> Result<RequestSpec> {
    ensure!(types.len() == sources.len());
    Ok(serde_json::from_value(json!({
      "id":"r","name":"Dubbo QA","method":"GET","url":format!("dubbo://127.0.0.1:{port}"),
      "description":"","query":[],"headers":[],"body_kind":"none","body":"",
      "auth":{"kind":"none","token":"","username":"","password":""},
      "timeout_ms":10000,"follow_redirects":false,"verify_tls":true,"assertions":[],"examples":[],
      "protocol":{"kind":"dubbo","source_version":1,"transport":"classic","serialization":"hessian2",
        "service":"fixture.EchoService","method":method,"version":"1.0.0","group":"",
        "arguments":types.iter().zip(sources).enumerate().map(|(i,(t,s))|json!({"id":format!("arg-{i}"),"type_name":t,"value_source":s,"secret":false})).collect::<Vec<_>>(),
        "attachments":[]}
    }))?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let java = args.next().expect("Java executable");
    let jar = args.next().expect("Dubbo worker JAR");
    let port: u16 = args.next().expect("provider port").parse()?;
    ensure!(args.next().is_none(), "Unexpected fixture arguments");
    let runtime = DubboRuntime::new(Path::new(&jar))?;
    let allow = NetworkPolicy {
        allow_private_network: true,
    };
    let cases = [
        (
            "echo",
            vec!["java.lang.String"],
            vec![r#""moleapi""#],
            json!("echo:moleapi"),
        ),
        (
            "join",
            vec!["java.lang.String", "java.lang.String"],
            vec![r#""first""#, r#""second""#],
            json!("first|second"),
        ),
        (
            "add",
            vec!["long", "long"],
            vec![r#""9007199254740993""#, "2"],
            json!(9007199254740995_i64),
        ),
        (
            "describe",
            vec!["fixture.Greeting"],
            vec![r#"{"$class":"fixture.Greeting","$":{"name":"鼹鼠","count":0}}"#],
            json!("鼹鼠:0"),
        ),
        (
            "names",
            vec!["java.util.List"],
            vec![r#"["first","鼹鼠"]"#],
            json!("first|鼹鼠"),
        ),
        (
            "maybeNull",
            vec!["java.lang.String"],
            vec![r#""null""#],
            Value::Null,
        ),
    ];
    for (method, types, sources, expected) in cases {
        let result = runtime
            .invoke(
                Path::new(&java),
                &request(port, method, &types, &sources)?,
                allow,
            )
            .await?;
        ensure!(
            result.status == 200 && serde_json::from_str::<Value>(&result.body)? == expected,
            "Dubbo {method} response mismatch: {}",
            result.body
        );
        println!("PASS Rust host / {method}");
    }
    let failure = runtime
        .invoke(
            Path::new(&java),
            &request(port, "fail", &["java.lang.String"], &[r#""expected""#])?,
            allow,
        )
        .await?;
    ensure!(
        failure.status == 500 && failure.body.contains("fixture:expected"),
        "Remote exception was lost"
    );
    println!("PASS Rust host / remote exception");
    let mut grouped = request(port, "echo", &["java.lang.String"], &[r#""grouped""#])?;
    if let Protocol::Dubbo { config } = &mut grouped.protocol {
        config.group = "qa".into();
    }
    let result = runtime.invoke(Path::new(&java), &grouped, allow).await?;
    ensure!(result.body == r#""echo:grouped""#, "Dubbo group was lost");
    println!("PASS Rust host / service group");
    let mut attached = request(
        port,
        "attachment",
        &["java.lang.String"],
        &[r#""trace-id""#],
    )?;
    if let Protocol::Dubbo { config } = &mut attached.protocol {
        config.attachments.push(serde_json::from_value(json!({"id":"trace","key":"trace-id","value":"host-attachment","enabled":true,"secret":true}))?);
    }
    let result = runtime.invoke(Path::new(&java), &attached, allow).await?;
    ensure!(
        result.body == r#""host-attachment""#,
        "Dubbo attachment was lost"
    );
    println!("PASS Rust host / request attachment");
    let blocked = runtime
        .invoke(
            Path::new(&java),
            &request(port, "echo", &["java.lang.String"], &[r#""moleapi""#])?,
            NetworkPolicy {
                allow_private_network: false,
            },
        )
        .await;
    ensure!(
        blocked.is_err_and(|error| error.to_string().contains("blocked")),
        "Hosted private policy allowed loopback"
    );
    println!("PASS Rust host / private network refusal");
    Ok(())
}
