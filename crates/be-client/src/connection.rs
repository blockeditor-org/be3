use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::Poll,
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
    shut: mpsc::UnboundedSender<()>,
    events: broadcast::Sender<ServerMessage>,
    closed: watch::Receiver<bool>,
    due: watch::Receiver<Duration>,
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
        let (shut, shutting) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (closing, closed) = watch::channel(false);
        let opened = clock.now();
        let (due, watched) = watch::channel(opened + PING_INTERVAL);
        let connection = Arc::new(Self {
            commands,
            shut,
            events: events.clone(),
            closed,
            due: watched,
            next: AtomicU64::new(0),
        });
        spawn(worker(
            Carried {
                writer,
                reader,
                commands: receiver,
                shut: shutting,
                events,
                clock: clock.clone(),
                opened,
                due,
            },
            closing,
        ));
        Ok(connection)
    }

    pub fn offline() -> Arc<Self> {
        let (commands, _) = mpsc::unbounded_channel();
        let (shut, _) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(1);
        let (_, closed) = watch::channel(true);
        let (_, due) = watch::channel(Duration::MAX);
        Arc::new(Self {
            commands,
            shut,
            events,
            closed,
            due,
            next: AtomicU64::new(0),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.events.subscribe()
    }

    pub fn closed(&self) -> watch::Receiver<bool> {
        self.closed.clone()
    }

    pub fn due(&self) -> watch::Receiver<Duration> {
        self.due.clone()
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.borrow()
    }

    pub fn close(&self) {
        let _ = self.shut.send(());
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
    shut: mpsc::UnboundedReceiver<()>,
    events: broadcast::Sender<ServerMessage>,
    clock: Clock,
    opened: Duration,
    due: watch::Sender<Duration>,
}

async fn worker(carried: Carried, closing: watch::Sender<bool>) {
    carry(carried).await;
    let _ = closing.send(true);
}

enum Out {
    Message(Vec<u8>),
    #[cfg(not(target_arch = "wasm32"))]
    Pong(Vec<u8>),
}

#[cfg(not(target_arch = "wasm32"))]
type Sending = std::pin::Pin<Box<dyn Future<Output = (Writer, bool, Result<(), String>)> + Send>>;
#[cfg(target_arch = "wasm32")]
type Sending = std::pin::Pin<Box<dyn Future<Output = (Writer, bool, Result<(), String>)>>>;

fn write(mut writer: Writer, out: Out, ping: bool) -> Sending {
    Box::pin(async move {
        let written = match out {
            Out::Message(bytes) => writer.send(bytes).await,
            #[cfg(not(target_arch = "wasm32"))]
            Out::Pong(payload) => writer.pong(payload).await,
        };
        (writer, ping, written)
    })
}

enum Woke {
    Wrote(Writer, bool, Result<(), String>),
    Command(Option<Outbound>),
    Frame(Option<Frame>),
    Timer,
    Shut(bool),
}

async fn carry(
    Carried {
        writer,
        mut reader,
        mut commands,
        shut,
        events,
        clock,
        opened,
        due,
    }: Carried,
) {
    let Ok(ping) = encode(&ClientMessage::Ping {
        request: PING_REQUEST,
    }) else {
        return;
    };
    let mut pending: HashMap<u64, Reply> = HashMap::new();
    let mut writer = Some(writer);
    let mut sending: Option<(Sending, Duration)> = None;
    let mut queued: VecDeque<(Out, bool)> = VecDeque::new();
    let mut heard = opened;
    let mut owed = false;
    let mut pinged: Option<Duration> = None;
    let mut reading_first = false;
    let mut shut = Some(shut);
    loop {
        let now = clock.now();
        if !owed && pinged.is_none() && now >= heard + PING_INTERVAL {
            queued.push_front((Out::Message(ping.clone()), true));
            owed = true;
        }
        if sending.is_none()
            && let Some((out, is_ping)) = queued.pop_front()
            && let Some(idle) = writer.take()
        {
            sending = Some((write(idle, out, is_ping), now));
        }
        let stuck = sending
            .as_ref()
            .map(|(_, since)| (heard + PING_INTERVAL).max(*since) + PONG_WAIT);
        let next = match (pinged, stuck) {
            (Some(sent), stuck) => {
                stuck.map_or(sent + PONG_WAIT, |stuck| stuck.min(sent + PONG_WAIT))
            }
            (None, Some(stuck)) if owed => stuck,
            (None, _) => heard + PING_INTERVAL,
        };
        due.send_if_modified(|due| std::mem::replace(due, next) != next);
        let woke = {
            let mut frame = std::pin::pin!(reader.next());
            let mut timer = std::pin::pin!(clock.sleep_until(next));
            let accepting = sending.is_none();
            std::future::poll_fn(|context| {
                if let Some(shutting) = &mut shut
                    && let Poll::Ready(shutting) = shutting.poll_recv(context)
                {
                    return Poll::Ready(Woke::Shut(shutting.is_some()));
                }
                if let Some((write, _)) = &mut sending
                    && let Poll::Ready((writer, ping, written)) = write.as_mut().poll(context)
                {
                    return Poll::Ready(Woke::Wrote(writer, ping, written));
                }
                for reading in [reading_first, !reading_first] {
                    if reading {
                        if let Poll::Ready(frame) = frame.as_mut().poll(context) {
                            return Poll::Ready(Woke::Frame(frame));
                        }
                    } else if accepting && let Poll::Ready(command) = commands.poll_recv(context) {
                        return Poll::Ready(Woke::Command(command));
                    }
                }
                timer.as_mut().poll(context).map(|()| Woke::Timer)
            })
            .await
        };
        reading_first = matches!(woke, Woke::Command(_));
        match woke {
            Woke::Shut(true) => return,
            Woke::Shut(false) => shut = None,
            Woke::Wrote(_, _, Err(_)) => return,
            Woke::Wrote(idle, ping, Ok(())) => {
                writer = Some(idle);
                sending = None;
                if ping {
                    owed = false;
                    pinged = Some(clock.now());
                }
            }
            Woke::Timer => {
                let now = clock.now();
                if pinged.is_some_and(|sent| now >= sent + PONG_WAIT)
                    || stuck.is_some_and(|stuck| now >= stuck)
                {
                    return;
                }
            }
            Woke::Command(None) => return,
            Woke::Command(Some(command)) => {
                if command.reply.is_closed() {
                    continue;
                }
                pending.insert(command.request, command.reply);
                queued.push_back((Out::Message(command.bytes), false));
            }
            Woke::Frame(frame) => {
                heard = clock.now();
                pinged = None;
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
                            Some(reply) => {
                                let _ = reply.send(Ok(message));
                            }
                            None => {
                                let _ = events.send(message);
                            }
                        }
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    Some(Frame::Ping(payload)) => queued.push_back((Out::Pong(payload), false)),
                    #[cfg(not(target_arch = "wasm32"))]
                    Some(Frame::Failed(_)) => return,
                    Some(Frame::Closed) | None => return,
                }
            }
        }
    }
}
