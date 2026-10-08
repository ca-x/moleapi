#![allow(dead_code, unused_imports)]
use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, Response, StatusCode, service::service_fn};
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use sspi::{
    AuthIdentity, AuthIdentityBuffers, BufferType, CredentialUse, DataRepresentation, Ntlm,
    SecurityBuffer, SecurityStatus, ServerRequestFlags, Sspi, SspiImpl, Username,
};
use std::{
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Session {
    ntlm: Ntlm,
    credentials: Option<AuthIdentityBuffers>,
    authenticated: bool,
    challenge: Option<String>,
}
pub struct Fixture {
    pub url: String,
    pub connections: Arc<AtomicUsize>,
    pub rounds: Arc<AtomicUsize>,
    pub task: tokio::task::JoinHandle<()>,
}
trait FixtureIo: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> FixtureIo for T {}
pub async fn serve_ntlm(close: bool) -> Fixture {
    serve_ntlm_tls(close, false).await
}
pub async fn serve_ntlm_tls(close: bool, tls: bool) -> Fixture {
    serve_ntlm_tls_binding(close, tls, false).await
}
pub async fn serve_ntlm_tls_binding(close: bool, tls: bool, mismatch: bool) -> Fixture {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "{}://{}",
        if tls { "https" } else { "http" },
        listener.local_addr().unwrap()
    );
    let connections = Arc::new(AtomicUsize::new(0));
    let rounds = Arc::new(AtomicUsize::new(0));
    let count = connections.clone();
    let requests = rounds.clone();
    let (tls_config, binding) = if tls {
        use sha2::Digest;
        let certified =
            rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])
                .unwrap();
        let der = certified.cert.der().clone();
        let private =
            rustls::pki_types::PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der());
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![der.clone()], private.into())
        .unwrap();
        let mut binding = b"tls-server-end-point:".to_vec();
        binding.extend(sha2::Sha256::digest(der.as_ref()));
        if mismatch {
            binding[20] ^= 1;
        }
        (Some(Arc::new(config)), Some(binding))
    } else {
        (None, None)
    };
    let task = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let conn = count.fetch_add(1, Ordering::SeqCst) + 1;
            let stream: Box<dyn FixtureIo> = if let Some(config) = &tls_config {
                match tokio_rustls::TlsAcceptor::from(config.clone())
                    .accept(stream)
                    .await
                {
                    Ok(stream) => Box::new(stream),
                    Err(_) => continue,
                }
            } else {
                Box::new(stream)
            };
            let mut ntlm = Ntlm::new();
            if let Some(binding) = &binding {
                ntlm.set_channel_bindings(binding);
            }
            let identity = AuthIdentity {
                username: Username::parse("DOMAIN\\User").unwrap(),
                password: "Password".to_string().into(),
            };
            let creds = ntlm
                .acquire_credentials_handle()
                .with_credential_use(CredentialUse::Inbound)
                .with_auth_data(&identity)
                .execute(&mut ntlm)
                .unwrap()
                .credentials_handle;
            let state = Arc::new(Mutex::new(Session {
                ntlm,
                credentials: creds,
                authenticated: false,
                challenge: None,
            }));
            let requests = requests.clone();
            tokio::spawn(async move {
                let service = service_fn(move |request: Request<hyper::body::Incoming>| {
                    let state = state.clone();
                    let requests = requests.clone();
                    async move {
                        requests.fetch_add(1, Ordering::SeqCst);
                        let method = request.method().to_string();
                        let path = request.uri().to_string();
                        let auth = request
                            .headers()
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned);
                        let body = request.into_body().collect().await.unwrap().to_bytes();
                        let mut response = Response::builder();
                        let mut state = state.lock().unwrap();
                        let mut accepted = state.authenticated;
                        if let Some(auth) = &auth {
                            let bytes = STANDARD
                                .decode(auth.strip_prefix("NTLM ").unwrap())
                                .unwrap();
                            let mut input = [SecurityBuffer::new(bytes, BufferType::Token)];
                            let mut output = [SecurityBuffer::new(Vec::new(), BufferType::Token)];
                            let Session {
                                ntlm,
                                credentials,
                                challenge,
                                ..
                            } = &mut *state;
                            let builder = ntlm
                                .accept_security_context()
                                .with_credentials_handle(credentials)
                                .with_context_requirements(ServerRequestFlags::empty())
                                .with_target_data_representation(DataRepresentation::Native)
                                .with_input(&mut input)
                                .with_output(&mut output);
                            let result = ntlm
                                .accept_security_context_impl(builder)
                                .and_then(|mut g| g.resolve_to_result());
                            match result {
                                Ok(result) if result.status == SecurityStatus::ContinueNeeded => {
                                    *challenge = Some(format!(
                                        "NTLM {}",
                                        STANDARD.encode(&output[0].buffer)
                                    ));
                                    response = response.header(
                                        "www-authenticate",
                                        format!(
                                            "Basic realm=\"quoted, realm\", NTLM {}",
                                            STANDARD.encode(&output[0].buffer)
                                        ),
                                    );
                                }
                                Ok(result) => {
                                    accepted = if matches!(
                                        result.status,
                                        SecurityStatus::CompleteNeeded
                                            | SecurityStatus::CompleteAndContinue
                                    ) {
                                        ntlm.complete_auth_token(&mut output).is_ok()
                                    } else {
                                        result.status == SecurityStatus::Ok
                                    };
                                }
                                Err(_) => accepted = false,
                            }
                        } else {
                            response = response
                                .header("www-authenticate", "Basic realm=\"quoted, realm\", NTLM");
                        }
                        state.authenticated = accepted;
                        if accepted
                            && let Ok(file) = std::env::var("MOLEAPI_NTLM_CAPTURE")
                            && let Some(challenge) = &state.challenge
                        {
                            std::fs::write(file,serde_json::to_vec_pretty(&json!({"username":"User","domain":"DOMAIN","password":"Password","challenge":challenge,"authenticate":auth})).unwrap()).unwrap();
                        }

                        if close && !accepted {
                            response = response.header("connection", "close");
                        }
                        let payload = if accepted {
                            json!({"connection":conn,"method":method,"path":path,"body_base64":STANDARD.encode(body),"auth":auth})
                        } else {
                            json!({"unauthorized":true})
                        };
                        if !accepted
                            && auth.is_some()
                            && response
                                .headers_ref()
                                .is_some_and(|h| !h.contains_key("www-authenticate"))
                        {
                            response = response.header("www-authenticate", "NTLM");
                        }
                        if accepted && path.split('?').next() == Some("/redirect") {
                            return Ok::<_, Infallible>(
                                response
                                    .status(StatusCode::SEE_OTHER)
                                    .header("location", "/resource")
                                    .body(Full::new(Bytes::from("redirect")))
                                    .unwrap(),
                            );
                        }
                        Ok::<_, Infallible>(
                            response
                                .status(if accepted {
                                    StatusCode::OK
                                } else {
                                    StatusCode::UNAUTHORIZED
                                })
                                .body(Full::new(Bytes::from(payload.to_string())))
                                .unwrap(),
                        )
                    }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });
    Fixture {
        url,
        connections,
        rounds,
        task,
    }
}
