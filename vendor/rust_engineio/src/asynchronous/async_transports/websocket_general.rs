use std::{borrow::Cow, str::from_utf8, sync::Arc, task::Poll};

use crate::{error::Result, Error, Packet, PacketId};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use bytes::Bytes;
use futures_util::{
    ready,
    FutureExt, Sink, SinkExt, Stream, StreamExt,
};
use tokio::sync::Mutex;
use std::pin::Pin;
use tungstenite::Message;

type AsyncWebsocketSender = Pin<Box<dyn Sink<Message, Error=tungstenite::Error> + Send>>;
type AsyncWebsocketReceiver = Pin<Box<dyn Stream<Item=std::result::Result<Message,tungstenite::Error>> + Send>>;

/// A general purpose asynchronous websocket transport type. Holds
/// the sender and receiver stream of a websocket connection
/// and implements the common methods `update` and `emit`. This also
/// implements `Stream`.
#[derive(Clone)]
pub(crate) struct AsyncWebsocketGeneralTransport {
    sender: Arc<Mutex<AsyncWebsocketSender>>,
    received: Arc<std::sync::atomic::AtomicUsize>,
    received_limit: Option<usize>,
    receiver: Arc<Mutex<AsyncWebsocketReceiver>>,
}

impl AsyncWebsocketGeneralTransport {
    pub(crate) async fn new<S,R>(sender: S, receiver: R) -> Self
    where S: Sink<Message,Error=tungstenite::Error> + Send + 'static,
          R: Stream<Item=std::result::Result<Message,tungstenite::Error>> + Send + 'static {
        AsyncWebsocketGeneralTransport {
            sender: Arc::new(Mutex::new(Box::pin(sender))),
            received: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            received_limit: None,
            receiver: Arc::new(Mutex::new(Box::pin(receiver))),
        }
    }

    pub(crate) fn with_received_limit(mut self, limit: usize) -> Self {
        self.received_limit = Some(limit);
        self
    }
    fn account(&self, message: &Message) -> Result<()> {
        if let Some(limit) = self.received_limit {
            let previous = self
                .received
                .fetch_add(message.len(), std::sync::atomic::Ordering::Relaxed);
            if previous.saturating_add(message.len()) > limit {
                return Err(Error::ReceiveLimit);
            }
        }
        Ok(())
    }

    /// Sends probe packet to ensure connection is valid, then sends upgrade
    /// request
    pub(crate) async fn upgrade(&self) -> Result<()> {
        let mut receiver = self.receiver.lock().await;
        let mut sender = self.sender.lock().await;

        sender
            .send(Message::text(Cow::Borrowed(from_utf8(&Bytes::from(
                Packet::new(PacketId::Ping, Bytes::from("probe")),
            ))?)))
            .await?;

        let msg = receiver
            .next()
            .await
            .ok_or(Error::IllegalWebsocketUpgrade())??;

        if msg.into_data() != Bytes::from(Packet::new(PacketId::Pong, Bytes::from("probe"))) {
            return Err(Error::InvalidPacket());
        }

        sender
            .send(Message::text(Cow::Borrowed(from_utf8(&Bytes::from(
                Packet::new(PacketId::Upgrade, Bytes::from("")),
            ))?)))
            .await?;

        Ok(())
    }

    pub(crate) async fn emit(&self, data: Bytes, is_binary_att: bool) -> Result<()> {
        let mut sender = self.sender.lock().await;

        let message = if is_binary_att {
            Message::binary(Cow::Borrowed(data.as_ref()))
        } else {
            Message::text(Cow::Borrowed(std::str::from_utf8(data.as_ref())?))
        };

        sender.send(message).await?;

        Ok(())
    }

    pub(crate) async fn poll_next(&self) -> Result<Option<Bytes>> {
        loop {
            let mut receiver = self.receiver.lock().await;
            let next = receiver.next().await;
            if let Some(Ok(message)) = &next {
                self.account(message)?;
            }
            match next {
                Some(Ok(Message::Text(str))) => return Ok(Some(Bytes::from(str))),
                Some(Ok(Message::Binary(data))) => {
                    // Use the SDK's existing binary packet representation. Raw bytes may
                    // contain polling delimiters, and an empty binary frame is still data.
                    return Ok(Some(Bytes::from(format!("b{}", STANDARD.encode(data)))));
                }
                // ignore packets other than text and binary
                Some(Ok(_)) => (),
                Some(Err(err)) => return Err(err.into()),
                None => return Ok(None),
            }
        }
    }
}

impl Stream for AsyncWebsocketGeneralTransport {
    type Item = Result<Bytes>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            let mut lock = ready!(Box::pin(self.receiver.lock()).poll_unpin(cx));
            let next = ready!(lock.poll_next_unpin(cx));

            if let Some(Ok(message)) = &next {
                if let Err(error) = self.account(message) {
                    return Poll::Ready(Some(Err(error)));
                }
            }
            match next {
                Some(Ok(Message::Text(str))) => return Poll::Ready(Some(Ok(Bytes::from(str)))),
                Some(Ok(Message::Binary(data))) => {
                    return Poll::Ready(Some(Ok(Bytes::from(format!(
                        "b{}",
                        STANDARD.encode(data)
                    )))));
                }
                // ignore packets other than text and binary
                Some(Ok(_)) => (),
                Some(Err(err)) => return Poll::Ready(Some(Err(err.into()))),
                None => return Poll::Ready(None),
            }
        }
    }
}
