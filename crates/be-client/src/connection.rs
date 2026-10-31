use std::{
    collections::{HashMap, VecDeque},
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll},
    time::Duration,
};

use be_commit::CommitId;
use be_ops::{
    MethodId, OpId, Side,
    net::{Arrival, Caller, Peer},
};
use be_protocol::{
    BlockEvent, ClientId, PairingMessage, SessionEvent, SessionState, client, hello, read_hello,
};
use futures_util::{
    StreamExt,
    future::{Either, select},
    stream::FuturesUnordered,
};
use tokio::sync::{broadcast, mpsc, watch};
use uuid::Uuid;

use crate::{
    ClientError, Clock,
    transport::{Frame, Reader, Writer, connect, spawn},
};

const EVENT_CAPACITY: usize = 4096;

const PING_FRAME_BYTES: usize = 32;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
pub const PING_INTERVAL: Duration = Duration::from_secs(15);
pub const PONG_WAIT: Duration = Duration::from_secs(20);

pub type Answer<T> = Pin<Box<dyn Future<Output = Result<T, ClientError>> + Send>>;

pub async fn ask<T: be_ops::DeserializeOwned>(
    pending: be_ops::net::Pending<Result<T, be_protocol::Error>>,
) -> Result<T, ClientError> {
    Ok(pending.await??)
}

type Pump = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    BlockChanged(be_protocol::BlockSummary),
    BlockRemoved {
        block: Uuid,
        revision: u64,
    },
    HeadChanged {
        block: Uuid,
        head: CommitId,
        author: Uuid,
    },
    BlockDeleted {
        block: Uuid,
    },
    SessionChanged {
        block: Uuid,
        state: SessionState,
    },
    Relayed {
        block: Uuid,
        from: ClientId,
        payload: Vec<u8>,
    },
    Paired(PairingMessage),
    ContactsChanged,
    KeyWorkChanged,
}

struct Watching {
    count: usize,
    _stop: tokio::sync::oneshot::Sender<()>,
}

#[derive(Default)]
struct Registry {
    subscribed: bool,
    watching: HashMap<Uuid, Watching>,
    sessions: HashMap<Uuid, Arc<client::sessions::Session>>,
}

pub struct Connection {
    peer: Peer,
    shut: mpsc::UnboundedSender<()>,
    pumps: mpsc::UnboundedSender<Pump>,
    events: broadcast::Sender<Event>,
    closed: watch::Receiver<bool>,
    due: watch::Receiver<Duration>,
    registry: Mutex<Registry>,
}

impl Caller for Connection {
    fn start(&self, parent: Option<OpId>, method: MethodId, args: Vec<u8>) -> OpId {
        self.peer.start(parent, method, args)
    }

    fn poll_op(&self, op: OpId, context: &mut Context<'_>) -> Poll<Arrival> {
        self.peer.poll_op(op, context)
    }

    fn credit(&self, op: OpId, items: u32, bytes: u64) {
        self.peer.credit(op, items, bytes);
    }

    fn cancel(&self, op: OpId) {
        self.peer.cancel(op);
    }

