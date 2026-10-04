//! Tonic supplies HTTP/2, gRPC framing, streaming and Reflection; prost-reflect supplies protobuf.
use crate::*;
use anyhow::{Context, Result, ensure};
use hyper_util::rt::TokioIo;
use moleapi_core::{
    Pair, checked_destination, grpc_json, grpc_message, protocol_url, request_headers,
};
use prost::Message as ProstMessage;
use prost_reflect::{DynamicMessage, MessageDescriptor};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    Status,
    codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder},
    metadata::{MetadataKey, MetadataMap, MetadataValue},
    transport::{Channel, ClientTlsConfig, Endpoint},
};
use tower::service_fn;

#[derive(Clone)]
pub(crate) struct DynamicCodec(pub MessageDescriptor);
pub(crate) struct DynamicEncoder;
pub(crate) struct DynamicDecoder(pub MessageDescriptor);
impl Codec for DynamicCodec {
    type Encode = DynamicMessage;
    type Decode = DynamicMessage;
    type Encoder = DynamicEncoder;
    type Decoder = DynamicDecoder;
    fn encoder(&mut self) -> Self::Encoder {
        DynamicEncoder
    }
    fn decoder(&mut self) -> Self::Decoder {
        DynamicDecoder(self.0.clone())
    }
}
impl Encoder for DynamicEncoder {
    type Item = DynamicMessage;
    type Error = Status;
    fn encode(&mut self, message: Self::Item, buf: &mut EncodeBuf<'_>) -> Result<(), Status> {
        message
            .encode(buf)
            .map_err(|e| Status::internal(e.to_string()))
    }
}
impl Decoder for DynamicDecoder {
    type Item = DynamicMessage;
    type Error = Status;
    fn decode(&mut self, buf: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Status> {
        DynamicMessage::decode(self.0.clone(), buf)
            .map(Some)
            .map_err(|e| Status::internal(e.to_string()))
    }
}
// Explicit opt-out of certificate trust/hostname validation; rustls still performs
// TLS framing, encryption, SNI and cryptographic handshake-signature validation.
#[derive(Debug)]
pub(crate) struct UnverifiedCertificate;
impl rustls::client::danger::ServerCertVerifier for UnverifiedCertificate {
    fn verify_server_cert(
        &self,
        _certificate: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &rustls::pki_types::CertificateDer<'_>,
        signature: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            certificate,
            signature,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &rustls::pki_types::CertificateDer<'_>,
        signature: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            certificate,
            signature,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
pub(crate) async fn channel(request: &RequestSpec, policy: NetworkPolicy) -> Result<Channel> {
    let url = protocol_url(&request.url, false)?;
    ensure!(
        url.path() == "/" && url.query().is_none() && url.fragment().is_none(),
        "gRPC endpoint must be an origin without a path, query or fragment"
    );

    let addresses = checked_destination(&url, policy).await?;
    let mut endpoint = Endpoint::from_shared(url.to_string())?
        .connect_timeout(Duration::from_millis(request.timeout_ms))
        .http2_adaptive_window(true)
        .buffer_size(32);
    if url.scheme() == "https" {
        let tls = ClientTlsConfig::new().domain_name(
            url.host_str()
                .context("Missing gRPC host")?
                .trim_matches(['[', ']']),
        );
        endpoint = if request.verify_tls {
            endpoint.tls_config(tls.with_native_roots())?
        } else {
            endpoint.tls_config_with_verifier(tls, Arc::new(UnverifiedCertificate))?
        };
    }
    // Tonic wraps this TCP stream in its own TLS connector using the original URI
    // and hostname. Reconnects use only the previously checked SocketAddr list.
    Ok(endpoint
        .connect_with_connector(service_fn(move |_uri| {
            let addresses = addresses.clone();
            async move {
                tokio::net::TcpStream::connect(addresses.as_slice())
                    .await
                    .map(TokioIo::new)
            }
        }))
        .await?)
}
pub(crate) fn metadata(request: &RequestSpec) -> Result<MetadataMap> {
    let headers = request_headers(request)?;
    let mut result = MetadataMap::new();
    let mut bytes = 0usize;
    for (name, value) in &headers {
        let name = name.as_str();
        ensure!(
            !name.starts_with("grpc-")
                && !matches!(
                    name,
                    "content-type"
                        | "te"
                        | "host"
                        | "connection"
                        | "transfer-encoding"
                        | "content-length"
                        | "upgrade"
                        | "keep-alive"
                        | "proxy-connection"
                ),
            "Reserved gRPC metadata header: {name}"
        );
        let text = value.to_str().context("Invalid gRPC metadata")?;
        bytes += name.len() + text.len();
        ensure!(
            bytes <= 32 * 1024 && result.len() < 128,
            "gRPC metadata exceeds limit"
        );
        if name.ends_with("-bin") {
            let decoded = STANDARD
                .decode(text)
                .context("Binary gRPC metadata requires base64")?;
            result.append_bin(
                MetadataKey::from_bytes(name.as_bytes())?,
                MetadataValue::from_bytes(&decoded),
            );
        } else {
            result.append(MetadataKey::from_bytes(name.as_bytes())?, text.parse()?);
        }
    }
    Ok(result)
}
fn safe_binary(bytes: &[u8], mask: &dyn Fn(&str) -> String) -> String {
    let text = String::from_utf8_lossy(bytes);
    if mask(&text) != text {
        "[REDACTED]".into()
    } else {
        mask(&STANDARD.encode(bytes))
    }
}
fn pairs(metadata: &MetadataMap, mask: &dyn Fn(&str) -> String) -> Result<Vec<Pair>> {
    let mut pairs = Vec::new();
    let mut bytes = 0usize;
    for entry in metadata.iter() {
        let (key, value) = match entry {
            tonic::metadata::KeyAndValueRef::Ascii(k, v) => {
                (k.as_str(), v.to_str().unwrap_or("[binary]").into())
            }
            tonic::metadata::KeyAndValueRef::Binary(k, v) => {
                (k.as_str(), safe_binary(&v.to_bytes()?, mask))
            }
        };
        bytes += key.len() + value.len();
        ensure!(
            bytes <= 32 * 1024 && pairs.len() < 128,
            "Received gRPC metadata exceeds limit"
        );
        pairs.push(Pair {
            id: uuid::Uuid::new_v4().to_string(),
            key: key.into(),
            value: if matches!(key, "authorization" | "set-cookie" | "cookie" | "x-api-key") {
                "[REDACTED]".into()
            } else {
                mask(&value)
            },
            enabled: true,
            secret: None,
            local_value: None,
        });
    }
    Ok(pairs)
}
fn safe_json(message: &DynamicMessage, mask: &dyn Fn(&str) -> String) -> Result<serde_json::Value> {
    // A private value inside a protobuf bytes field may be hidden by base64 JSON
    // alignment. Conservatively withhold the message when its decoded wire bytes
    // contain a private execution value.
    let encoded = message.encode_to_vec();
    let wire_text = String::from_utf8_lossy(&encoded);
    if mask(&wire_text) != wire_text {
        return Ok(serde_json::json!("[REDACTED]"));
    }
    let text = serde_json::to_string(&grpc_json(message)?)?;
    ensure!(
        text.len() <= MAX_MESSAGE,
        "Protobuf JSON response exceeds 1 MiB"
    );
    Ok(serde_json::from_str(&mask(&text)).unwrap_or(serde_json::json!("[REDACTED]")))
}
fn record_message(
    session: &Session,
    direction: &str,
    message: &DynamicMessage,
    mask: &dyn Fn(&str) -> String,
) -> Result<()> {
    let size = message.encoded_len();
    if direction == "incoming" {
        session.received(size)?;
    } else {
        session.record.lock().unwrap().summary.sent_bytes += size as u64;
    }
    session.event(
        direction,
        EventMessage::GrpcMessage {
            message: safe_json(message, mask)?,
        },
    )
}
fn record_status(session: &Session, status: &Status, mask: &dyn Fn(&str) -> String) -> Result<()> {
    ensure!(
        status.details().len() <= 32 * 1024 && status.message().len() <= 32 * 1024,
        "gRPC status exceeds limit"
    );
    session.event(
        "incoming",
        EventMessage::GrpcStatus {
            code: status.code() as u32,
            name: format!("{:?}", status.code()),
            message: mask(status.message()),
            details_base64: safe_binary(status.details(), mask),
            metadata: pairs(status.metadata(), mask)?,
        },
    )
}
struct AbortTask(tokio::task::JoinHandle<Result<()>>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
pub(crate) async fn run(
    session: Arc<Session>,
    request: RequestSpec,
    policy: NetworkPolicy,
    mut commands: mpsc::Receiver<Command>,
    mask: Arc<dyn Fn(&str) -> String + Send + Sync>,
) -> Result<String> {
    let method = session
        .grpc_method
        .lock()
        .unwrap()
        .clone()
        .context("Missing gRPC method descriptor")?;
    let Protocol::Grpc { message_source, .. } = &request.protocol else {
        anyhow::bail!("Expected gRPC request")
    };
    let initial = grpc_message(method.input(), message_source)?;
    let metadata = metadata(&request)?;
    let invocation = async {
        let channel = channel(&request, policy).await?;
        let (tx, rx) = mpsc::channel(1);
        record_message(&session, "outgoing", &initial, &*mask)?;
        tx.send(initial).await.context("gRPC input stream closed")?;
        // No HTTP handshake/status is synthesized for gRPC.
        session.record.lock().unwrap().summary.state = SessionState::Open;
        session.event(
            "system",
            EventMessage::State {
                state: SessionState::Open,
                reason: None,
            },
        )?;
        let sender_session = session.clone();
        let sender_mask = mask.clone();
        let client_streaming = method.is_client_streaming();
        let mut pump = AbortTask(tokio::spawn(async move {
            if !client_streaming {
                return Ok(());
            }
            while let Some(command) = commands.recv().await {
                match command {
                    Command::GrpcMessage(message) => {
                        record_message(&sender_session, "outgoing", &message, &*sender_mask)?;
                        tx.send(message).await.context("gRPC input stream closed")?;
                    }
                    Command::GrpcHalfClose => break,
                    Command::Mcp(_)
                    | Command::Websocket(_)
                    | Command::Mqtt(_)
                    | Command::Socketio(_) => {
                        anyhow::bail!("Unexpected protocol command")
                    }
                }
            }
            Ok(())
        }));
        let mut client = tonic::client::Grpc::new(channel)
            .max_encoding_message_size(MAX_MESSAGE)
            .max_decoding_message_size(MAX_MESSAGE);
        client.ready().await?;
        let path = format!("/{}/{}", method.parent_service().full_name(), method.name()).parse()?;
        let mut rpc = tonic::Request::new(ReceiverStream::new(rx));
        *rpc.metadata_mut() = metadata;
        rpc.set_timeout(Duration::from_millis(request.timeout_ms));
        let receive = async {
            let response = match client
                .streaming(rpc, path, DynamicCodec(method.output()))
                .await
            {
                Ok(response) => response,
                Err(status) => {
                    record_status(&session, &status, &*mask)?;
                    return Ok(format!(
                        "gRPC {:?}: {}",
                        status.code(),
                        mask(status.message())
                    ));
                }
            };
            session.event(
                "incoming",
                EventMessage::GrpcMetadata {
                    phase: "headers".into(),
                    metadata: pairs(response.metadata(), &*mask)?,
                },
            )?;
            let mut stream = response.into_inner();
            let mut count = 0usize;
            loop {
                match stream.message().await {
                    Ok(Some(message)) => {
                        count += 1;
                        ensure!(
                            count <= 10_000 && (method.is_server_streaming() || count <= 1),
                            "gRPC response message count exceeded"
                        );
                        record_message(&session, "incoming", &message, &*mask)?;
                    }
                    Ok(None) => break,
                    Err(status) => {
                        record_status(&session, &status, &*mask)?;
                        return Ok(format!(
                            "gRPC {:?}: {}",
                            status.code(),
                            mask(status.message())
                        ));
                    }
                }
            }
            let trailers = stream.trailers().await?;
            session.event(
                "incoming",
                EventMessage::GrpcMetadata {
                    phase: "trailers".into(),
                    metadata: pairs(&trailers.unwrap_or_default(), &*mask)?,
                },
            )?;
            ensure!(
                method.is_server_streaming() || count == 1,
                "Unary gRPC response missing message"
            );
            record_status(&session, &Status::ok(""), &*mask)?;
            Ok::<_, anyhow::Error>("gRPC OK".into())
        };
        tokio::pin!(receive);
        tokio::select! {
            result = &mut receive => result,
            result = &mut pump.0 => { result.context("gRPC sender task failed")??; receive.await },
        }
    };
    tokio::select! {
        biased;
        _ = session.cancel.cancelled() => { record_status(&session, &Status::cancelled("Cancelled by client"), &*mask)?; Ok("gRPC cancelled by client".into()) },
        _ = tokio::time::sleep(Duration::from_millis(request.timeout_ms)) => { record_status(&session, &Status::deadline_exceeded("Deadline exceeded"), &*mask)?; Ok("gRPC deadline exceeded".into()) },
        result = invocation => result,
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ReflectionStatus {
    pub code: u32,
    pub name: String,
    pub message: String,
    pub details_base64: String,
}
#[derive(Debug, serde::Serialize)]
pub struct ReflectionResult {
    pub status: Option<ReflectionStatus>,
    pub specification: Option<moleapi_core::Specification>,
    pub schema: Option<moleapi_core::GrpcSchema>,
    pub error: Option<String>,
}
// Both protocol revisions use their generated tonic clients and typed messages.
macro_rules! reflection_client {
    ($function:ident, $version:ident) => {
        async fn $function(
            channel: Channel,
            metadata: MetadataMap,
        ) -> Result<Vec<prost_types::FileDescriptorProto>> {
            use tonic_reflection::pb::$version::{
                ServerReflectionRequest, server_reflection_client::ServerReflectionClient,
                server_reflection_request::MessageRequest,
                server_reflection_response::MessageResponse,
            };
            let mut client = ServerReflectionClient::new(channel)
                .max_decoding_message_size(moleapi_core::MAX_PROTO_BYTES)
                .max_encoding_message_size(MAX_MESSAGE);
            let (tx, rx) = mpsc::channel(1);
            let mut request = tonic::Request::new(ReceiverStream::new(rx));
            *request.metadata_mut() = metadata;
            tx.send(ServerReflectionRequest {
                host: String::new(),
                message_request: Some(MessageRequest::ListServices(String::new())),
            })
            .await?;
            let mut responses = client.server_reflection_info(request).await?.into_inner();
            let first = responses
                .message()
                .await?
                .context("Reflection returned no service list")?;
            let services = match first.message_response {
                Some(MessageResponse::ListServicesResponse(response)) => response.service,
                Some(MessageResponse::ErrorResponse(error)) => {
                    return Err(Status::new(
                        tonic::Code::from_i32(error.error_code),
                        error.error_message,
                    )
                    .into())
                }
                _ => anyhow::bail!("Reflection returned unexpected service-list response"),
            };
            ensure!(services.len() <= 256, "Reflection service limit exceeded");
            let mut requests: VecDeque<_> = services
                .into_iter()
                .filter(|s| !s.name.starts_with("grpc.reflection."))
                .map(|s| MessageRequest::FileContainingSymbol(s.name))
                .collect();
            let mut files = HashMap::new();
            let mut requested_files = std::collections::HashSet::new();
            let mut bytes = 0usize;
            let mut count = 0usize;
            while let Some(message_request) = requests.pop_front() {
                count += 1;
                ensure!(count <= 512, "Reflection request limit exceeded");
                tx.send(ServerReflectionRequest {
                    host: String::new(),
                    message_request: Some(message_request),
                })
                .await?;
                let response = responses
                    .message()
                    .await?
                    .context("Reflection stream ended before descriptor response")?;
                let descriptors = match response.message_response {
                    Some(MessageResponse::FileDescriptorResponse(response)) => {
                        response.file_descriptor_proto
                    }
                    Some(MessageResponse::ErrorResponse(error)) => {
                        return Err(Status::new(
                            tonic::Code::from_i32(error.error_code),
                            error.error_message,
                        )
                        .into());
                    }
                    _ => anyhow::bail!("Reflection returned unexpected descriptor response"),
                };
                for descriptor in descriptors {
                    bytes += descriptor.len();
                    ensure!(
                        bytes <= moleapi_core::MAX_PROTO_BYTES,
                        "Reflection descriptors exceed 2 MiB"
                    );
                    let file = prost_types::FileDescriptorProto::decode(descriptor.as_slice())?;
                    let name = file
                        .name
                        .clone()
                        .context("Reflection descriptor requires name")?;
                    moleapi_core::validate_proto_path(&name)?;
                    if let Some(existing) = files.insert(name, file.clone()) {
                        ensure!(
                            existing == file,
                            "Reflection returned conflicting descriptors"
                        );
                    }
                    ensure!(files.len() <= 64, "Reflection file limit exceeded");
                }
                for dependency in files.values().flat_map(|f| &f.dependency) {
                    if !files.contains_key(dependency) && requested_files.insert(dependency.clone())
                    {
                        requests.push_back(MessageRequest::FileByFilename(dependency.clone()));
                    }
                }
            }
            drop(tx);
            Ok(files.into_values().collect())
        }
    };
}
reflection_client!(reflect_v1, v1);
reflection_client!(reflect_v1alpha, v1alpha);
pub async fn reflect(request: &RequestSpec, policy: NetworkPolicy) -> Result<ReflectionResult> {
    let metadata = metadata(request)?;
    let descriptors = tokio::time::timeout(Duration::from_millis(request.timeout_ms), async {
        let channel = channel(request, policy).await?;
        match reflect_v1(channel.clone(), metadata.clone()).await {
            Ok(files) => Ok(files),
            Err(error)
                if error
                    .downcast_ref::<Status>()
                    .is_some_and(|status| status.code() == tonic::Code::Unimplemented) =>
            {
                reflect_v1alpha(channel, metadata).await
            }
            Err(error) => Err(error),
        }
    })
    .await
    .map_err(|_| Status::deadline_exceeded("Reflection deadline exceeded"))??;
    let source = moleapi_core::ProtobufSource::Descriptor {
        descriptor_set_base64: STANDARD
            .encode(prost_types::FileDescriptorSet { file: descriptors }.encode_to_vec()),
    };
    let specification = moleapi_core::Specification {
        id: uuid::Uuid::new_v4().to_string(),
        name: "Reflected protobuf schema".into(),
        kind: "protobuf".into(),
        source: serde_json::to_string(&source)?,
        dialect: "descriptor-set".into(),
    };
    let pool = moleapi_core::protobuf_pool(&specification)?;
    Ok(ReflectionResult {
        status: None,
        schema: Some(moleapi_core::grpc_schema(&pool)?),
        specification: Some(specification),
        error: None,
    })
}
