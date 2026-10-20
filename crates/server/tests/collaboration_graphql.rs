mod common;
use async_graphql::{Object, Schema, Subscription};
use common::*;
use futures_util::{Stream, stream};
use std::time::Duration;
struct Query;
#[Object]
impl Query {
    async fn hello(&self, name: String) -> String {
        format!("Hello {name}")
    }
}
struct Mutation;
#[Object]
impl Mutation {
    async fn double(&self, value: i32) -> i32 {
        value * 2
    }
}
struct Subscriptions;
#[Subscription]
impl Subscriptions {
    async fn ticks(&self) -> impl Stream<Item = i32> {
        stream::iter([1, 2, 3])
    }
    async fn waiting(&self) -> impl Stream<Item = i32> {
        stream::pending()
    }
}
#[tokio::test]
async fn members_execute_graphql_schema_and_saved_subscriptions_in_the_owner_namespace() {
    let schema = Schema::build(Query, Mutation, Subscriptions).finish();
    let http = schema.clone();
    let (url, fixture) = serve(
        Router::new()
            .route(
                "/graphql",
                axum::routing::post(
                    move |axum::Json(request): axum::Json<async_graphql::Request>| {
                        let schema = http.clone();
                        async move { axum::Json(schema.execute(request).await) }
                    },
                ),
            )
            .route_service(
                "/subscriptions",
                async_graphql_axum::GraphQLSubscription::new(schema),
            ),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let mut settings = config(
        moleapi_server::sqlite_database_url(&dir.path().join("graphql.db")).unwrap(),
        true,
    );
    settings.allow_private_network = true;
    let router = hosted(settings).await.unwrap();
    let owner = register(&router, "gqlowner").await;
    let editor = register(&router, "gqleditor").await;
    let viewer = register(&router, "gqlviewer").await;
    let foreign = register(&router, "gqlforeign").await;
    let mut data = example_data();
    let request = &mut data["collections"][0]["requests"][0];
    request["url"] = format!("{url}/graphql").into();
    request["examples"] = json!([]);
    request["protocol"] = json!({"kind":"graphql","document":"query Hello($name:String!){hello(name:$name)}","variables":{"name":"owner"},"operation_name":"Hello"});
    let mut subscription = request.clone();
    subscription["id"] = "subscription".into();
    subscription["protocol"] = json!({"kind":"graphql","document":"subscription Watch{ticks}","operation_name":"Watch","variables":{},"subscription_url":format!("{}/subscriptions",url.replace("http:","ws:")),"connection_params":{}});
    data["collections"][0]["requests"]
        .as_array_mut()
        .unwrap()
        .push(subscription);
    let (status, saved) = call(
        &router,
        "POST",
        "/api/workspaces",
        Some(&owner),
        Some(json!({"id":"w","name":"GraphQL","data":data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let mut collision = data.clone();
    collision["collections"][0]["requests"][0]["protocol"]["variables"]["name"] =
        "wrong-member".into();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/workspaces",
            Some(&editor),
            Some(json!({"id":"w","name":"Collision","data":collision}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut base = String::new();
    let mut member_id = Value::Null;
    for (token, role) in [(&editor, "editor"), (&viewer, "viewer")] {
        let (_, invite) = call(
            &router,
            "POST",
            "/api/workspaces/w/invitations",
            Some(&owner),
            Some(json!({"role":role,"expires_in_days":1})),
        )
        .await;
        let (status, accepted) = call(
            &router,
            "POST",
            "/api/workspace-invitations/accept",
            Some(token),
            Some(json!({"token":invite["token"]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        if role == "editor" {
            member_id = accepted["member_id"].clone();
        }
        base = format!(
            "/api/owners/{}/workspaces/w",
            accepted["owner_id"].as_str().unwrap()
        );
    }
    let (status, response) = call(
        &router,
        "POST",
        &format!("{base}/requests/r/execute"),
        Some(&editor),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let body: Value = serde_json::from_str(response["body"].as_str().unwrap()).unwrap();
    assert_eq!(body["data"]["hello"], "Hello owner");
    let mut draft = saved["data"]["collections"][0]["requests"][0].clone();
    draft["protocol"] = json!({"kind":"graphql","document":"mutation Double($value:Int!){double(value:$value)}","variables":{"value":7},"operation_name":"Double"});
    let input = json!({"collection_id":"c","expected_workspace_revision":saved["revision"],"request":draft});
    let (status, response) = call(
        &router,
        "POST",
        &format!("{base}/execute-draft"),
        Some(&editor),
        Some(input.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        serde_json::from_str::<Value>(response["body"].as_str().unwrap()).unwrap()["data"]["double"],
        14
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{base}/execute-draft"),
            Some(&viewer),
            Some(input.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let introspect = json!({"request_id":"r","expected_revision":saved["revision"]});
    let (status, result) = call(
        &router,
        "POST",
        &format!("{base}/graphql/introspect"),
        Some(&editor),
        Some(introspect.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(result["sdl"].as_str().unwrap().contains("hello"));
    assert_eq!(result["specification"]["kind"], "graphql-introspection");
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{base}/graphql/introspect"),
            Some(&viewer),
            Some(introspect.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{base}/graphql/introspect"),
            Some(&foreign),
            Some(introspect.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (_, unchanged) = call(&router, "GET", &base, Some(&editor), None).await;
    assert_eq!(unchanged["data"], saved["data"]);
    let mut linked = unchanged["data"].clone();
    linked["specifications"] = json!([result["specification"]]);
    linked["collections"][0]["requests"][0]["specification_id"] =
        result["specification"]["id"].clone();
    let (status, updated) = call(
        &router,
        "PUT",
        &base,
        Some(&editor),
        Some(json!({"name":"GraphQL","expected_revision":saved["revision"],"data":linked})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let source = json!({"request_id":"r","specification_id":result["specification"]["id"],"expected_revision":updated["revision"]});
    let (status, schema) = call(
        &router,
        "POST",
        &format!("{base}/graphql/schema"),
        Some(&editor),
        Some(source.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{schema}");
    assert_eq!(schema["sdl"], result["sdl"]);
    let mut wrong = source.clone();
    wrong["request_id"] = "subscription".into();
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{base}/graphql/schema"),
            Some(&editor),
            Some(wrong)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{base}/graphql/introspect"),
            Some(&editor),
            Some(introspect)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (status, session) = call(
        &router,
        "POST",
        &format!("{base}/sessions"),
        Some(&editor),
        Some(json!({"request_id":"subscription","expected_revision":updated["revision"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let path = format!("{base}/sessions/{}", session["id"].as_str().unwrap());
    let events = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let (_, batch) = call(
                &router,
                "GET",
                &format!("{path}/events?after=0"),
                Some(&editor),
                None,
            )
            .await;
            if batch["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["message"]["kind"] == "graphql_complete")
            {
                break batch;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["message"]["kind"] == "graphql_next")
            .count(),
        3
    );
    assert_eq!(
        call(&router, "GET", &path, Some(&viewer), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{base}/sessions"),
            Some(&editor),
            Some(json!({"request_id":"r","expected_revision":updated["revision"]}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut subscription_draft = input;
    subscription_draft["expected_workspace_revision"] = updated["revision"].clone();
    subscription_draft["request"]["protocol"] =
        json!({"kind":"graphql","document":"subscription Watch{ticks}","variables":{}});
    assert_eq!(
        call(
            &router,
            "POST",
            &format!("{base}/execute-draft"),
            Some(&editor),
            Some(subscription_draft)
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    let mut pending_data = updated["data"].clone();
    pending_data["collections"][0]["requests"][1]["protocol"]["document"] =
        "subscription Waiting {waiting}".into();
    pending_data["collections"][0]["requests"][1]["protocol"]["operation_name"] = "Waiting".into();
    let (status, waiting_workspace) = call(
        &router,
        "PUT",
        &base,
        Some(&editor),
        Some(json!({"name":"GraphQL","expected_revision":updated["revision"],"data":pending_data})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{waiting_workspace}");
    for boundary in ["role", "logout"] {
        let(status,session)=call(&router,"POST",&format!("{base}/sessions"),Some(&editor),Some(json!({"request_id":"subscription","expected_revision":waiting_workspace["revision"]}))).await;
        assert_eq!(status, StatusCode::OK, "{session}");
        let id = session["id"].as_str().unwrap();
        let path = format!("{base}/sessions/{id}");
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let (_, summary) = call(&router, "GET", &path, Some(&editor), None).await;
                if summary["state"] == "open" {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        if boundary == "role" {
            assert_eq!(
                call(
                    &router,
                    "PATCH",
                    &format!("{base}/members/{}", member_id.as_str().unwrap()),
                    Some(&owner),
                    Some(json!({"role":"viewer","expected_revision":1}))
                )
                .await
                .0,
                StatusCode::OK
            );
            assert_eq!(
                call(&router, "GET", &path, Some(&editor), None).await.0,
                StatusCode::NOT_FOUND
            );
        } else {
            assert_eq!(
                call(&router, "POST", "/api/auth/logout", Some(&editor), None)
                    .await
                    .0,
                StatusCode::OK
            );
            assert_eq!(
                call(&router, "GET", &path, Some(&editor), None).await.0,
                StatusCode::UNAUTHORIZED
            );
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if call(
                    &router,
                    "GET",
                    &format!("/api/sessions/{id}"),
                    Some(&owner),
                    None,
                )
                .await
                .0 == StatusCode::NOT_FOUND
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        if boundary == "role" {
            assert_eq!(
                call(
                    &router,
                    "PATCH",
                    &format!("{base}/members/{}", member_id.as_str().unwrap()),
                    Some(&owner),
                    Some(json!({"role":"editor","expected_revision":2}))
                )
                .await
                .0,
                StatusCode::OK
            );
        }
    }
    let (_, audit) = call(&router, "GET", &format!("{base}/audit"), Some(&owner), None).await;
    assert!(audit.to_string().contains("graphql.introspect"));
    assert!(audit.to_string().contains("graphql.schema.read"));
    assert!(!audit.to_string().contains("mutation Double"));
    fixture.abort();
}
