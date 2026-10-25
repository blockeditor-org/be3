use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use be_protocol::{ClientMessage, ServerMessage, decode, encode, hello, read_hello};
use futures_util::future::{Either, select};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::{
    ClientError, Clock,
    transport::{Frame, Reader, Writer, connect, spawn},
};

const EVENT_CAPACITY: usize = 4096;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
pub const PING_INTERVAL: Duration = Duration::from_secs(15);
pub const PONG_WAIT: Duration = Duration::from_secs(20);

const PING_REQUEST: u64 = 0;

type Reply = oneshot::Sender<Result<ServerMessage, ClientError>>;

struct Outbound {
    request: u64,
    bytes: Vec<u8>,
    reply: Reply,
}

pub struct Connection {
    commands: mpsc::UnboundedSender<Outbound>,
    events: broadcast::Sender<ServerMessage>,
    closed: watch::Receiver<bool>,
    next: AtomicU64,
}

impl Connection {
    pub async fn connect(url: &str, clock: &Clock) -> Result<Arc<Self>, ClientError> {
        let opening = std::pin::pin!(async {
            let (mut writer, mut reader) = connect(url).await.map_err(ClientError::Disconnected)?;
            greet(&mut writer, &mut reader).await?;
            Ok::<_, ClientError>((writer, reader))
        });
        let (writer, reader) = match select(opening, clock.sleep(CONNECT_TIMEOUT)).await {
            Either::Left((opened, _)) => opened?,
            Either::Right(_) => return Err(ClientError::TimedOut("connecting to the server")),
        };
        let (commands, receiver) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (closing, closed) = watch::channel(false);
        let connection = Arc::new(Self {
            commands,
            events: events.clone(),
            closed,
            next: AtomicU64::new(0),
        });
        spawn(worker(
            Carried {
                writer,
                reader,
                commands: receiver,
                events,
                clock: clock.clone(),
            },
            closing,
        ));
        Ok(connection)
    }

    pub fn offline() -> Arc<Self> {
        let (commands, _) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(1);
        let (_, closed) = watch::channel(true);
        Arc::new(Self {
            commands,
            events,
            closed,
            next: AtomicU64::new(0),
        })
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
        self.send(build)?.await
    }

    pub fn send(&self, build: impl FnOnce(u64) -> ClientMessage) -> Result<Answer, ClientError> {
        let message = build(self.next.fetch_add(1, Ordering::Relaxed) + 1);
        let request = message.request();
        let bytes = encode(&message).map_err(|error| ClientError::Encoding(error.to_string()))?;
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Outbound {
                request,
                bytes,
                reply,
            })
            .map_err(|_| ClientError::Disconnected("the connection worker stopped".into()))?;
        Ok(Box::pin(async move {
            let response = response
                .await
                .map_err(|_| ClientError::Disconnected("the connection closed".into()))??;
            match response {
                ServerMessage::Failed { code, message, .. } => {
                    Err(ClientError::Refused(code, message))
                }
                response => Ok(response),
            }
        }))
    }
}

pub type Answer =
    std::pin::Pin<Box<dyn Future<Output = Result<ServerMessage, ClientError>> + Send>>;

