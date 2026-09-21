use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use be_protocol::{ClientMessage, ServerMessage, decode, encode};
use futures_util::future::{Either, select};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::{
    ClientError,
    transport::{Frame, Reader, Writer, connect, spawn},
};

const EVENT_CAPACITY: usize = 256;

struct Outbound {
    message: ClientMessage,
    reply: oneshot::Sender<ServerMessage>,
}

pub struct Connection {
    commands: mpsc::UnboundedSender<Outbound>,
    events: broadcast::Sender<ServerMessage>,
    closed: watch::Receiver<bool>,
    next: AtomicU64,
}

impl Connection {
    pub async fn connect(url: &str) -> Result<Arc<Self>, ClientError> {
        let (writer, reader) = connect(url).await.map_err(ClientError::Disconnected)?;
        let (commands, receiver) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (closing, closed) = watch::channel(false);
        let connection = Arc::new(Self {
            commands,
            events: events.clone(),
            closed,
            next: AtomicU64::new(0),
        });
        spawn(worker(writer, reader, receiver, events, closing));
        Ok(connection)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.events.subscribe()
    }

    pub fn closed(&self) -> watch::Receiver<bool> {
        self.closed.clone()
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.borrow()
    }

    pub async fn request(
        &self,
        build: impl FnOnce(u64) -> ClientMessage,
    ) -> Result<ServerMessage, ClientError> {
        let request = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Outbound {
                message: build(request),
                reply,
            })
            .map_err(|_| ClientError::Disconnected("the connection worker stopped".into()))?;
        let response = response
            .await
            .map_err(|_| ClientError::Disconnected("the connection closed".into()))?;
        match response {
            ServerMessage::Failed { code, message, .. } => Err(ClientError::Refused(code, message)),
            response => Ok(response),
        }
    }
}

async fn worker(
    writer: Writer,
    reader: Reader,
    commands: mpsc::UnboundedReceiver<Outbound>,
    events: broadcast::Sender<ServerMessage>,
    closing: watch::Sender<bool>,
) {
    carry(writer, reader, commands, events).await;
    let _ = closing.send(true);
}

async fn carry(
    mut writer: Writer,
    mut reader: Reader,
    mut commands: mpsc::UnboundedReceiver<Outbound>,
    events: broadcast::Sender<ServerMessage>,
) {
    let mut pending: HashMap<u64, oneshot::Sender<ServerMessage>> = HashMap::new();
    loop {
        let command = std::pin::pin!(commands.recv());
        let frame = std::pin::pin!(reader.next());
        match select(command, frame).await {
            Either::Left((command, _)) => {
                let Some(command) = command else { return };
                let Ok(bytes) = encode(&command.message) else {
                    continue;
                };
                pending.insert(command.message.request(), command.reply);
                if writer.send(bytes).await.is_err() {
                    return;
                }
            }
            Either::Right((frame, _)) => match frame {
                Some(Frame::Binary(bytes)) => {
                    let Ok(message) = decode::<ServerMessage>(&bytes) else {
                        continue;
                    };
                    match message
                        .request()
                        .and_then(|request| pending.remove(&request))
                    {
                        Some(reply) => {
                            let _ = reply.send(message);
                        }
                        None => {
                            let _ = events.send(message);
                        }
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                Some(Frame::Ping(payload)) => {
                    if writer.pong(payload).await.is_err() {
                        return;
                    }
                }
                Some(Frame::Closed) | None => return,
            },
        }
    }
}
