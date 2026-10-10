#[allow(dead_code)]
mod common;
use common::*;
use serde_json::{Value, json};
#[tokio::test]
async fn private_scoped_overlays_select_profiles_and_preserve_canonical_sources() {
    let (url,fixture)=serve(axum::Router::new().route("/verify",axum::routing::get(|headers:axum::http::HeaderMap|async move {
        let get=|name:&str|headers.get(name).unwrap().to_str().unwrap();
        let environment=get("x-environment");assert!(matches!(environment,"env-ci-secret"|"prod-saved-secret"));
        assert_eq!(get("x-project"),"project-ci-secret");assert_eq!(get("x-collection"),"collection-ci-secret");assert_eq!(get("x-effective"),"temporary-ci-secret");
        axum::Json(json!({"environment":environment,"project":get("x-project"),"collection":get("x-collection"),"effective":get("x-effective")}))
    }))).await;
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("variables.db");
    let input = temporary.path().join("workspace.json");
    let vars = temporary.path().join("variables.json");
    let report = temporary.path().join("report.json");
    let mut data = source(&url);
    let request = &mut data["data"]["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/verify"));
    request["extractions"] = json!([]);
    request["headers"] = json!([{"id":"env","key":"X-Environment","value":"{{token}}","enabled":true},{"id":"project","key":"X-Project","value":"{{global}}","enabled":true},{"id":"collection","key":"X-Collection","value":"{{col}}","enabled":true},{"id":"effective","key":"X-Effective","value":"{{shared}}","enabled":true}]);
    request["post_response_script"] = json!(
        "pm.test('scopes',()=>{pm.expect(pm.globals.get('global')).to.equal(pm.response.json().project);pm.expect(pm.collectionVariables.get('col')).to.equal(pm.response.json().collection);pm.expect(pm.environment.get('token')).to.equal(pm.response.json().environment);pm.expect(pm.variables.get('shared')).to.equal(pm.response.json().effective);pm.expect(pm.globals.has('deleted')).to.equal(false);pm.expect(pm.environment.has('envgone')).to.equal(false);});"
    );
    data["data"]["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    data["data"]["scenarios"] = json!([]);
    data["data"]["global_variables"] = json!([{"id":"g","key":"global","value":"saved-global","enabled":true},{"id":"gone","key":"deleted","value":"saved-deleted","enabled":true},{"id":"shared","key":"shared","value":"saved-global-shared","enabled":true}]);
    data["data"]["collections"][0]["variables"] = json!([{"id":"col","key":"col","value":"saved-col","enabled":true},{"id":"share","key":"shared","value":"saved-col-shared","enabled":true}]);
    data["data"]["environments"][0]["variables"] = json!([{"id":"tok","key":"token","value":"dev-saved-secret","enabled":true,"secret":true},{"id":"gone","key":"envgone","value":"saved-env-deleted","enabled":true}]);
    data["data"]["environments"][1]["variables"] = json!([{"id":"tok","key":"token","value":"prod-saved-secret","enabled":true,"secret":true}]);
    write(&input, &data);
    let workspace = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "import",
            "--input",
            input.to_str().unwrap(),
        ])
        .await,
        0,
    );
    let id = workspace["id"].as_str().unwrap();
    let baseline_path = temporary.path().join("before.json");
    assert!(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "export",
            "--workspace",
            id,
            "--include-secrets",
            "--output",
            baseline_path.to_str().unwrap()
        ])
        .await
        .status
        .success()
    );
    let baseline: Value = serde_json::from_slice(&std::fs::read(baseline_path).unwrap()).unwrap();
    let overlay = json!({"project":{"global":"project-ci-secret","deleted":null},"collection":{"col":"collection-ci-secret"},"environment":{"token":"env-ci-secret","envgone":null},"temporary":{"shared":"temporary-ci-secret"}});
    write(&vars, &overlay);
    let summary = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "run",
            "--workspace",
            id,
            "--environment",
            "Development",
            "--variables-file",
            vars.to_str().unwrap(),
            "--report",
            report.to_str().unwrap(),
            "--reporter",
            "json",
        ])
        .await,
        0,
    );
    assert_eq!(summary["passed"], 1);
    let contents = std::fs::read_to_string(&report).unwrap();
    let _: Value = serde_json::from_str(&contents).unwrap();
    for value in [
        "env-ci-secret",
        "project-ci-secret",
        "collection-ci-secret",
        "temporary-ci-secret",
    ] {
        assert!(!contents.contains(value));
        assert!(!summary.to_string().contains(value));
    }
    let mut prod = overlay;
    prod["environment"] = json!({});
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .args([
            "--database",
            database.to_str().unwrap(),
            "run",
            "--workspace",
            id,
            "--environment",
            "Production",
            "--variables-env",
            "MOLEAPI_PRIVATE_FIXTURE",
        ])
        .env(
            "MOLEAPI_PRIVATE_FIXTURE",
            serde_json::to_string(&prod).unwrap(),
        )
        .output()
        .await
        .unwrap();
    assert_eq!(result(output, 0)["passed"], 1);
    let export = temporary.path().join("canonical.json");
    let output = cli(&[
        "--database",
        database.to_str().unwrap(),
        "export",
        "--workspace",
        id,
        "--include-secrets",
        "--output",
        export.to_str().unwrap(),
    ])
    .await;
    assert!(output.status.success());
    let saved: Value = serde_json::from_slice(&std::fs::read(export).unwrap()).unwrap();
    assert_eq!(saved["data"], baseline["data"]);
    fixture.abort();
}
#[tokio::test]
async fn invalid_or_missing_private_inputs_do_not_import_or_echo_their_contents() {
    let temporary = tempfile::tempdir().unwrap();
    let database = temporary.path().join("validation.db");
    let input = temporary.path().join("collection.txt");
    std::fs::write(&input, "curl https://example.com/").unwrap();
    let vars = temporary.path().join("vars.json");
    for contents in [
        "{\"temporary\":{\"x\":private-value-should-not-echo}}".to_owned(),
        "{\"unknown\":\"private-value-should-not-echo\"}".to_owned(),
        "private-value-should-not-echo".repeat(50000),
    ] {
        std::fs::write(&vars, &contents).unwrap();
        let output = cli(&[
            "--database",
            database.to_str().unwrap(),
            "run",
            "--input",
            input.to_str().unwrap(),
            "--input-format",
            "curl",
            "--variables-file",
            vars.to_str().unwrap(),
        ])
        .await;
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private-value-should-not-echo"));
    }
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .args([
            "--database",
            database.to_str().unwrap(),
            "run",
            "--input",
            input.to_str().unwrap(),
            "--input-format",
            "curl",
            "--variables-env",
            "MOLEAPI_ABSENT_FIXTURE",
        ])
        .env_remove("MOLEAPI_ABSENT_FIXTURE")
        .output()
        .await
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let list = result(
        cli(&[
            "--database",
            database.to_str().unwrap(),
            "list",
            "workspaces",
        ])
        .await,
        0,
    );
    assert_eq!(list, json!([]));
}