async fn greet(writer: &mut Writer, reader: &mut Reader) -> Result<(), ClientError> {
    let ours = build_id::current().map_err(ClientError::NoBuild)?;
    writer
        .send(hello(ours))
        .await
        .map_err(ClientError::Disconnected)?;
    let Some(bytes) = next_binary(writer, reader).await? else {
        return Err(ClientError::Disconnected(
            "the server closed the connection before greeting it".into(),
        ));
    };
    let theirs = read_hello(&bytes).ok_or_else(|| ClientError::Incompatible {
        ours: ours.to_owned(),
        theirs: "a build that does not greet".to_owned(),
    })?;
    if build_id::compatible(ours, &theirs) {
        Ok(())
    } else {
        Err(ClientError::Incompatible {
            ours: ours.to_owned(),
            theirs,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn next_binary(
    writer: &mut Writer,
    reader: &mut Reader,
) -> Result<Option<Vec<u8>>, ClientError> {
    loop {
        match reader.next().await {
            Some(Frame::Binary(bytes)) => return Ok(Some(bytes)),
            Some(Frame::Ping(payload)) => writer
                .pong(payload)
                .await
                .map_err(ClientError::Disconnected)?,
            Some(Frame::Failed(reason)) => return Err(ClientError::Disconnected(reason)),
            Some(Frame::Closed) | None => return Ok(None),
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn next_binary(
    _writer: &mut Writer,
    reader: &mut Reader,
) -> Result<Option<Vec<u8>>, ClientError> {
    Ok(match reader.next().await {
        Some(Frame::Binary(bytes)) => Some(bytes),
        Some(Frame::Closed) | None => None,
    })
}

struct Carried {
    writer: Writer,
    reader: Reader,
    commands: mpsc::UnboundedReceiver<Outbound>,
    events: broadcast::Sender<ServerMessage>,
    clock: Clock,
}

async fn worker(carried: Carried, closing: watch::Sender<bool>) {
    carry(carried).await;
    let _ = closing.send(true);
}

enum Woke {
    Command(Option<Outbound>),
    Frame(Option<Frame>),
    Timer,
}

async fn carry(
    Carried {
        mut writer,
        mut reader,
        mut commands,
        events,
        clock,
    }: Carried,
) {
    let mut pending: HashMap<u64, (Duration, Reply)> = HashMap::new();
    let mut heard = clock.now();
    let mut pinged = false;
    loop {
        let deadline = |written: Duration, heard: Duration| written.max(heard) + REQUEST_TIMEOUT;
        pending.retain(|_, (_, reply)| !reply.is_closed());
        if !pinged && clock.now() >= heard + PING_INTERVAL {
            let Ok(ping) = encode(&ClientMessage::Ping {
                request: PING_REQUEST,
            }) else {
                return;
            };
            if writer.send(ping).await.is_err() {
                return;
            }
            pinged = true;
        }
        let next = if pinged {
            heard + PING_INTERVAL + PONG_WAIT
        } else {
            heard + PING_INTERVAL
        };
        let next = pending
            .values()
            .map(|(written, _)| deadline(*written, heard))
            .fold(next, Duration::min);
        let woke = {
            let command = std::pin::pin!(commands.recv());
            let frame = std::pin::pin!(reader.next());
            let timer = clock.sleep_until(next);
            match select(select(command, frame), timer).await {
                Either::Left((Either::Left((command, _)), _)) => Woke::Command(command),
                Either::Left((Either::Right((frame, _)), _)) => Woke::Frame(frame),
                Either::Right(_) => Woke::Timer,
            }
        };
        match woke {
            Woke::Timer => {
                let now = clock.now();
                let expired: Vec<u64> = pending
                    .iter()
                    .filter(|(_, (written, _))| deadline(*written, heard) <= now)
                    .map(|(request, _)| *request)
                    .collect();
                for request in expired {
                    if let Some((_, reply)) = pending.remove(&request) {
                        let _ = reply.send(Err(ClientError::TimedOut("a request to the server")));
                    }
                }
                if now >= heard + PING_INTERVAL + PONG_WAIT {
                    return;
                }
            }
            Woke::Command(None) => return,
            Woke::Command(Some(command)) => {
                let started = clock.now();
                pending.insert(command.request, (started, command.reply));
                if writer.send(command.bytes).await.is_err() {
                    return;
                }
                heard += clock.now().saturating_sub(started);
            }
            Woke::Frame(frame) => {
                heard = clock.now();
                pinged = false;
                match frame {
                    Some(Frame::Binary(bytes)) => {
                        let Ok(message) = decode::<ServerMessage>(&bytes) else {
                            continue;
                        };
                        if matches!(message, ServerMessage::Pong { .. }) {
                            continue;
                        }
                        match message
                            .request()
                            .and_then(|request| pending.remove(&request))
                        {
                            Some((_, reply)) => {
                                let _ = reply.send(Ok(message));
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
                    #[cfg(not(target_arch = "wasm32"))]
                    Some(Frame::Failed(_)) => return,
                    Some(Frame::Closed) | None => return,
                }
            }
        }
    }
}
