use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use be_protocol::{ClientMessage, MAX_FRAME_BYTES, ServerMessage, decode, encode};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};

use crate::ClientError;

const EVENT_CAPACITY: usize = 256;

struct Outbound {
    message: ClientMessage,
    reply: oneshot::Sender<ServerMessage>,
}

pub struct Connection {
    commands: mpsc::UnboundedSender<Outbound>,
    events: broadcast::Sender<ServerMessage>,
    next: AtomicU64,
}

impl Connection {
    pub async fn connect(url: &str) -> Result<Arc<Self>, ClientError> {
        let (socket, _) = connect_async_with_config(
            url,
            Some(WebSocketConfig {
                max_frame_size: Some(MAX_FRAME_BYTES),
                max_message_size: Some(MAX_FRAME_BYTES),
                ..WebSocketConfig::default()
            }),
            false,
        )
        .await
        .map_err(|error| ClientError::Disconnected(error.to_string()))?;
        let (commands, receiver) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let connection = Arc::new(Self {
            commands,
            events: events.clone(),
            next: AtomicU64::new(0),
        });
        tokio::spawn(worker(socket, receiver, events));
        Ok(connection)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.events.subscribe()
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
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    mut commands: mpsc::UnboundedReceiver<Outbound>,
    events: broadcast::Sender<ServerMessage>,
) {
    let (mut sink, mut source) = socket.split();
    let mut pending: HashMap<u64, oneshot::Sender<ServerMessage>> = HashMap::new();
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { break };
                let Ok(bytes) = encode(&command.message) else { continue };
                pending.insert(command.message.request(), command.reply);
                if sink.send(Message::Binary(bytes)).await.is_err() {
                    break;
                }
            }
            frame = source.next() => {
                let Some(Ok(frame)) = frame else { break };
                match frame {
                    Message::Binary(bytes) => {
                        let Ok(message) = decode::<ServerMessage>(&bytes) else { continue };
                        match message.request().and_then(|request| pending.remove(&request)) {
                            Some(reply) => {
                                let _ = reply.send(message);
                            }
                            None => {
                                let _ = events.send(message);
                            }
                        }
                    }
                    Message::Close(_) => break,
                    Message::Ping(payload) => {
                        if sink.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Message::Text(_) | Message::Pong(_) | Message::Frame(_) => {}
                }
            }
        }
    }
}