#[tokio::test]
async fn remote_overrides_remain_private_in_reports_history_and_saved_workspace() {
    let (url, fixture) = serve(axum::Router::new().route(
        "/verify",
        axum::routing::get(|headers: axum::http::HeaderMap| async move {
            assert_eq!(headers.get("x-secret").unwrap(), "remote-ci-private");
            axum::Json(json!({"received":"remote-ci-private"}))
        }),
    ))
    .await;
    let temporary = tempfile::tempdir().unwrap();
    let router = moleapi_server::hosted_with_worker(
        moleapi_server::Config {
            database_url: format!(
                "sqlite://{}?mode=rwc",
                temporary.path().join("hosted.db").display()
            ),
            setup_token: "fixture-setup".into(),
            allow_registration: true,
            allow_private_network: true,
        },
        std::path::Path::new(env!("CARGO_BIN_EXE_moleapi-cli")),
    )
    .await
    .unwrap();
    let (server, handle) = serve(router).await;
    let http = reqwest::Client::new();
    let session:Value=http.post(format!("{server}/api/auth/register")).json(&json!({"username":"variablesowner","password":"fixture-password-123","setup_token":"fixture-setup"})).send().await.unwrap().error_for_status().unwrap().json().await.unwrap();
    let session = session["token"].as_str().unwrap();
    let token_file = temporary.path().join("session.txt");
    std::fs::write(&token_file, session).unwrap();
    let mut data = source(&url);
    let request = &mut data["data"]["collections"][0]["requests"][0];
    request["url"] = json!(format!("{url}/verify"));
    request["headers"] = json!([{"id":"s","key":"X-Secret","value":"{{secret}}","enabled":true}]);
    request["extractions"] = json!([]);
    request["post_response_script"] = json!(
        "pm.test('received',()=>pm.expect(pm.response.json().received).to.equal(pm.variables.get('secret')));"
    );
    data["data"]["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    data["data"]["scenarios"] = json!([]);
    let before: Value = http
        .post(format!("{server}/api/workspaces"))
        .bearer_auth(session)
        .json(&json!({"name":"Remote variables","data":data["data"]}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = before["id"].as_str().unwrap();
    let report = temporary.path().join("report.json");
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_moleapi-cli"))
        .args([
            "--server",
            &server,
            "--token-file",
            token_file.to_str().unwrap(),
            "run",
            "--workspace",
            id,
            "--environment",
            "Development",
            "--variables-env",
            "MOLEAPI_PRIVATE_REMOTE",
            "--report",
            report.to_str().unwrap(),
            "--reporter",
            "json",
        ])
        .env(
            "MOLEAPI_PRIVATE_REMOTE",
            r#"{"temporary":{"secret":"remote-ci-private"}}"#,
        )
        .output()
        .await
        .unwrap();
    let summary = result(output, 0);
    assert_eq!(summary["passed"], 1);
    assert!(!summary.to_string().contains("remote-ci-private"));
    assert!(
        !std::fs::read_to_string(report)
            .unwrap()
            .contains("remote-ci-private")
    );
    let history: Value = http
        .get(format!("{server}/api/workspaces/{id}/history"))
        .bearer_auth(session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!history.to_string().contains("remote-ci-private"));
    let after: Value = http
        .get(format!("{server}/api/workspaces/{id}"))
        .bearer_auth(session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(before, after);
    handle.abort();
    fixture.abort();
}
