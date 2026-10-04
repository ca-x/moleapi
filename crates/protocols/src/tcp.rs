//! Official Tokio codecs own framing; this module adapts ownership and IO budgets.
use crate::{Command, EventMessage, MAX_INPUT, PrivacyMask, SendMessage, Session, SessionState};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::{Bytes, BytesMut};
use futures_util::{SinkExt, StreamExt};
use moleapi_core::{
    Environment, NetworkPolicy, Protocol, RequestSpec, TcpConfig, checked_destination,
    resolve_tcp_message, tcp_payload, tcp_url,
};
use std::{
    io,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context as TaskContext, Poll},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    sync::mpsc,
};
use tokio_util::codec::{BytesCodec, Decoder, Encoder, Framed, LengthDelimitedCodec, LinesCodec};
#[derive(Default)]
pub(crate) struct Control {
    pub config: TcpConfig,
    pub environment: Option<Environment>,
    pub private: bool,
    sends: usize,
    received_frames: usize,
}
impl Control {
    pub fn new(config: TcpConfig, environment: Environment, private: bool) -> Self {
        Self {
            config,
            environment: Some(environment),
            private,
            sends: 0,
            received_frames: 0,
        }
    }
}
pub(crate) fn send(session: &Arc<Session>, message: SendMessage) -> Result<()> {
    let mut record = session.record.lock().unwrap();
    ensure!(
        record.summary.protocol == "tcp"
            && record.summary.state == SessionState::Open
            && !session.cancel.is_cancelled(),
        "Expected open TCP session"
    );
    ensure!(
        !record.summary.client_half_closed,
        "TCP send side is half-closed"
    );
    let mut control = session.tcp.lock().unwrap();
    let (command, size, half, private) = match message {
        SendMessage::TcpSend { message } => {
            ensure!(control.sends < 512, "TCP send-command limit reached (512)");
            let resolved = control
                .environment
                .as_ref()
                .map(|e| resolve_tcp_message(&message, e))
                .transpose()
                .map_err(|_| anyhow::anyhow!("Invalid or unresolved TCP payload"))?
                .unwrap_or(message);
            let bytes = tcp_payload(&resolved)?;
            ensure!(
                bytes.len() <= control.config.max_frame_bytes,
                "TCP payload exceeds configured frame limit"
            );
            if control.config.framing == "lines" {
                ensure!(
                    std::str::from_utf8(&bytes).is_ok(),
                    "Line framing requires UTF8 payload"
                );
            }
            let size = bytes.len();
            (Command::TcpData(bytes), size, false, resolved.secret)
        }
        SendMessage::TcpHalfClose => (Command::TcpHalfClose, 0, true, false),
        _ => anyhow::bail!("Expected TCP command"),
    };
    ensure!(
        record.input.saturating_add(size) <= MAX_INPUT,
        "TCP input limit reached (20 MiB)"
    );
    session
        .commands
        .try_send(command)
        .map_err(|_| anyhow::anyhow!("Session command queue is full or closed"))?;
    record.input += size;
    record.summary.client_half_closed |= half;
    control.sends += usize::from(!half);
    control.private |= private;
    Ok(())
}
trait Socket: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Socket for T {}
type Stream = Box<dyn Socket>;
struct MeasuredIo {
    inner: Stream,
    session: Arc<Session>,
    activity: Arc<Mutex<Instant>>,
}
impl AsyncRead for MeasuredIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buffer.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buffer) {
            Poll::Ready(Ok(())) => {
                let n = buffer.filled().len() - before;
                if n > 0 {
                    *self.activity.lock().unwrap() = Instant::now();
                    if self.session.received(n).is_err() {
                        return Poll::Ready(Err(io::Error::other(
                            "TCP received-wire limit reached (20 MiB)",
                        )));
                    }
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
impl AsyncWrite for MeasuredIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let remaining = MAX_INPUT
            .saturating_sub(self.session.record.lock().unwrap().summary.sent_bytes as usize);
        if remaining == 0 && !bytes.is_empty() {
            return Poll::Ready(Err(io::Error::other(
                "TCP sent-wire limit reached (20 MiB)",
            )));
        }
        match Pin::new(&mut self.inner).poll_write(cx, &bytes[..bytes.len().min(remaining)]) {
            Poll::Ready(Ok(n)) => {
                self.session.record.lock().unwrap().summary.sent_bytes += n as u64;
                if n > 0 {
                    *self.activity.lock().unwrap() = Instant::now();
                }
                Poll::Ready(Ok(n))
            }
            other => other,
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
fn record(session: &Session, direction: &str, bytes: &[u8]) -> Result<()> {
    let private = {
        let mut control = session.tcp.lock().unwrap();
        if direction == "incoming" {
            ensure!(
                control.received_frames < 4096,
                "TCP received-frame limit reached (4096)"
            );
            control.received_frames += 1;
        }
        control.private
    };
    session.event(
        direction,
        EventMessage::TcpData {
            base64: if private {
                String::new()
            } else {
                STANDARD.encode(bytes)
            },
            text: if private {
                None
            } else {
                std::str::from_utf8(bytes).ok().map(String::from)
            },
            bytes: bytes.len(),
            redacted: private,
        },
    )
}
pub(crate) async fn run(
    session: Arc<Session>,
    request: RequestSpec,
    policy: NetworkPolicy,
    commands: mpsc::Receiver<Command>,
    _mask: PrivacyMask,
) -> Result<String> {
    let Protocol::Tcp { config } = &request.protocol else {
        anyhow::bail!("Expected TCP configuration")
    };
    let url = tcp_url(&request.url)?;
    let activity = Arc::new(Mutex::new(Instant::now()));
    let connect = async {
        let addresses = checked_destination(&url, policy).await?;
        let mut connected = None;
        let mut last_error = None;
        for address in addresses {
            match TcpStream::connect(address).await {
                Ok(stream) => {
                    connected = Some(stream);
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }
        let stream = connected.ok_or_else(|| {
            anyhow::Error::new(
                last_error.unwrap_or_else(|| io::Error::other("No checked TCP addresses")),
            )
            .context("TCP connection failed")
        })?;
        stream.set_nodelay(config.no_delay)?;
        let stream = MeasuredIo {
            inner: Box::new(stream),
            session: session.clone(),
            activity: activity.clone(),
        };
        let stream: Stream = if url.scheme() == "tcps" {
            let mut roots = rustls::RootCertStore::empty();
            for certificate in rustls_native_certs::load_native_certs().certs {
                let _ = roots.add(certificate);
            }
            let mut tls = rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(roots)
            .with_no_client_auth();
            if !request.verify_tls {
                tls.dangerous()
                    .set_certificate_verifier(Arc::new(crate::grpc::UnverifiedCertificate));
            }
            let name = rustls::pki_types::ServerName::try_from(
                url.host_str().unwrap().trim_matches(['[', ']']).to_owned(),
            )?;
            Box::new(
                tokio_rustls::TlsConnector::from(Arc::new(tls))
                    .connect(name, stream)
                    .await?,
            )
        } else {
            Box::new(stream)
        };
        Ok::<_, anyhow::Error>(stream)
    };
    let stream = tokio::select! {biased;_=session.cancel.cancelled()=>return Ok("TCP closed while connecting".into()),value=tokio::time::timeout(Duration::from_millis(request.timeout_ms),connect)=>value.context("TCP connect timed out")??};
    *activity.lock().unwrap() = Instant::now();
    {
        session.record.lock().unwrap().summary.state = SessionState::Open;
    }
    session.event(
        "system",
        EventMessage::State {
            state: SessionState::Open,
            reason: None,
        },
    )?;
    match config.framing.as_str() {
        "raw" => {
            exchange(
                session,
                Framed::new(stream, BytesCodec::new()),
                commands,
                config,
                activity,
                |v| Ok(Bytes::from(v)),
                |v: &BytesMut| v.as_ref(),
            )
            .await
        }
        "lines" => {
            exchange(
                session,
                Framed::new(
                    stream,
                    LinesCodec::new_with_max_length(config.max_frame_bytes),
                ),
                commands,
                config,
                activity,
                |v| {
                    String::from_utf8(v)
                        .map_err(|_| anyhow::anyhow!("Line framing requires UTF8 payload"))
                },
                |v: &String| v.as_bytes(),
            )
            .await
        }
        _ => {
            let mut builder = LengthDelimitedCodec::builder();
            builder
                .length_field_length(4)
                .max_frame_length(config.max_frame_bytes);
            if config.framing == "length_le" {
                builder.little_endian();
            } else {
                builder.big_endian();
            }
            exchange(
                session,
                Framed::new(stream, builder.new_codec()),
                commands,
                config,
                activity,
                |v| Ok(Bytes::from(v)),
                |v: &BytesMut| v.as_ref(),
            )
            .await
        }
    }
}
async fn exchange<C, O>(
    session: Arc<Session>,
    mut framed: Framed<Stream, C>,
    mut commands: mpsc::Receiver<Command>,
    config: &TcpConfig,
    activity: Arc<Mutex<Instant>>,
    encode: fn(Vec<u8>) -> Result<O>,
    bytes: fn(&C::Item) -> &[u8],
) -> Result<String>
where
    C: Decoder + Encoder<O> + Unpin + Send,
    C::Item: Send,
    O: Send,
    <C as Decoder>::Error: std::error::Error + Send + Sync + 'static,
    <C as Encoder<O>>::Error: std::error::Error + Send + Sync + 'static,
{
    let mut clock =
        tokio::time::interval(Duration::from_millis((config.idle_timeout_ms / 2).max(10)));
    loop {
        tokio::select! {biased;
            _=session.cancel.cancelled()=>return Ok("TCP session closed by client".into()),
            _=clock.tick(),if config.idle_timeout_ms>0=>{if activity.lock().unwrap().elapsed()>=Duration::from_millis(config.idle_timeout_ms){anyhow::bail!("TCP idle timeout reached");}},
            command=commands.recv()=>{
                match command {
                    Some(Command::TcpData(payload))=>{
                        let recorded=payload.clone();let item=encode(payload)?;
                        tokio::select!{biased;_=session.cancel.cancelled()=>return Ok("TCP closed while sending".into()),value=tokio::time::timeout(Duration::from_secs(5),framed.send(item))=>{value.context("TCP send timed out")??;}}
                        record(&session,"outgoing",&recorded)?;
                    },
                    Some(Command::TcpHalfClose)=>{
                        tokio::select!{biased;_=session.cancel.cancelled()=>return Ok("TCP closed during half-close".into()),value=tokio::time::timeout(Duration::from_secs(5),framed.close())=>{value.context("TCP half-close timed out")??;}}
                        session.event("system",EventMessage::TcpHalfClosed)?;
                    },
                    None=>return Ok("TCP command channel closed".into()),
                    _=>anyhow::bail!("Unexpected TCP command"),
                }
            },
            incoming=framed.next()=>{
                let Some(incoming)=incoming else{return Ok("TCP peer ended stream".into())};
                let incoming=incoming?;let payload=bytes(&incoming);ensure!(payload.len()<=config.max_frame_bytes,"TCP received chunk exceeds configured frame limit");record(&session,"incoming",payload)?;
            },
        }
    }
}
