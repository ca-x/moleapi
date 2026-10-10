#![cfg(unix)]
#[allow(dead_code)]
mod common;
use common::*;
use serde_json::{Value, json};
#[tokio::test]
async fn generated_commit_test_script_runs_real_collection_and_preserves_failure_exit() {
    let (url, fixture) = serve(axum::Router::new().route(
        "/",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(
                headers.get("x-private").unwrap(),
                "runtime-private-ci-value"
            );
            axum::Json(json!({"ok":true}))
        }),
    ))
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let input = temporary
        .path()
        .join("-collection $(touch injected) 'quoted'.json");
    let config = temporary.path().join("ci.json");
    let database = temporary.path().join("must-not-exist/db.sqlite");
    let suite = "-Suite $(touch injected) 'quoted'";
    let mut collection = json!({"info":{"name":suite,"schema":"https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},"item":[{"name":"Echo","request":{"method":"GET","url":url,"header":[{"key":"X-Private","value":"{{injected}}"}]},"event":[{"listen":"test","script":{"type":"text/javascript","exec":["pm.test('ok',()=>{pm.expect(pm.response.json().ok).to.equal(true);pm.expect(pm.variables.has('injected')).to.equal(true);});"]}}]}]});
    write(&input, &collection);
    write(
        &config,
        &json!({"provider":"github","source":{"kind":"file","path":input.file_name().unwrap().to_str().unwrap(),"format":"postman"},"collection":suite,"iterations":2,"variables_secret":"CI_VARIABLES_JSON"}),
    );
    let preset = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "ci",
            "--config",
            config.to_str().unwrap(),
        ])
        .await,
        0,
    );
    assert!(!database.exists());
    let yaml = temporary.path().join("workflow.yml");
    let output = cli(&[
        "ci",
        "--config",
        config.to_str().unwrap(),
        "--output",
        yaml.to_str().unwrap(),
    ])
    .await;
    assert!(output.status.success());
    assert_eq!(
        std::fs::read_to_string(&yaml).unwrap(),
        preset["content"].as_str().unwrap()
    );
    assert_eq!(
        cli(&[
            "ci",
            "--config",
            config.to_str().unwrap(),
            "--output",
            yaml.to_str().unwrap()
        ])
        .await
        .status
        .code(),
        Some(2)
    );
    let doc: Value = serde_yaml_ng::from_str(preset["content"].as_str().unwrap()).unwrap();
    let script = doc["jobs"]["api-tests"]["steps"][1]["run"]
        .as_str()
        .unwrap();
    let directory = std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .parent()
        .unwrap();
    let path = std::env::join_paths(
        std::iter::once(directory.to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let run = tokio::process::Command::new("sh")
        .args(["-c", script])
        .env("PATH", &path)
        .env(
            "MOLEAPI_RUN_VARIABLES",
            r#"{"temporary":{"injected":"runtime-private-ci-value"}}"#,
        )
        .current_dir(temporary.path())
        .output()
        .await
        .unwrap();
    let summary = result(run, 0);
    assert_eq!(summary["passed"], 2);
    assert!(!temporary.path().join("injected").exists());
    let xml = std::fs::read_to_string(temporary.path().join(".moleapi-ci/report.xml")).unwrap();
    roxmltree::Document::parse(&xml).unwrap();
    collection["item"][0]["event"][0]["script"]["exec"] =
        json!(["pm.test('failure',()=>pm.expect(true).to.equal(false));"]);
    write(&input, &collection);
    let failed = tokio::process::Command::new("sh")
        .args(["-c", script])
        .env("PATH", &path)
        .env(
            "MOLEAPI_RUN_VARIABLES",
            r#"{"temporary":{"injected":"runtime-private-ci-value"}}"#,
        )
        .current_dir(temporary.path())
        .output()
        .await
        .unwrap();
    let summary = result(failed, 1);
    assert_eq!(summary["failed"], 2);
    let xml = std::fs::read_to_string(temporary.path().join(".moleapi-ci/report.xml")).unwrap();
    let report = roxmltree::Document::parse(&xml).unwrap();
    assert!(
        report
            .descendants()
            .any(|node| node.has_tag_name("failure"))
    );
    assert!(!temporary.path().join("injected").exists());
    fixture.abort();
}
