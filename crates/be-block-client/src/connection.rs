use std::{
    collections::{HashMap, VecDeque},
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll},
    time::Duration,
};

use be_block_commit::CommitId;
use be_block_protocol::{
    Authenticated, ClientId, Greeting, JoinedSession, Journal, JournalEntry, MembershipEntry,
    PairingMessage, Query, SessionEvent, SessionSend, SessionState, Target, Topic, Welcome, client,
    hello, read_hello,
};
use be_block_store::Hash;
use be_wasm_ops::{
    MethodId, OpId, Side,
    net::{Arrival, Caller, Peer},
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

/// The socket plus the greeting. Once connected no request has a deadline of its own: each is
/// answered or fails with `Disconnected` when the connection closes.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Silence after which the connection calls `connection.ping` ahead of anything queued. It is a
/// protocol call because a browser cannot send websocket pings; any frame from the server
/// proves it alive.
pub const PING_INTERVAL: Duration = Duration::from_secs(15);
/// How long after a ping is actually written, with nothing heard, before the connection closes
/// itself; a write still stuck this long after the ping fell due closes it too. Only an
/// unanswered ping closes it, so a process suspended for longer keeps a live connection when it
/// wakes.
pub const PONG_WAIT: Duration = Duration::from_secs(20);
/// Silence after something was sent that the connection takes as a sign the server may be gone:
/// it pings then rather than at `PING_INTERVAL`, so a link that went silent under a request is
/// noticed in `ANSWER_WAIT` plus `PONG_WAIT`. The ping waits as long as any, so a server that
/// is slow but answers is not taken for gone. A message that has no answer (a relay, an
/// append) on an otherwise quiet link costs one extra ping this long after it.
pub const ANSWER_WAIT: Duration = Duration::from_secs(5);

pub type Answer<T> = Pin<Box<dyn Future<Output = Result<T, ClientError>> + Send>>;

pub async fn ask<T>(
    pending: impl Future<Output = Result<Result<T, be_block_protocol::Error>, be_wasm_ops::OpError>>,
) -> Result<T, ClientError> {
    Ok(pending.await??)
}

type Pump = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    /// The tree of an entry point the connection follows changed, up to `sequence`: read it with
    /// `query.tree` (`Peer::changed_page`). The server tells nothing more of the entry until
    /// it is read to its end.
    TreeAdvanced {
        entry: Uuid,
        sequence: u64,
    },
    /// What lies at the top of the organization, in the trash or is shared with this account may
    /// have changed: list them again.
    Roots,
    /// The server wrote a membership entry into a block's session (`session-event.membership`).
    Membership {
        block: Uuid,
        entry: MembershipEntry,
    },
    /// A seal moved a block's head to `commit`, holding its session's order through `through`
    /// in its `run` (`session-event.sealed`).
    Sealed {
        block: Uuid,
        commit: CommitId,
        through: u64,
        run: u64,
    },
    Relayed {
        block: Uuid,
        from: ClientId,
        payload: Vec<u8>,
        /// When the server took it, by its clock (`relayed.received`).
        received: i64,
    },
    /// The order of a block's session numbered a submission, this connection's own too
    /// (`session-event.sequenced`); its payload sealed.
    Sequenced {
        block: Uuid,
        entry: JournalEntry,
    },
    Paired(PairingMessage),
    /// Keys this account may open were stored through another connection (`topic.keys`), or
    /// opened here by any path (`KeyBook::on_opened`: a fetch, a contact verified since, escrow):
    /// `Peer::keys_changed`, then reading again what was locked, opens what they unlock.
    KeysChanged,
    /// The account's notification modes changed on another of its devices
    /// (`topic.notify-modes`).
    NotifyModesChanged,
    /// A message arrived in the account's mailbox, or another connection deleted one
    /// (`topic.mailbox`): a notice, an accepted link's card, a contact's key change, or a record
    /// of a contact made or removed. A device that takes a record into the account's block
    /// deletes its letter, so this tells the account's other devices that the block changed too.
    MailboxChanged,
    /// The organization's key work for this account may have changed (`topic.key-work`): read it
    /// again.
    KeyWorkChanged,
    /// What the account's storage holds changed (`topic.storage`).
    StorageChanged,
}

