#[path = "../tests/fixtures/mcp/server.rs"]
mod fixture;
use rmcp::ServiceExt;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().any(|arg| arg == "--oversize-stdio") {
        use tokio::io::AsyncWriteExt;
        tokio::io::stdout()
            .write_all(&vec![b'x'; 2 * 1024 * 1024])
            .await?;
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    } else if std::env::args().any(|arg| arg == "--stdio") {
        fixture::Fixture::default()
            .serve(rmcp::transport::stdio())
            .await?
            .waiting()
            .await?;
    } else {
        let address = std::env::var("MOLEAPI_MCP_FIXTURE_LISTEN")
            .unwrap_or_else(|_| "127.0.0.1:18901".into());
        let listener = tokio::net::TcpListener::bind(address).await?;
        axum::serve(listener, fixture::router(false)).await?;
    }
    Ok(())
}
