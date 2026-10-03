#[path = "../tests/support/grpc.rs"]
mod fixture;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:18884".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!(
        "gRPC fixture http://{address}; service moleapi.fixture.EchoService; methods Unary, ServerStream, ClientStream, Bidi; Reflection v1/v1alpha"
    );
    fixture::serve_at(listener, false).await?;
    Ok(())
}