#[derive(Default)]
struct Registry {
    subscribed: bool,
    /// The sessions this connection is in, by block, with the organization each block is in.
    sessions: HashMap<Uuid, Uuid>,
    /// The session events of each block being joined, held until the join is answered, so a
    /// subscriber hears none before it.
    joining: HashMap<Uuid, Vec<Event>>,
}

/// What a connection signs in with, in its greeting.
#[derive(Clone)]
pub enum Credentials {
    Token(String),
    /// The request's Authorization header, which a proxy injects (`authorize`): `be` behind
    /// one, which never holds the token.
    Header,
}

impl Credentials {
    fn token(&self) -> Option<String> {
        match self {
            Self::Token(token) => Some(token.clone()),
            Self::Header => None,
        }
    }
}

pub struct Connection {
    peer: Peer,
    shut: mpsc::UnboundedSender<()>,
    pumps: mpsc::UnboundedSender<Pump>,
    events: broadcast::Sender<Event>,
    closed: watch::Receiver<bool>,
    /// Asked to close (`close`): closed to whoever asks from then on, though its worker may
    /// still be shutting the socket.
    asked_to_close: std::sync::atomic::AtomicBool,
    due: watch::Receiver<Duration>,
    registry: Mutex<Registry>,
    /// The clock the connection keeps time by (`Clock::real` for an offline one).
    clock: Clock,
    /// What reaching the account's block learned, kept for the next pass
    /// (`AccountRecords::open`).
    pub(crate) account_block: Mutex<Option<crate::peer::account_block::Reached>>,
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

    fn forget(&self, op: OpId) {
        self.peer.forget(op);
    }

    fn request_cancel(&self, op: OpId) {
        self.peer.request_cancel(op);
    }
}