    fn request_cancel(&self, op: OpId) {
        self.peer.request_cancel(op);
    }
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
        let (outbound, frames) = mpsc::unbounded_channel();
        let peer = Peer::new(Side::Guest, move |frame| {
            let _ = outbound.send(frame);
        });
        let (shut, shutting) = mpsc::unbounded_channel();
        let (pumps, pumping) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (closing, closed) = watch::channel(false);
        let opened = clock.now();
        let (due, watched) = watch::channel(opened + PING_INTERVAL);
        let connection = Arc::new(Self {
            peer: peer.clone(),
            shut,
            pumps,
            events,
            closed,
            due: watched,
            registry: Mutex::new(Registry::default()),
        });
        spawn(worker(
            Carried {
                writer,
                reader,
                peer,
                frames,
                shut: shutting,
                pumps: pumping,
                clock: clock.clone(),
                opened,
                due,
            },
            closing,
        ));
        Ok(connection)
    }

    pub fn offline() -> Arc<Self> {
        let peer = Peer::new(Side::Guest, |_| {});
        peer.close();
        let (shut, _) = mpsc::unbounded_channel();
        let (pumps, _) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(1);
        let (_, closed) = watch::channel(true);
        let (_, due) = watch::channel(Duration::MAX);
        Arc::new(Self {
            peer,
            shut,
            pumps,
            events,
            closed,
            due,
            registry: Mutex::new(Registry::default()),
        })
    }

    fn registry(&self) -> std::sync::MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn pump(&self, pump: impl Future<Output = ()> + Send + 'static) {
        let _ = self.pumps.send(Box::pin(pump));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        let receiver = self.events.subscribe();
        if std::mem::replace(&mut self.registry().subscribed, true) {
            return receiver;
        }
        let events = self.events.clone();
        let mut changes = client::blocks::changes(&self.peer);
        self.pump(async move {
            while let Some(change) = changes.recv().await {
                let _ = events.send(match change {
                    be_protocol::BlockChange::Changed(block) => Event::BlockChanged(block),
                    be_protocol::BlockChange::Removed { block, revision } => {
                        Event::BlockRemoved { block, revision }
                    }
                });
            }
        });
        let events = self.events.clone();
        let mut paired = client::account::paired(&self.peer);
        self.pump(async move {
            while let Some(message) = paired.recv().await {
                let _ = events.send(Event::Paired(message));
            }
        });
        let events = self.events.clone();
        let mut contacts = client::contacts::contacts_changed(&self.peer);
        self.pump(async move {
            while contacts.recv().await.is_some() {
                let _ = events.send(Event::ContactsChanged);
            }
        });
        let events = self.events.clone();
        let mut key_work = client::keys::key_work_changed(&self.peer);
        self.pump(async move {
            while key_work.recv().await.is_some() {
                let _ = events.send(Event::KeyWorkChanged);
            }
        });
        receiver
    }

    pub fn watch(&self, block: Uuid) {
        let mut registry = self.registry();
        if let Some(watching) = registry.watching.get_mut(&block) {
            watching.count += 1;
            return;
        }
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        registry.watching.insert(
            block,
            Watching {
                count: 1,
                _stop: stop,
            },
        );
        drop(registry);
        let events = self.events.clone();
        let mut heard = client::blocks::watch(&self.peer, block);
        self.pump(async move {
            let heard = async move {
                while let Some(event) = heard.recv().await {
                    let _ = events.send(match event {
                        BlockEvent::Moved { head, author } => Event::HeadChanged {
                            block,
                            head,
                            author,
                        },
                        BlockEvent::Deleted => Event::BlockDeleted { block },
                    });
                }
            };
            select(std::pin::pin!(heard), stopped).await;
        });
    }

    pub fn unwatch(&self, block: Uuid) {
        let mut registry = self.registry();
        if let Some(watching) = registry.watching.get_mut(&block) {
            watching.count -= 1;
            if watching.count == 0 {
                registry.watching.remove(&block);
            }
        }
    }

    pub fn join_session(self: &Arc<Self>, block: Uuid) -> Answer<(ClientId, SessionState)> {
        let session = Arc::new(client::sessions::Session::new(&self.peer, block));
        let mut joining = session.join();
        let left = self.registry().sessions.insert(block, Arc::clone(&session));
        drop(left);
        let connection = Arc::clone(self);
        Box::pin(async move {
            let joined = match joining.recv().await {
                Some(SessionEvent::Joined { client, state }) => Ok((client, state)),
                Some(SessionEvent::Refused(error)) => Err(ClientError::from(error)),
                Some(SessionEvent::Changed(_) | SessionEvent::Relayed { .. }) => {
                    Err(ClientError::Unexpected)
                }
                None => Err(joining
                    .ended()
                    .and_then(Result::err)
                    .map_or(ClientError::Unexpected, ClientError::from)),
            };
            if joined.is_err() {
                connection.forget_session(block, &session);
                return joined;
            }
            let events = connection.events.clone();
            connection.pump(async move {
                while let Some(event) = joining.recv().await {
                    let _ = events.send(match event {
                        SessionEvent::Changed(state) => Event::SessionChanged { block, state },
                        SessionEvent::Relayed { from, payload } => Event::Relayed {
                            block,
                            from,
                            payload,
                        },
                        SessionEvent::Joined { .. } | SessionEvent::Refused(_) => continue,
                    });
                }
            });
            joined
        })
    }

    fn forget_session(&self, block: Uuid, session: &Arc<client::sessions::Session>) {
        let mut registry = self.registry();
        if registry
            .sessions
            .get(&block)
            .is_some_and(|held| Arc::ptr_eq(held, session))
        {
            registry.sessions.remove(&block);
        }
    }

    fn session(&self, block: Uuid) -> Result<Arc<client::sessions::Session>, ClientError> {
        self.registry()
            .sessions
            .get(&block)
            .cloned()
            .ok_or_else(|| {
                ClientError::Refused(
                    be_protocol::ErrorCode::InvalidRequest,
                    "join the session before speaking in it".into(),
                )
            })
    }

    pub fn leave_session(&self, block: Uuid) -> Answer<()> {
        let session = self.registry().sessions.remove(&block);
        Box::pin(async move {
            if let Some(session) = session.and_then(Arc::into_inner) {
                let _ = session.close().await;
            }
            Ok(())
        })
    }

    pub fn claim_ownership(&self, block: Uuid, generation: u64) -> Answer<SessionState> {
        let claimed = self
            .session(block)
            .map(|session| session.claim_ownership(generation));
        Box::pin(async move { Ok(claimed?.await??) })
    }

    pub fn heartbeat(
        &self,
        block: Uuid,
        generation: u64,
        clean_at: Option<CommitId>,
    ) -> Answer<SessionState> {
        let beat = self
            .session(block)
            .map(|session| session.heartbeat(generation, clean_at));
        Box::pin(async move { Ok(beat?.await??) })
    }

    pub fn relay(&self, block: Uuid, to: Option<ClientId>, payload: Vec<u8>) -> Answer<()> {
        let relayed = self
            .session(block)
            .map(|session| session.relay(to, payload));
        Box::pin(async move { Ok(relayed?.await??) })
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
}

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

