mod common;
use common::*;

#[tokio::test]
async fn classic_dubbo_capability_reports_missing_worker_without_exposing_paths() {
    let temporary = tempfile::tempdir().unwrap();
    let router = local(&temporary.path().join("dubbo.db")).await.unwrap();
    let (status, result) = call(&router, "GET", "/api/dubbo/capability", None, None).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["available"], false);
    assert_eq!(result["engine"], "Apache Dubbo 3.3.6 classic/Hessian2");
    assert!(!result.to_string().contains("/tmp/"));
}