impl Connection {
    /// Opens a connection signed in with `credentials`, in its greeting: the connection, and
    /// who it signed in as. One the server does not sign in fails with its refusal, and is
    /// closed before any call.
    pub async fn connect(
        url: &str,
        clock: &Clock,
        credentials: &Credentials,
    ) -> Result<(Arc<Self>, Authenticated), ClientError> {
        let greeting = Greeting {
            token: credentials.token(),
        };
        let opening = std::pin::pin!(async {
            let (mut writer, mut reader) = connect(url).await.map_err(ClientError::Disconnected)?;
            let signed_in = greet(&mut writer, &mut reader, &greeting).await?;
            Ok::<_, ClientError>((writer, reader, signed_in))
        });
        let (writer, reader, signed_in) = match select(opening, clock.sleep(CONNECT_TIMEOUT)).await
        {
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
            asked_to_close: std::sync::atomic::AtomicBool::new(false),
            due: watched,
            registry: Mutex::new(Registry::default()),
            clock: clock.clone(),
            account_block: Mutex::new(None),
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
        Ok((connection, signed_in))
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
            asked_to_close: std::sync::atomic::AtomicBool::new(true),
            due,
            registry: Mutex::new(Registry::default()),
            clock: Clock::real(),
            account_block: Mutex::new(None),
        })
    }

    /// The clock this connection keeps time by: monotonic, or a test's manual one.
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    fn registry(&self) -> std::sync::MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn pump(&self, pump: impl Future<Output = ()> + Send + 'static) {
        let _ = self.pumps.send(Box::pin(pump));
    }

    /// Tells this connection's subscribers that keys this account may open arrived: a key opened
    /// here (`KeyBook::on_opened`), from a fetch, a contact verified since it was sealed, or
    /// escrow.
    pub(crate) fn keys_opened(&self) {
        let _ = self.events.send(Event::KeysChanged);
    }

    /// Hears what the server tells this connection (`server.events`) from now on: the trees,
    /// sessions and topics it follows (`Peer::follow_topics`), and its account's other
    /// connections' messages.
    pub fn subscribe(self: &Arc<Self>) -> broadcast::Receiver<Event> {
        let receiver = self.events.subscribe();
        if std::mem::replace(&mut self.registry().subscribed, true) {
            return receiver;
        }
        let mut told = client::server::events(&self.peer);
        let connection = Arc::clone(self);
        self.pump(async move {
            while let Some(event) = told.recv().await {
                if let Some(event) = connection.heard(event) {
                    let _ = connection.events.send(event);
                }
            }
        });
        receiver
    }

    /// What `event` tells this connection's subscribers; none for a session's while it is being
    /// joined, which waits for the join's answer (`join_session`).
    fn heard(&self, event: be_block_protocol::Event) -> Option<Event> {
        use be_block_protocol::Event as Told;
        Some(match event {
            Told::TreeAdvanced {
                entry, sequence, ..
            } => Event::TreeAdvanced { entry, sequence },
            Told::TopicChanged { topic, .. } => match topic {
                Topic::Roots => Event::Roots,
                Topic::Keys => Event::KeysChanged,
                Topic::KeyWork => Event::KeyWorkChanged,
                Topic::NotifyModes => Event::NotifyModesChanged,
                Topic::Mailbox => Event::MailboxChanged,
                Topic::Storage => Event::StorageChanged,
                Topic::Tree(_) | Topic::Session(_) => return None,
            },
            Told::Paired(message) => Event::Paired(message),
            Told::Session { block, event, .. } => {
                let event = match event {
                    SessionEvent::Membership(entry) => Event::Membership { block, entry },
                    SessionEvent::Sealed {
                        commit,
                        through,
                        run,
                    } => Event::Sealed {
                        block,
                        commit,
                        through,
                        run,
                    },
                    SessionEvent::Relayed {
                        from,
                        payload,
                        received,
                    } => Event::Relayed {
                        block,
                        from,
                        payload,
                        received,
                    },
                    SessionEvent::Sequenced(entry) => Event::Sequenced { block, entry },
                };
                let mut registry = self.registry();
                if let Some(held) = registry.joining.get_mut(&block) {
                    held.push(event);
                    return None;
                }
                event
            }
        })
    }

    /// Joins a block's session in `organization`, running the build `tag` of its type: this
    /// connection's id, the session's state, the block's journal as of joining, sealed, and
    /// whether this account may edit it as the server answered. What the order numbers after it
    /// arrives as `Event::Sequenced`, after the join is answered.
    pub fn join_session(
        self: &Arc<Self>,
        organization: Uuid,
        block: Uuid,
        tag: Hash,
    ) -> Answer<(ClientId, SessionState, Option<Journal>, bool)> {
        self.registry().joining.insert(block, Vec::new());
        let joining = be_block_protocol::ask::<JoinedSession, _>(
            &self.peer,
            Some(organization),
            Query::Session { block, tag },
            true,
        );
        let connection = Arc::clone(self);
        Box::pin(async move {
            let joined = joining.await;
            let held = {
                let mut registry = connection.registry();
                let held = registry.joining.remove(&block).unwrap_or_default();
                match &joined {
                    Ok(Ok(_)) => {
                        registry.sessions.insert(block, organization);
                    }
                    _ => {
                        registry.sessions.remove(&block);
                    }
                }
                held
            };
            let JoinedSession {
                client,
                state,
                journal,
                may_edit,
            } = joined??;
            for event in held {
                let _ = connection.events.send(event);
            }
            Ok((client, state, journal, may_edit))
        })
    }

    /// The organization of a block's session this connection is in.
    fn session(&self, block: Uuid) -> Result<Uuid, ClientError> {
        self.registry()
            .sessions
            .get(&block)
            .copied()
            .ok_or_else(|| {
                ClientError::Refused(
                    be_block_protocol::ErrorCode::InvalidRequest,
                    "join the session before speaking in it".into(),
                )
            })
    }

    pub fn leave_session(&self, block: Uuid) -> Answer<()> {
        let organization = self.registry().sessions.remove(&block);
        let left = organization.map(|organization| {
            client::server::unfollow(&self.peer, Some(organization), Topic::Session(block))
        });
        Box::pin(async move {
            if let Some(left) = left {
                let _ = left.await;
            }
            Ok(())
        })
    }

    fn send_session(&self, block: Uuid, kind: SessionSend, payload: Vec<u8>) -> Answer<()> {
        let sent = self.session(block).map(|organization| {
            client::server::send(
                &self.peer,
                Some(organization),
                Target::Session { block, kind },
                payload,
            )
        });
        Box::pin(async move { Ok(sent?.await??) })
    }

    /// Claims the lease of `block`'s session: the server writes the claim into its order.
    pub fn claim(&self, block: Uuid) -> Answer<()> {
        self.send_session(block, SessionSend::Claim, Vec::new())
    }

    pub fn relay(&self, block: Uuid, to: Option<ClientId>, payload: Vec<u8>) -> Answer<()> {
        self.send_session(block, SessionSend::Ephemeral(to), payload)
    }

    /// Hands the session's order a submission, its payload sealed: answered once the server
    /// numbered it and kept it in the journal, which every participant hears as
    /// `Event::Sequenced`.
    pub fn submit(&self, block: Uuid, submission: be_block_session::Submission) -> Answer<()> {
        let kind = SessionSend::Ordered(be_block_protocol::Submission {
            origin: submission.id.origin,
            counter: submission.id.counter,
            base: submission.base,
            barrier: submission.barrier,
        });
        self.send_session(block, kind, submission.payload)
    }

    pub fn closed(&self) -> watch::Receiver<bool> {
        self.closed.clone()
    }

    pub fn due(&self) -> watch::Receiver<Duration> {
        self.due.clone()
    }

    /// Whether the connection closed, or was asked to (`close`): one asked to close is never
    /// handed on, though its socket may take a moment to shut.
    pub fn is_closed(&self) -> bool {
        self.asked_to_close
            .load(std::sync::atomic::Ordering::Acquire)
            || *self.closed.borrow()
    }

    pub fn close(&self) {
        self.asked_to_close
            .store(true, std::sync::atomic::Ordering::Release);
        let _ = self.shut.send(());
    }
}

