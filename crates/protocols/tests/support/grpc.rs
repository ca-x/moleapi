//! Real generated tonic fixture shared by integration tests and the manual browser target.
#![allow(dead_code)]
use futures_util::Stream;
use std::{pin::Pin, time::Duration};
use tonic::{Request, Response, Status, metadata::MetadataMap};
#[allow(clippy::all)]
pub mod pb {
    include!("../fixtures/moleapi.fixture.rs");
}
use pb::{
    Echo,
    echo_service_server::{EchoService, EchoServiceServer},
};
pub const DESCRIPTORS: &[u8] = include_bytes!("../fixtures/schema.bin");
pub struct Fixture;
fn auth<T>(request: &Request<T>) -> Result<(), Status> {
    let expected = if request
        .metadata()
        .get("x-require-auth")
        .and_then(|value| value.to_str().ok())
        == Some("basic")
    {
        "Basic dXNlcjpwYXNz"
    } else {
        "Bearer fixture-secret"
    };
    if request.metadata().get("x-require-auth").is_some()
        && request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            != Some(expected)
    {
        return Err(Status::unauthenticated(
            "fixture requires bearer authentication",
        ));
    }
    Ok(())
}
fn response<T>(value: T) -> Response<T> {
    let mut response = Response::new(value);
    response
        .metadata_mut()
        .insert("x-fixture", "tonic".parse().unwrap());
    response.metadata_mut().insert_bin(
        "x-fixture-bin",
        tonic::metadata::MetadataValue::from_bytes(&[0, 255]),
    );
    response
}
fn rich_error() -> Status {
    let mut metadata = MetadataMap::new();
    metadata.insert("x-trailer", "rich-error".parse().unwrap());
    Status::with_details_and_metadata(
        tonic::Code::PermissionDenied,
        "fixture rejected this message",
        vec![0, 255, 42].into(),
        metadata,
    )
}
type EchoStream = Pin<Box<dyn Stream<Item = Result<Echo, Status>> + Send + 'static>>;
#[tonic::async_trait]
impl EchoService for Fixture {
    async fn unary(&self, request: Request<Echo>) -> Result<Response<Echo>, Status> {
        auth(&request)?;
        let echo = request.into_inner();
        if echo.text == "error" {
            return Err(rich_error());
        }
        if echo.text == "wait" {
            tokio::time::sleep(Duration::from_secs(20)).await;
        }
        Ok(response(echo))
    }
    type ServerStreamStream = EchoStream;
    async fn server_stream(&self, request: Request<Echo>) -> Result<Response<EchoStream>, Status> {
        auth(&request)?;
        let echo = request.into_inner();
        let wait = echo.text == "wait";
        let stream = futures_util::stream::unfold((echo, 0), move |(echo, n)| async move {
            if wait && n == 1 {
                tokio::time::sleep(Duration::from_secs(20)).await;
            }
            if n < 3 {
                let mut next = echo.clone();
                next.count = n;
                Some((Ok(next), (echo, n + 1)))
            } else if n == 3 {
                let mut status = Status::ok("");
                status
                    .metadata_mut()
                    .insert("x-trailer", "completed".parse().unwrap());
                Some((Err(status), (echo, n + 1)))
            } else {
                None
            }
        });
        Ok(response(Box::pin(stream)))
    }
    async fn client_stream(
        &self,
        request: Request<tonic::Streaming<Echo>>,
    ) -> Result<Response<Echo>, Status> {
        auth(&request)?;
        let mut stream = request.into_inner();
        let mut result = Echo::default();
        while let Some(message) = stream.message().await? {
            result.text.push_str(&message.text);
            result.count += 1;
        }
        Ok(response(result))
    }
    type BidiStream = EchoStream;
    async fn bidi(
        &self,
        request: Request<tonic::Streaming<Echo>>,
    ) -> Result<Response<EchoStream>, Status> {
        auth(&request)?;
        Ok(response(Box::pin(request.into_inner())))
    }
}
fn reflection_auth(request: Request<()>) -> Result<Request<()>, Status> {
    auth(&request)?;
    Ok(request)
}
pub async fn serve_at(
    listener: tokio::net::TcpListener,
    alpha_only: bool,
) -> Result<(), tonic::transport::Error> {
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(DESCRIPTORS);
    if alpha_only {
        tonic::transport::Server::builder()
            .add_service(EchoServiceServer::new(Fixture))
            .add_service(tonic::service::interceptor::InterceptedService::new(
                reflection.build_v1alpha().unwrap(),
                reflection_auth,
            ))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
    } else {
        tonic::transport::Server::builder()
            .add_service(EchoServiceServer::new(Fixture))
            .add_service(tonic::service::interceptor::InterceptedService::new(
                reflection.build_v1().unwrap(),
                reflection_auth,
            ))
            .add_service(
                tonic_reflection::server::Builder::configure()
                    .register_encoded_file_descriptor_set(DESCRIPTORS)
                    .build_v1alpha()
                    .unwrap(),
            )
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
    }
}
pub async fn start_tls() -> (String, tokio::task::JoinHandle<()>) {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let identity = tonic::transport::Identity::from_pem(
        certified.cert.pem(),
        certified.signing_key.serialize_pem(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "https://localhost:{}",
        listener.local_addr().unwrap().port()
    );
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .tls_config(tonic::transport::ServerTlsConfig::new().identity(identity))
            .unwrap()
            .add_service(EchoServiceServer::new(Fixture))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    (url, task)
}
pub async fn start(alpha_only: bool) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        serve_at(listener, alpha_only).await.unwrap();
    });
    (url, task)
}
