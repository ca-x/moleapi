//! SDK fixtures for actual workbench QA; ports are explicit and isolated.
#[path = "../tests/fixtures/a2a/server.rs"]
mod current;
#[path = "../tests/fixtures/a2a/legacy.rs"]
mod legacy;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dialect = args.get(1).map(String::as_str).unwrap_or("1.0");
    let port = args
        .get(2)
        .map(String::as_str)
        .unwrap_or(if dialect == "0.3" { "18912" } else { "18911" });
    let address = format!("127.0.0.1:{port}");
    let base = format!("http://{address}");
    let app = if dialect == "0.3" {
        legacy::router(&base)
    } else {
        current::router(&base)
    };
    eprintln!("A2A {dialect} SDK fixture {base}");
    axum::serve(tokio::net::TcpListener::bind(address).await?, app).await?;
    Ok(())
}