/// Greets the server with our build and `greeting`, and answers who its greeting says the
/// connection signed in as.
async fn greet(
    writer: &mut Writer,
    reader: &mut Reader,
    greeting: &Greeting,
) -> Result<Authenticated, ClientError> {
    let ours = be_lib_build_id::current().map_err(ClientError::NoBuild)?;
    writer
        .send(hello(ours, greeting))
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
    if !be_lib_build_id::compatible(ours, &theirs.build) {
        return Err(ClientError::Incompatible {
            ours: ours.to_owned(),
            theirs: theirs.build,
        });
    }
    let welcome: Welcome = theirs.said().ok_or_else(|| {
        ClientError::Disconnected("the server's greeting said nothing readable".into())
    })?;
    Ok(welcome?)
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
            be_wasm_ops::decode_frame::<be_wasm_ops::Frame>(frame),
            Ok(be_wasm_ops::Frame::Start { method, .. }) if method == be_block_protocol::__args::SERVER_PING
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
    let mut ping: Option<be_wasm_ops::net::Pending<()>> = None;
    let mut writer = Some(writer);
    let mut sending: Option<(Sending, Duration)> = None;
    let mut queued: VecDeque<(Out, bool)> = VecDeque::new();
    let mut heard = opened;
    let mut owed = false;
    let mut pinged: Option<Duration> = None;
    // When something other than a ping was written with nothing heard since.
    let mut asked: Option<Duration> = None;
    let mut sending_message = false;
    let mut reading_first = false;
    let mut shut = Some(shut);
    loop {
        let now = clock.now();
        let early = asked.is_some_and(|at| now >= at + ANSWER_WAIT);
        if !owed && pinged.is_none() && (now >= heard + PING_INTERVAL || early) {
            ping = Some(client::server::ping(&peer));
            owed = true;
        }
        while let Ok(frame) = frames.try_recv() {
            queue(&mut queued, frame);
        }
        if sending.is_none()
            && let Some((out, is_ping)) = queued.pop_front()
            && let Some(idle) = writer.take()
        {
            sending_message = matches!(out, Out::Message(_)) && !is_ping;
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
            (None, _) => match (owed, asked) {
                (false, Some(at)) => (heard + PING_INTERVAL).min(at + ANSWER_WAIT),
                _ => heard + PING_INTERVAL,
            },
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
                } else if sending_message && asked.is_none() {
                    asked = Some(clock.now());
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
                asked = None;
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

fn answered(ping: &mut be_wasm_ops::net::Pending<()>) -> bool {
    let mut context = Context::from_waker(std::task::Waker::noop());
    Pin::new(ping).poll(&mut context).is_ready()
}
