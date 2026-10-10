mod common;
use common::*;
fn configuration() -> Value {
    json!({"provider":"github","source":{"kind":"remote","server":"https://api.example.com","workspace":"w"},"collection":"c","environment":"dev","dataset":"rows","requests":["r"],"notifications":{"mode":"silent"}})
}
#[tokio::test]
async fn owned_ci_presets_validate_revision_sources_selections_and_credentials() {
    let temporary = tempfile::tempdir().unwrap();
    let router = hosted(config(
        format!(
            "sqlite://{}?mode=rwc",
            temporary.path().join("ci.db").display()
        ),
        true,
    ))
    .await
    .unwrap();
    let owner = register(&router, "ciowner").await;
    let other = register(&router, "otherowner").await;
    let mut data = example_data();
    data["environments"] = json!([{"id":"dev","name":"Development","variables":[]}]);
    data["datasets"] =
        json!([{"id":"rows","name":"Rows","source":{"format":"csv","source":"value\none\n"}}]);
    let (status, workspace) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"CI","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let payload = json!({"expected_revision":workspace["revision"],"config":configuration()});
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/ci-preset",
            Some(&other),
            Some(payload.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces/w/ci-preset",
            None,
            Some(payload.clone())
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, preset) = call(
        &router,
        "POST",
        "/api/workspaces/w/ci-preset",
        Some(&owner),
        Some(payload.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preset}");
    assert!(
        preset["content"]
            .as_str()
            .unwrap()
            .contains("secrets.MOLEAPI_TOKEN")
    );
    assert!(
        preset["command"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "--dataset=rows")
    );
    for (path, value, status) in [
        ("/expected_revision", json!(999), StatusCode::CONFLICT),
        (
            "/config/source/workspace",
            json!("other"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/config/environment",
            json!("missing"),
            StatusCode::NOT_FOUND,
        ),
        (
            "/config/requests",
            json!(["missing"]),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/config/requests",
            json!(["r", "r"]),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/config/source/server",
            json!("https://user:password@example.com"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "/config/notifications",
            json!({"mode":"selected","ids":["missing"]}),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let mut modified = payload.clone();
        *modified
            .pointer_mut(path)
            .unwrap_or_else(|| panic!("missing {path}")) = value;
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/workspaces/w/ci-preset",
                Some(&owner),
                Some(modified)
            )
            .await
            .0,
            status,
            "{path}"
        );
    }
    let (_, history) = call(
        &router,
        "GET",
        "/api/workspaces/w/history",
        Some(&owner),
        None,
    )
    .await;
    assert_eq!(history, json!([]));
    let (_, saved) = call(&router, "GET", "/api/workspaces/w", Some(&owner), None).await;
    assert_eq!(saved, workspace);
}
