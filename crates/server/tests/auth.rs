mod common;
use common::*;
#[tokio::test]
async fn first_admin_creation_is_serialized_and_requires_setup_token() {
    let dir = tempfile::tempdir().unwrap();
    let router = moleapi_server::hosted(config(
        format!("sqlite://{}?mode=rwc", dir.path().join("auth.db").display()),
        false,
    ))
    .await
    .unwrap();
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/auth/register",
            None,
            Some(json!({"username":"wrong","password":"goodpassword123","setup_token":"wrong"}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (a, b) = tokio::join!(
        call(
            &router,
            "POST",
            "/api/auth/register",
            None,
            Some(
                json!({"username":"first","password":"goodpassword123","setup_token":"setup-secret"})
            )
        ),
        call(
            &router,
            "POST",
            "/api/auth/register",
            None,
            Some(
                json!({"username":"second","password":"goodpassword123","setup_token":"setup-secret"})
            )
        )
    );
    assert!(
        (a.0 == StatusCode::OK && b.0 == StatusCode::FORBIDDEN)
            || (b.0 == StatusCode::OK && a.0 == StatusCode::FORBIDDEN),
        "{a:?} {b:?}"
    );
    assert_eq!(
        call(&router, "GET", "/api/auth/status", None, None).await.1["setup_required"],
        false
    );
}
