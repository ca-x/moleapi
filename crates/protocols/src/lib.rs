//! Volatile owner-scoped protocol sessions. Event cursors order delivery independently of clocks.
mod grpc;
pub use grpc::{ReflectionResult, ReflectionStatus, reflect};
mod engine;
mod graphql;
mod models;
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
pub use models::*;
use moleapi_core::{NetworkPolicy, Protocol, RequestSpec};
use reqwest_websocket::Message;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

type PrivacyMask = Arc<dyn Fn(&str) -> String + Send + Sync>;
pub const MAX_MESSAGE: usize = 1024 * 1024;
pub const MAX_WIRE: usize = 8 * 1024 * 1024;
const MAX_INPUT: usize = 20 * 1024 * 1024;
const MAX_EVENTS: usize = 256;
const MAX_EVENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_LIFETIME: Duration = Duration::from_secs(30 * 60);
const RETENTION: Duration = Duration::from_secs(10 * 60);
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub(crate) enum Command {
    Websocket(Message),
    GrpcMessage(prost_reflect::DynamicMessage),
    GrpcHalfClose,
}
struct Record {
    summary: SessionSummary,
    events: VecDeque<(SessionEvent, usize)>,
    bytes: usize,
    last_change: Instant,
    input: usize,
    grpc_input_messages: usize,
}
pub(crate) struct Session {
    owner: String,
    record: Mutex<Record>,
    cancel: CancellationToken,
    done: watch::Sender<bool>,
    commands: mpsc::Sender<Command>,
    receiver: Mutex<Option<mpsc::Receiver<Command>>>,
    grpc_method: Mutex<Option<prost_reflect::MethodDescriptor>>,
    grpc_environment: Mutex<Option<moleapi_core::Environment>>,
    grpc_mask: Mutex<Option<PrivacyMask>>,
}
impl Session {
    pub(crate) fn event(&self, direction: &str, message: EventMessage) -> Result<()> {
        let payload_size = match &message {
            EventMessage::GrpcMessage { .. }
            | EventMessage::GrpcMetadata { .. }
            | EventMessage::GrpcStatus { .. } => serde_json::to_vec(&message)?.len(),
            EventMessage::Sse {
                event, data, id, ..
            } => event.len() + data.len() + id.len(),
            EventMessage::GraphqlNext {
                operation_id,
                payload,
            }
            | EventMessage::GraphqlError {
                operation_id,
                payload,
            }
            | EventMessage::GraphqlComplete {
                operation_id,
                payload,
            } => operation_id.len() + serde_json::to_vec(payload)?.len(),
            EventMessage::Text { text } => text.len(),
            EventMessage::Binary { base64 }
            | EventMessage::Ping { base64 }
            | EventMessage::Pong { base64 } => {
                base64.len() / 4 * 3 - base64.bytes().rev().take_while(|b| *b == b'=').count()
            }
            EventMessage::Close { reason, .. } => reason.len(),
            EventMessage::State { reason, .. } => reason.as_ref().map_or(0, String::len),
            EventMessage::ScriptLog { level, message } => level.len() + message.len(),
            EventMessage::ScriptTest { test } => {
                test.name.len() + test.actual.len() + test.expected.len()
            }
        };
        ensure!(
            payload_size <= MAX_MESSAGE,
            "Session event exceeds 1 MiB payload limit"
        );
        let mut r = self.record.lock().unwrap();
        r.summary.event_count += 1;
        let cursor = r.summary.event_count;
        r.summary.updated_at = now();
        let event = SessionEvent {
            cursor,
            received_at: now(),
            direction: direction.into(),
            message,
        };
        let size = serde_json::to_vec(&event)?.len();
        r.events.push_back((event, size));
        r.bytes += size;
        while r.events.len() > MAX_EVENTS || r.bytes > MAX_EVENT_BYTES {
            if let Some((_, size)) = r.events.pop_front() {
                r.bytes -= size;
            }
        }
        Ok(())
    }
    pub(crate) fn open(&self, handshake: Handshake) -> Result<()> {
        {
            let mut r = self.record.lock().unwrap();
            r.summary.handshake = Some(handshake);
            r.summary.state = SessionState::Open;
        }
        self.event(
            "system",
            EventMessage::State {
                state: SessionState::Open,
                reason: None,
            },
        )
    }
    fn finish(&self, state: SessionState, reason: String) {
        {
            let mut r = self.record.lock().unwrap();
            r.summary.state = state;
            r.summary.reason = Some(reason.clone());
            r.last_change = Instant::now();
        }
        let _ = self.event(
            "system",
            EventMessage::State {
                state,
                reason: Some(reason),
            },
        );
        self.done.send_replace(true);
    }
    pub(crate) fn received(&self, bytes: usize) -> Result<()> {
        let mut r = self.record.lock().unwrap();
        r.summary.received_bytes += bytes as u64;
        ensure!(
            r.summary.received_bytes <= MAX_INPUT as u64,
            "Session received-byte limit reached (20 MiB)"
        );
        Ok(())
    }
}
pub struct SessionManager {
    records: Mutex<HashMap<String, Arc<Session>>>,
}
/// Temporary owner/workspace-bound admission and cancellation for Reflection.
/// It shares live session quotas, but leaves no retained record after the response.
pub struct ReflectionLease {
    manager: Arc<SessionManager>,
    session: Arc<Session>,
    id: String,
}
impl ReflectionLease {
    pub async fn cancelled(&self) {
        self.session.cancel.cancelled().await;
    }
}
impl Drop for ReflectionLease {
    fn drop(&mut self) {
        self.session.cancel.cancel();
        self.session
            .finish(SessionState::Closed, "Reflection finished".into());
        self.manager.records.lock().unwrap().remove(&self.id);
    }
}
impl Default for SessionManager {
    fn default() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }
}
impl Drop for SessionManager {
    fn drop(&mut self) {
        for session in self.records.get_mut().unwrap().values() {
            session.cancel.cancel();
        }
    }
}
impl SessionManager {
    pub fn new() -> Arc<Self> {
        let manager = Arc::new(Self::default());
        let weak = Arc::downgrade(&manager);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let Some(manager) = weak.upgrade() else { break };
                manager.prune();
            }
        });
        manager
    }
    fn prune(&self) {
        self.records.lock().unwrap().retain(|_, s| {
            let r = s.record.lock().unwrap();
            let expired = r.summary.state.live() && r.last_change.elapsed() >= MAX_LIFETIME;
            let retain = r.summary.state.live() || r.last_change.elapsed() < RETENTION;
            drop(r);
            if expired && s.receiver.lock().unwrap().take().is_some() {
                s.cancel.cancel();
                s.finish(
                    SessionState::Closed,
                    "Session lifetime expired before connecting (30 minutes)".into(),
                );
            }
            retain
        });
    }
    /// Register before the server's final ownership recheck. No network starts here.
    pub fn register(
        &self,
        owner: &str,
        workspace: &str,
        request: &RequestSpec,
        safe_url: String,
        feedback: PreparedFeedback,
    ) -> Result<SessionSummary> {
        ensure!(
            request.protocol != Protocol::Http,
            "HTTP requests use the execute API"
        );
        self.prune();
        let mut records = self.records.lock().unwrap();
        let live = |s: &&Arc<Session>| s.record.lock().unwrap().summary.state.live();
        ensure!(
            records.values().filter(live).count() < 64,
            "Global live-session capacity reached (64)"
        );
        let owned: Vec<_> = records.values().filter(|s| s.owner == owner).collect();
        ensure!(
            owned.len() < 16,
            "Owner retained-session capacity reached (16); delete old sessions"
        );
        ensure!(
            owned
                .iter()
                .filter(|s| s.record.lock().unwrap().summary.state.live())
                .count()
                < 4,
            "Owner live-session capacity reached (4)"
        );
        ensure!(
            records.len() < 1024,
            "Global retained-session capacity reached (1024)"
        );
        let summary = SessionSummary {
            id: uuid::Uuid::new_v4().to_string(),
            workspace_id: workspace.into(),
            request_id: request.id.clone(),
            url: safe_url,
            protocol: if request.protocol.is_grpc() {
                "grpc"
            } else if request.protocol.is_graphql() {
                "graphql"
            } else if request.protocol == Protocol::Sse {
                "sse"
            } else {
                "websocket"
            }
            .into(),
            state: SessionState::Connecting,
            reason: None,
            created_at: now(),
            updated_at: now(),
            received_bytes: 0,
            sent_bytes: 0,
            event_count: 0,
            handshake: None,
            client_half_closed: false,
            variable_updates: vec![],
            request_updates: vec![],
        };
        let (commands, rx) = mpsc::channel(32);
        let (done, _) = watch::channel(false);
        let session = Arc::new(Session {
            owner: owner.into(),
            record: Mutex::new(Record {
                summary: summary.clone(),
                events: VecDeque::new(),
                bytes: 0,
                last_change: Instant::now(),
                input: 0,
                grpc_input_messages: usize::from(request.protocol.is_grpc()),
            }),
            cancel: CancellationToken::new(),
            done,
            commands,
            receiver: Mutex::new(Some(rx)),
            grpc_method: Mutex::new(None),
            grpc_environment: Mutex::new(None),
            grpc_mask: Mutex::new(None),
        });
        session.event(
            "system",
            EventMessage::State {
                state: SessionState::Connecting,
                reason: None,
            },
        )?;
        for log in feedback.logs {
            session.event(
                "system",
                EventMessage::ScriptLog {
                    level: log.level,
                    message: log.message,
                },
            )?;
        }
        for test in feedback.tests {
            session.event("system", EventMessage::ScriptTest { test })?;
        }
        let summary = session.record.lock().unwrap().summary.clone();
        records.insert(summary.id.clone(), session);
        Ok(summary)
    }
    pub fn reserve_reflection(
        self: &Arc<Self>,
        owner: &str,
        workspace: &str,
        request: &RequestSpec,
        safe_url: String,
    ) -> Result<ReflectionLease> {
        let summary = self.register(
            owner,
            workspace,
            request,
            safe_url,
            PreparedFeedback::default(),
        )?;
        let session = self.owned(owner, &summary.id)?;
        Ok(ReflectionLease {
            manager: self.clone(),
            session,
            id: summary.id,
        })
    }
    fn owned(&self, owner: &str, id: &str) -> Result<Arc<Session>> {
        self.prune();
        self.records
            .lock()
            .unwrap()
            .get(id)
            .filter(|s| s.owner == owner)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Session not found"))
    }
    pub fn summary(&self, owner: &str, id: &str) -> Result<SessionSummary> {
        Ok(self
            .owned(owner, id)?
            .record
            .lock()
            .unwrap()
            .summary
            .clone())
    }
    pub fn events(&self, owner: &str, id: &str, after: u64) -> Result<EventBatch> {
        let s = self.owned(owner, id)?;
        let r = s.record.lock().unwrap();
        ensure!(
            after <= r.summary.event_count,
            "Event cursor is ahead of this session"
        );
        let earliest = r
            .events
            .front()
            .map(|(e, _)| e.cursor)
            .unwrap_or(r.summary.event_count + 1);
        Ok(EventBatch {
            events: r
                .events
                .iter()
                .filter(|(e, _)| e.cursor > after)
                .map(|(e, _)| e.clone())
                .collect(),
            next_cursor: r.summary.event_count,
            earliest_cursor: earliest,
            dropped_count: earliest.saturating_sub(after.saturating_add(1)),
        })
    }
    pub fn configure_grpc(
        &self,
        owner: &str,
        id: &str,
        method: prost_reflect::MethodDescriptor,
        environment: moleapi_core::Environment,
        mask: Arc<dyn Fn(&str) -> String + Send + Sync>,
    ) -> Result<()> {
        let s = self.owned(owner, id)?;
        let mut r = s.record.lock().unwrap();
        ensure!(
            r.summary.protocol == "grpc" && r.summary.state == SessionState::Connecting,
            "Expected connecting gRPC session"
        );
        r.summary.client_half_closed = !method.is_client_streaming();
        *s.grpc_method.lock().unwrap() = Some(method);
        *s.grpc_environment.lock().unwrap() = Some(environment);
        *s.grpc_mask.lock().unwrap() = Some(mask);
        Ok(())
    }
    pub fn start(
        &self,
        owner: &str,
        id: &str,
        request: RequestSpec,
        policy: NetworkPolicy,
        mask: Arc<dyn Fn(&str) -> String + Send + Sync>,
    ) -> Result<()> {
        let s = self.owned(owner, id)?;
        let receiver = s
            .receiver
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| anyhow::anyhow!("Session already started or closed"))?;
        let lifetime = MAX_LIFETIME.saturating_sub(s.record.lock().unwrap().last_change.elapsed());
        let s_worker = s.clone();
        tokio::spawn(async move {
            let task_session = s_worker.clone();
            let task_mask = mask.clone();
            let task = tokio::spawn(async move {
                tokio::select! {
                    biased;
                    _ = tokio::time::sleep(lifetime) => Ok("Session lifetime expired (30 minutes)".into()),
                    result = engine::run(&task_session, request, policy, receiver, task_mask) => result,
                }
            });
            match task.await {
                Ok(Ok(reason)) => s_worker.finish(SessionState::Closed, reason),
                Ok(Err(error)) => s_worker.finish(SessionState::Error, mask(&format!("{error:#}"))),
                Err(_) => s_worker.finish(
                    SessionState::Error,
                    "Protocol worker terminated unexpectedly".into(),
                ),
            }
        });
        Ok(())
    }
    pub fn send(&self, owner: &str, id: &str, message: SendMessage) -> Result<()> {
        let s = self.owned(owner, id)?;
        if matches!(
            message,
            SendMessage::GrpcMessage { .. } | SendMessage::GrpcHalfClose
        ) {
            let mut r = s.record.lock().unwrap();
            ensure!(
                r.summary.protocol == "grpc" && r.summary.state.live(),
                "Expected live gRPC session"
            );
            ensure!(
                !r.summary.client_half_closed,
                "gRPC send side is half-closed"
            );
            let method = s
                .grpc_method
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| anyhow::anyhow!("gRPC method is not configured"))?;
            ensure!(
                method.is_client_streaming(),
                "gRPC method does not accept client streaming"
            );
            if matches!(&message, SendMessage::GrpcMessage { .. }) {
                ensure!(
                    r.grpc_input_messages < 10_000,
                    "gRPC sent-message limit reached (10000)"
                );
            }
            let (command, size, close) = match message {
                SendMessage::GrpcMessage { message_source } => {
                    let environment = s.grpc_environment.lock().unwrap();
                    let mask = s.grpc_mask.lock().unwrap();
                    let prepared = environment
                        .as_ref()
                        .map(|environment| {
                            moleapi_core::resolve_grpc_source(&message_source, environment)
                        })
                        .transpose()
                        .map_err(|_| anyhow::anyhow!("Invalid or unresolved gRPC JSON message"))?
                        .unwrap_or(message_source);
                    let dynamic =
                        moleapi_core::grpc_message(method.input(), &prepared).map_err(|error| {
                            anyhow::anyhow!(mask.as_ref().map_or_else(
                                || "Invalid gRPC message".into(),
                                |mask| mask(&error.to_string())
                            ))
                        })?;
                    (Command::GrpcMessage(dynamic), prepared.len(), false)
                }
                _ => (Command::GrpcHalfClose, 0, true),
            };
            ensure!(
                r.input + size <= MAX_INPUT,
                "Session input limit reached (20 MiB)"
            );
            s.commands
                .try_send(command)
                .map_err(|_| anyhow::anyhow!("Session command queue is full or closed"))?;
            r.input += size;
            r.grpc_input_messages += usize::from(!close);
            r.summary.client_half_closed |= close;
            return Ok(());
        }
        let ping = matches!(&message, SendMessage::Ping { .. });
        let (message, size) = match message {
            SendMessage::GrpcMessage { .. } | SendMessage::GrpcHalfClose => unreachable!(),
            SendMessage::Text { text } => {
                ensure!(text.len() <= MAX_MESSAGE, "Message exceeds 1 MiB");
                let n = text.len();
                (Message::Text(text), n)
            }
            SendMessage::Binary { base64 } | SendMessage::Ping { base64 } => {
                let limit = if ping { 125usize } else { MAX_MESSAGE };
                ensure!(
                    base64.len() <= limit.div_ceil(3) * 4,
                    "Encoded message exceeds 1 MiB decoded limit"
                );
                let bytes = STANDARD.decode(&base64)?;
                ensure!(bytes.len() <= MAX_MESSAGE, "Message exceeds 1 MiB");
                let n = bytes.len();
                if ping {
                    ensure!(n <= 125, "Ping payload exceeds 125 bytes");
                }
                (
                    if ping {
                        Message::Ping(bytes.into())
                    } else {
                        Message::Binary(bytes.into())
                    },
                    n,
                )
            }
        };
        let mut r = s.record.lock().unwrap();
        ensure!(
            r.summary.protocol == "websocket",
            "SSE sessions are receive-only"
        );
        ensure!(r.summary.state == SessionState::Open, "Session is not open");
        ensure!(
            r.input + size <= MAX_INPUT,
            "Session input limit reached (20 MiB)"
        );
        s.commands
            .try_send(Command::Websocket(message))
            .map_err(|_| anyhow::anyhow!("Session command queue is full or closed"))?;
        r.input += size;
        Ok(())
    }
    pub async fn close(&self, owner: &str, id: &str) -> Result<SessionSummary> {
        let s = self.owned(owner, id)?;
        let mut done = s.done.subscribe();
        s.cancel.cancel();
        if s.receiver.lock().unwrap().take().is_some() {
            s.finish(
                SessionState::Closed,
                "Session closed before connecting".into(),
            );
        }
        if !*done.borrow() {
            let _ =
                tokio::time::timeout(Duration::from_secs(5), done.wait_for(|done| *done)).await?;
        }
        self.summary(owner, id)
    }
    pub async fn remove(&self, owner: &str, id: &str) -> Result<()> {
        self.close(owner, id).await?;
        self.records.lock().unwrap().remove(id);
        Ok(())
    }
    pub async fn close_owner(&self, owner: &str) {
        let ids: Vec<_> = self
            .records
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, s)| s.owner == owner)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            let _ = self.close(owner, &id).await;
        }
    }
    pub async fn reconcile_workspace(&self, owner: &str, workspace: &str, request_ids: &[String]) {
        let ids: Vec<_> = self
            .records
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, s)| {
                let r = s.record.lock().unwrap();
                s.owner == owner
                    && r.summary.workspace_id == workspace
                    && !request_ids.contains(&r.summary.request_id)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            let _ = self.remove(owner, &id).await;
        }
    }
    pub async fn close_workspace(&self, owner: &str, workspace: &str) {
        let sessions: Vec<_> = self
            .records
            .lock()
            .unwrap()
            .values()
            .filter(|s| {
                s.owner == owner && s.record.lock().unwrap().summary.workspace_id == workspace
            })
            .cloned()
            .collect();
        for s in &sessions {
            s.cancel.cancel();
            if s.receiver.lock().unwrap().take().is_some() {
                s.finish(
                    SessionState::Closed,
                    "Workspace closed before connecting".into(),
                );
            }
        }
        for s in sessions {
            let mut done = s.done.subscribe();
            if !*done.borrow() {
                let _ = tokio::time::timeout(Duration::from_secs(5), done.wait_for(|x| *x)).await;
            }
            let id = s.record.lock().unwrap().summary.id.clone();
            self.records.lock().unwrap().remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> RequestSpec {
        serde_json::from_value(serde_json::json!({"protocol":{"kind":"sse"},"id":"r","name":"r","method":"GET","url":"http://127.0.0.1/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
    }
    #[tokio::test]
    async fn retention_gaps_monotonic_order_bytes_admission_and_registered_cancellation() {
        let m = SessionManager::new();
        let request = request();
        let s = m
            .register(
                "owner",
                "w",
                &request,
                "safe".into(),
                PreparedFeedback::default(),
            )
            .unwrap();
        let record = m.owned("owner", &s.id).unwrap();
        for n in 0..300 {
            record
                .event(
                    "incoming",
                    EventMessage::Text {
                        text: n.to_string(),
                    },
                )
                .unwrap();
        }
        let batch = m.events("owner", &s.id, 0).unwrap();
        assert_eq!(batch.events.len(), 256);
        assert_eq!(batch.dropped_count, 45);
        assert_eq!(batch.earliest_cursor, 46);
        assert_eq!(batch.next_cursor, 301);
        assert!(
            batch
                .events
                .windows(2)
                .all(|e| e[0].cursor + 1 == e[1].cursor)
        );
        assert!(m.events("owner", &s.id, 302).is_err());
        for _ in 0..12 {
            record
                .event(
                    "incoming",
                    EventMessage::Text {
                        text: "a".repeat(MAX_MESSAGE),
                    },
                )
                .unwrap();
        }
        assert!(record.record.lock().unwrap().bytes <= MAX_EVENT_BYTES);
        for _ in 0..3 {
            m.register(
                "owner",
                "w",
                &request,
                "safe".into(),
                PreparedFeedback::default(),
            )
            .unwrap();
        }
        assert!(
            m.register(
                "owner",
                "w",
                &request,
                "safe".into(),
                PreparedFeedback::default()
            )
            .is_err()
        );
        m.close_workspace("owner", "w").await;
        assert_eq!(
            record.record.lock().unwrap().summary.state,
            SessionState::Closed
        );
        assert!(m.summary("owner", &s.id).is_err());
        assert!(
            m.start(
                "owner",
                &s.id,
                request.clone(),
                NetworkPolicy {
                    allow_private_network: true
                },
                Arc::new(str::to_owned)
            )
            .is_err()
        );
        let second = m
            .register(
                "owner",
                "other-workspace",
                &request,
                "safe".into(),
                PreparedFeedback::default(),
            )
            .unwrap();
        m.remove("owner", &second.id).await.unwrap();
        assert!(m.summary("owner", &second.id).is_err());
    }
}

#[cfg(test)]
mod bound_tests {
    use super::*;
    #[tokio::test]
    async fn bounded_commands_input_terminal_retention_and_shutdown() {
        let manager = SessionManager::new();
        let request:RequestSpec=serde_json::from_value(serde_json::json!({"protocol":{"kind":"websocket"},"id":"r","name":"r","method":"GET","url":"ws://127.0.0.1/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap();
        let s = manager
            .register(
                "owner",
                "w",
                &request,
                "safe".into(),
                PreparedFeedback::default(),
            )
            .unwrap();
        let record = manager.owned("owner", &s.id).unwrap();
        record.record.lock().unwrap().summary.state = SessionState::Open;
        for _ in 0..32 {
            manager
                .send(
                    "owner",
                    &s.id,
                    SendMessage::Text {
                        text: "bounded".into(),
                    },
                )
                .unwrap();
        }
        assert!(
            manager
                .send(
                    "owner",
                    &s.id,
                    SendMessage::Text {
                        text: "full".into()
                    }
                )
                .unwrap_err()
                .to_string()
                .contains("queue")
        );
        manager.close("owner", &s.id).await.unwrap();
        record.record.lock().unwrap().last_change =
            Instant::now() - RETENTION - Duration::from_secs(1);
        assert!(manager.summary("owner", &s.id).is_err());
        let s = manager
            .register(
                "owner",
                "w",
                &request,
                "safe".into(),
                PreparedFeedback::default(),
            )
            .unwrap();
        let record = manager.owned("owner", &s.id).unwrap();
        record.record.lock().unwrap().summary.state = SessionState::Open;
        for _ in 0..20 {
            manager
                .send(
                    "owner",
                    &s.id,
                    SendMessage::Text {
                        text: "x".repeat(MAX_MESSAGE),
                    },
                )
                .unwrap();
        }
        assert!(
            manager
                .send(
                    "owner",
                    &s.id,
                    SendMessage::Text {
                        text: "over".into()
                    }
                )
                .unwrap_err()
                .to_string()
                .contains("input limit")
        );
        drop(manager);
        assert!(record.cancel.is_cancelled());
    }
}