fn is_ping(frame: &[u8]) -> bool {
    frame.len() <= PING_FRAME_BYTES
        && matches!(
            be_ops::decode_frame::<be_ops::Frame>(frame),
            Ok(be_ops::Frame::Start { method, .. }) if method == be_protocol::__args::CONNECTION_PING
        )
}

fn queue(queued: &mut VecDeque<(Out, bool)>, frame: Vec<u8>) {
    match is_ping(&frame) {
        true => queued.push_front((Out::Message(frame), true)),
        false => queued.push_back((Out::Message(frame), false)),
    }
}

struct Carried {
    writer: Writer,
    reader: Reader,
    peer: Peer,
    frames: mpsc::UnboundedReceiver<Vec<u8>>,
    shut: mpsc::UnboundedReceiver<()>,
    pumps: mpsc::UnboundedReceiver<Pump>,
    clock: Clock,
    opened: Duration,
    due: watch::Sender<Duration>,
}

async fn worker(carried: Carried, closing: watch::Sender<bool>) {
    let peer = carried.peer.clone();
    carry(carried).await;
    peer.close();
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
    Outbound(Option<Vec<u8>>),
    Frame(Option<Frame>),
    Timer,
    Shut(bool),
}

async fn carry(
    Carried {
        writer,
        mut reader,
        peer,
        mut frames,
        shut,
        mut pumps,
        clock,
        opened,
        due,
    }: Carried,
) {
    let mut running: FuturesUnordered<Pump> = FuturesUnordered::new();
    let mut ping: Option<be_ops::net::Pending<()>> = None;
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
            ping = Some(client::connection::ping(&peer));
            owed = true;
        }
        while let Ok(frame) = frames.try_recv() {
            queue(&mut queued, frame);
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
                while let Poll::Ready(Some(pump)) = pumps.poll_recv(context) {
                    running.push(pump);
                }
                while let Poll::Ready(Some(())) = running.poll_next_unpin(context) {}
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
                    } else if accepting && let Poll::Ready(outbound) = frames.poll_recv(context) {
                        return Poll::Ready(Woke::Outbound(outbound));
                    }
                }
                timer.as_mut().poll(context).map(|()| Woke::Timer)
            })
            .await
        };
        reading_first = matches!(woke, Woke::Outbound(_));
        match woke {
            Woke::Shut(true) => return,
            Woke::Shut(false) => shut = None,
            Woke::Wrote(_, _, Err(_)) => return,
            Woke::Wrote(idle, is_ping, Ok(())) => {
                writer = Some(idle);
                sending = None;
                if is_ping {
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
            Woke::Outbound(None) => return,
            Woke::Outbound(Some(frame)) => queue(&mut queued, frame),
            Woke::Frame(frame) => {
                heard = clock.now();
                pinged = None;
                match frame {
                    Some(Frame::Binary(bytes)) => {
                        if peer.receive(&bytes).is_err() {
                            return;
                        }
                        if ping.as_mut().is_some_and(answered) {
                            ping = None;
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

fn answered(ping: &mut be_ops::net::Pending<()>) -> bool {
    let mut context = Context::from_waker(std::task::Waker::noop());
    Pin::new(ping).poll(&mut context).is_ready()
}
