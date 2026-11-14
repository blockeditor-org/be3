use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
    time::Instant,
};

use be_wasm_threads::Mutex;
use std::sync::PoisonError;

use be_block_client::{Answer, ClientError, Event, Peer, Reconciled, Saved, Sealed, Write};
use be_block_commit::CommitId;
use be_block_graph::BlockParent;
use be_block_protocol::{ClientId, ErrorCode, SessionState};
use be_sdk_type_api::ops::wire::{self, Decode};
use be_sdk_type_api::{
    Change, Description, EditorId, Endpoint, Opening, PlainFile, PublishOutcome, Recipients,
    ReplicaId, Resume, Status,
    embed::{
        exports::replicas::{self, Replica},
        host::Call,
    },
    ops::{Connection, OpError, OpId, Outcome, Reply, Ticket},
};
use be_sdk_type_guest::editor::FromEditor;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use uuid::Uuid;

use crate::{
    Host,
    modules::{self as types, Heard, Modules},
};

pub const HOST_EDITOR: EditorId = u64::MAX;

pub const OFFLINE_CLIENT: ClientId = 0;

const JOINING_OWNER: ClientId = ClientId::MAX;

pub const EARLY_OPEN: std::time::Duration = std::time::Duration::from_millis(250);

fn offline_session(head: Option<CommitId>) -> SessionState {
    SessionState {
        generation: 0,
        owner: Some(OFFLINE_CLIENT),
        participants: vec![OFFLINE_CLIENT],
        clean_at: head,
    }
}

fn joining_session(head: Option<CommitId>) -> SessionState {
    SessionState {
        generation: 0,
        owner: Some(JOINING_OWNER),
        participants: vec![JOINING_OWNER, OFFLINE_CLIENT],
        clean_at: head,
    }
}

/// The server's journal of a block as joining answered it: sealed, and opened.
type ServerJournal = (be_block_protocol::Journal, be_block_protocol::Journal);

type Joined = Result<
    (
        ClientId,
        SessionState,
        Option<CommitId>,
        Option<CommitId>,
        Option<ServerJournal>,
    ),
    ClientError,
>;

/// The client, session and remote head, what commits of the block this device keeps, and the
/// server's journal as of joining, opened.
type Rejoined = Result<
    (
        ClientId,
        SessionState,
        Option<CommitId>,
        Kept,
        Option<ServerJournal>,
    ),
    ClientError,
>;

/// What a replica opens knowing of this device's kept commits: those in the outbox, and the
/// refused ones set aside.
type Kept = (be_sdk_type_api::KeptCommits, Vec<CommitId>);

type Pending<T> = std::pin::Pin<Box<dyn Future<Output = T>>>;

type Waiting = tokio::sync::oneshot::Sender<Outcome>;

type Content = Ticket<Option<Vec<u8>>, wire::Optional<wire::Bytes>>;

pub(crate) fn answer_in(
    answered: &mut tokio::sync::oneshot::Receiver<Outcome>,
) -> Option<Result<Outcome, OpError>> {
    match answered.try_recv() {
        Ok(outcome) => Some(Ok(outcome)),
        Err(tokio::sync::oneshot::error::TryRecvError::Closed) => Some(Err(OpError::Canceled)),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty) => None,
    }
}

/// An announcement as the type module sent it.
fn announcement(sent: be_sdk_type_api::Announcement) -> be_block_content::Announcement {
    be_block_content::Announcement {
        key: sent.key,
        text: sent.text,
        several: sent.several,
        mentions: sent.mentions,
    }
}

/// Sends what the edits the server now holds announce (`Peer::send_announcements`); an
/// unreachable server leaves them for the next time.
pub async fn send_announcements<H: Host>(peer: &Peer<H::Store>, shared: &Arc<Mutex<H>>) {
    if let Err(error) = peer.send_announcements().await
        && !unreachable(&error)
    {
        H::record(shared, error);
    }
}

fn unreachable(error: &ClientError) -> bool {
    error.is_unreachable()
}

const PUBLISHING: u8 = 0;
const RELEASED: u8 = 1;
const SETTLED: u8 = 2;

#[derive(Default)]
struct Publishing {
    state: std::sync::atomic::AtomicU8,
    kept: Mutex<Option<CommitId>>,
}

impl Publishing {
    fn keep(&self, commit: CommitId) {
        *self.kept.lock().unwrap_or_else(PoisonError::into_inner) = Some(commit);
    }

    fn kept(&self) -> Option<CommitId> {
        *self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn release(&self) -> bool {
        self.moved(RELEASED)
    }

    fn settle(&self) -> bool {
        self.moved(SETTLED)
    }

    fn moved(&self, to: u8) -> bool {
        use std::sync::atomic::Ordering::SeqCst;
        self.state
            .compare_exchange(PUBLISHING, to, SeqCst, SeqCst)
            .is_ok()
    }
}

struct Unanswered {
    replica: ReplicaId,
    module: usize,
    generation: u64,
    epoch: u64,
    reply: Reply<Result<PublishOutcome, String>>,
    publishing: Arc<Publishing>,
}

pub enum Answered {
    Load(
        Reply<Result<be_sdk_type_api::Loaded, String>>,
        Result<be_sdk_type_api::Loaded, String>,
    ),
    ReadPart(
        Reply<Result<Vec<u8>, String>, wire::Fallible<wire::Bytes, wire::Plain>>,
        Result<Vec<u8>, String>,
    ),
    PutPart(
        Reply<Result<be_sdk_type_api::PartRef, String>>,
        Result<be_sdk_type_api::PartRef, String>,
    ),
    Stash(Reply<Result<CommitId, String>>, Result<CommitId, String>),
    Resume(Reply<Result<Resume, String>>, Result<Resume, String>),
    Append(Reply<bool>, bool),
}

impl Answered {
    fn heads(&self) -> bool {
        matches!(self, Answered::Stash(..))
    }

    fn send(self, connection: &mut Connection) {
        match self {
            Answered::Load(reply, result) => reply.send(connection, &result),
            Answered::ReadPart(reply, result) => reply.send(connection, &result),
            Answered::PutPart(reply, result) => reply.send(connection, &result),
            Answered::Stash(reply, result) => reply.send(connection, &result),
            Answered::Resume(reply, result) => reply.send(connection, &result),
            Answered::Append(reply, held) => reply.send(connection, &held),
        }
    }
}

pub enum Io {
    Answer {
        replica: ReplicaId,
        module: usize,
        generation: u64,
        epoch: u64,
        answer: Box<Answered>,
        published: Option<CommitId>,
    },
    /// A read of the replica's met a key this device does not hold: `arrivals` as the read
    /// started (`Peer::arrivals`), so one that keys arrived during is not taken for locked.
    Locked {
        replica: ReplicaId,
        arrivals: u64,
    },
    Published {
        publish: u64,
        result: Result<PublishOutcome, String>,
        published: Option<CommitId>,
        kept: bool,
    },
    Relayed,
    Left,
    Sealed,
    /// A search for `key`, which relayed messages waited on, answered: how many keys it opened.
    Keyed {
        key: Uuid,
        fetched: Result<usize, ClientError>,
    },
    Joined {
        replica: ReplicaId,
        block: Uuid,
        joined: Box<Joined>,
        /// `Peer::arrivals` as the join started.
        arrivals: u64,
    },
    Rejoined {
        replica: ReplicaId,
        block: Uuid,
        rejoin: u64,
        joined: Box<Rejoined>,
    },
    Claimed {
        replica: ReplicaId,
        claimed: Box<Result<SessionState, ClientError>>,
    },
    Resynchronized {
        replica: ReplicaId,
        joined: Box<Result<(ClientId, SessionState), ClientError>>,
    },
    /// A `read-part` made under a stateless request, read.
    Read {
        module: usize,
        generation: u64,
        request: OpId,
        read: Box<types::PartRead>,
        result: Result<Vec<u8>, ClientError>,
    },
}

/// A merge's answer: the commit to write, which reuses the parts a type that merges by parts
/// kept as they were, and how many conflicts it kept both sides of.
pub struct Merged {
    pub content: be_sdk_type_api::Written,
    pub conflicts: usize,
    /// The parts the type read to merge, in the order it asked.
    pub read: Vec<be_block_store::PartRef>,
}

impl Merged {
    pub fn parts(&self) -> be_block_store::Part<'_> {
        be_sdk_type_api::written_parts(&self.content.bytes, &self.content.parts)
    }
}

/// A block's searchable text, as its type answered `search-text` and the host checked it.
#[derive(Debug)]
pub struct Searched {
    pub documents: Vec<be_sdk_type_api::SearchDocument>,
    /// The parts the type read, in the order it asked.
    pub read: Vec<be_block_store::PartRef>,
}

/// Why `Replicas::search_text` has no answer.
#[derive(Debug)]
pub enum NotSearched {
    /// A part could not be read (offline, a key this device lacks): try again later.
    Read(ClientError),
    /// No module has the type, or it failed or broke the request's rules: the message.
    Type(String),
}

impl std::fmt::Display for NotSearched {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "{error}"),
            Self::Type(message) => formatter.write_str(message),
        }
    }
}

#[derive(Clone, Copy)]
enum Asked {
    Content,
    Seal(bool),
}

impl Asked {
    fn send(self, replica: Replica, connection: &mut Connection) -> OpId {
        match self {
            Asked::Content => replica.content(connection).op(),
            Asked::Seal(force) => replica.seal(connection, force).op(),
        }
    }
}

#[derive(Clone)]
enum Replayed {
    Message {
        from: Endpoint,
        bytes: Vec<u8>,
        at: u64,
        /// What the server says of a peer's message.
        delivery: Option<be_sdk_type_api::Delivery>,
    },
    Moved {
        head: CommitId,
    },
    Lease {
        session: SessionState,
        resynchronize: bool,
    },
    Change {
        change: Change,
        op: Option<OpId>,
    },
}

impl Replayed {
    fn local_edit(&self) -> bool {
        matches!(
            self,
            Replayed::Message {
                from: Endpoint::Editor(_),
                ..
            } | Replayed::Change { .. }
        )
    }

    fn send(&self, replica: Replica, connection: &mut Connection) -> Option<OpId> {
        match self.clone() {
            Replayed::Change { change, .. } => {
                return Some(replica.change(connection, change).op());
            }
            Replayed::Message {
                from,
                bytes,
                at,
                delivery,
            } => {
                replica.message(connection, from, bytes, at, delivery);
            }
            Replayed::Moved { head } => {
                replica.head_moved(connection, head);
            }
            Replayed::Lease {
                session,
                resynchronize,
            } => {
                replica.lease(connection, session, resynchronize);
            }
        }
        None
    }
}

fn resend(
    handle: Replica,
    connection: &mut Connection,
    module: usize,
    replica: ReplicaId,
    replay: &mut [Replayed],
    departed: &BTreeMap<EditorId, bool>,
    asked: Vec<(Asked, Option<Waiting>)>,
    waiting: &mut HashMap<OpId, Waiting>,
    replies: &mut HashMap<(usize, OpId), Waiting>,
    pending: &mut HashMap<(usize, OpId), (ReplicaId, Asked)>,
) {
    // An editor that made an edit kept here and has since detached is attached while its edits
    // are given again, as it could edit when it made them: the replica drops what an editor it
    // does not have sends, and the edit, not journaled yet, would be lost.
    for (editor, editable) in departed {
        handle.attach(connection, *editor, *editable);
    }
    for replayed in replay.iter_mut() {
        let sent = replayed.send(handle, connection);
        if let (Replayed::Change { op, .. }, Some(sent)) = (replayed, sent) {
            if let Some(answer) = op.and_then(|op| waiting.remove(&op)) {
                replies.insert((module, sent), answer);
            }
            *op = Some(sent);
        }
    }
    for editor in departed.keys() {
        handle.detach(connection, *editor);
    }
    for (request, answer) in asked {
        let sent = request.send(handle, connection);
        pending.insert((module, sent), (replica, request));
        if let Some(answer) = answer {
            replies.insert((module, sent), answer);
        }
    }
}

struct Open {
    block: Uuid,
    module: usize,
    handle: Replica,
    content_type: Uuid,
    client: ClientId,
    organization: Uuid,
    /// The account this device signs in as (`opening.account`).
    account: Uuid,
    editors: BTreeMap<EditorId, bool>,
    status: Status,
    session: SessionState,
    reader: bool,
    publishing: usize,
    moved: Vec<CommitId>,
    published: Vec<CommitId>,
    /// The head the replica opened at or was last told moved to: news of a block whose head is
    /// still this (another device renamed or shared it) moves nothing.
    heard: Option<CommitId>,
    replay: Vec<Replayed>,
    /// Editors that detached while `replay` holds edits of theirs, and whether they could edit:
    /// what a replica opened again is given their edits as (`resend`).
    departed: BTreeMap<EditorId, bool>,
    closing: bool,
    reopen: bool,
    epoch: u64,
    joining: bool,
    rejoining: Option<u64>,
    made_up: bool,
    early: Vec<Replayed>,
    /// How many local edits (`Replayed::local_edit`) were sent its module since the replica
    /// was opened or opened again, which the module counts too: those past what its status
    /// says it took (`Status::taken`) a status does not yet reflect.
    sent: u64,
    /// How many of those its head and this device's journal on it hold, as its last status
    /// said (`Status::journaled_taken`): a replica opened again with that journal is given
    /// only those past it.
    journaled_taken: u64,
    /// Its last append to this device's journal failed: what its status says that journal
    /// holds may not be there.
    append_failed: bool,
    reporting: Option<CommitId>,
    /// A join's answer held while the replica settles (`Replicas::settling`), handled once it
    /// has.
    held: Option<Io>,
    /// Why its last publish failed, if it did: what a host stuck owing a seal names.
    publish_error: Option<String>,
}

impl Open {
    /// Whether it was sent a local edit its module has not reported on yet.
    fn unreported(&self) -> bool {
        self.sent > self.status.taken
    }

    /// Gives its module again the local edits it holds, as one opened again counts them
    /// from none: those it took before are in what it opens from or in its journal.
    fn resent(&mut self) {
        self.sent = self
            .replay
            .iter()
            .filter(|replayed| replayed.local_edit())
            .count() as u64;
        self.status.taken = 0;
        self.journaled_taken = 0;
    }

    /// Drops what a clean status covers from what would be given a replica opened again: up to
    /// and including the last local edit it took (`Status::taken`), keeping what came after,
    /// which it may not reflect. Once it took them all there is nothing to give again, and what
    /// came between (heads, leases, peers' messages) is had afresh by whatever opens it again.
    fn reported_clean(&mut self, taken: u64) {
        let local = self
            .replay
            .iter()
            .filter(|replayed| replayed.local_edit())
            .count() as u64;
        // The replay holds the last `local` edits sent: those before it were dropped already.
        let covered = taken.saturating_sub(self.sent.saturating_sub(local));
        if covered >= local {
            self.replay.clear();
            self.departed.clear();
            return;
        }
        let mut seen = 0;
        let cut = match covered {
            0 => 0,
            _ => self
                .replay
                .iter()
                .position(|replayed| {
                    seen += u64::from(replayed.local_edit());
                    seen == covered
                })
                .map_or(0, |at| at + 1),
        };
        self.replay.drain(..cut);
    }
}

/// An open block that owes a seal, and what its seal may be waiting on: what a host names when
/// it stays busy sealing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owing {
    pub block: Uuid,
    pub content_type: Uuid,
    /// Why its last publish failed, if it did.
    pub publish_error: Option<String>,
    pub journaled: bool,
    /// Whether an editor sent it something its module has not reported on yet.
    pub unreported: bool,
    /// Publishes of it not yet answered.
    pub publishing: usize,
    pub joining: bool,
    pub closing: bool,
}

impl Owing {
    fn of(open: &Open) -> Self {
        Self {
            block: open.block,
            content_type: open.content_type,
            publish_error: open.publish_error.clone(),
            journaled: open.status.journaled,
            unreported: open.unreported(),
            publishing: open.publishing,
            joining: open.joining,
            closing: open.closing,
        }
    }
}

impl std::fmt::Display for Owing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} of type {} (journaled: {}, unreported: {}, publishes unanswered: {}, joining: {}, closing: {}, last publish failed: {:?})",
            self.block,
            self.content_type,
            self.journaled,
            self.unreported,
            self.publishing,
            self.joining,
            self.closing,
            self.publish_error
        )
    }
}

/// How many commits sealed here `Replicas` remembers per block.
const SEALED_REMEMBERED: usize = 16;

/// The most keys `Replicas::searched` holds before it starts over.
const MOST_SEARCHED_KEYS: usize = 4096;

pub struct Replicas<H: Host> {
    pub modules: Modules,
    answers: UnboundedReceiver<Io>,
    open: HashMap<ReplicaId, Open>,
    blocks: HashMap<Uuid, ReplicaId>,
    next: ReplicaId,
    io: UnboundedSender<Io>,
    handles: HashMap<(usize, OpId), ReplicaId>,
    contents: HashMap<(usize, OpId), (ReplicaId, Content)>,
    replies: HashMap<(usize, OpId), Waiting>,
    asked: HashMap<(usize, OpId), (ReplicaId, Asked)>,
    scopes: HashMap<(usize, OpId), types::Scope>,
    pending: Vec<Io>,
    relaying: usize,
    left: usize,
    /// Per block, events behind a relayed message whose key is being searched for.
    unsealing: HashMap<Uuid, VecDeque<Event>>,
    /// Keys a search is on its way for, which relayed messages wait on.
    searching: HashSet<Uuid>,
    /// Keys a search answered for since a search last opened any or this device last
    /// reconnected or keys arrived (`unlock`): a message still sealed with one is let go.
    searched: HashSet<Uuid>,
    /// Blocks a read here found locked (`H::set_locked`), until keys arrive (`unlock`).
    locked: HashSet<Uuid>,
    /// How many searches for keys relayed messages started.
    #[cfg(test)]
    key_searches: usize,
    /// What peers' relayed messages said, unsealed.
    #[cfg(test)]
    heard_from_peers: Vec<Vec<u8>>,
    leaving: Vec<Uuid>,
    rejoins: u64,
    /// Blocks opened here while their creation waited in the outbox: they join their session
    /// once it is sent (`created`).
    uncreated: HashSet<Uuid>,
    /// Replicas whose join answer waits for them to settle (`Open::held`).
    holding: HashSet<ReplicaId>,
    publishes: HashMap<u64, Unanswered>,
    published: u64,
    keeping: bool,
    kept: bool,
    changed: bool,
    /// Blocks whose next publish is a bookmark, with its label (`Replicas::bookmark`).
    bookmarks: HashMap<Uuid, String>,
    /// The newest commits each block's replicas sealed here, published or kept, across
    /// reopens: a journal rebased on one hands its announcements to it (`Peer::rebase_journal`).
    sealed: HashMap<Uuid, VecDeque<CommitId>>,
    pub handled: u64,
    /// How many heads replicas were told moved, for tests.
    #[cfg(test)]
    pub(crate) heads_told: usize,
    host: std::marker::PhantomData<fn() -> H>,
}

pub type Ctx<'a, H> = (&'a Arc<Peer<<H as Host>::Store>>, &'a Arc<Mutex<H>>);

impl<H: Host> Replicas<H> {
    pub fn new(modules: Modules) -> Self {
        let (io, answers) = unbounded_channel();
        Self {
            modules,
            answers,
            open: HashMap::new(),
            blocks: HashMap::new(),
            next: 0,
            io,
            handles: HashMap::new(),
            contents: HashMap::new(),
            replies: HashMap::new(),
            asked: HashMap::new(),
            scopes: HashMap::new(),
            pending: Vec::new(),
            relaying: 0,
            left: 0,
            unsealing: HashMap::new(),
            searching: HashSet::new(),
            searched: HashSet::new(),
            locked: HashSet::new(),
            #[cfg(test)]
            key_searches: 0,
            #[cfg(test)]
            heard_from_peers: Vec::new(),
            leaving: Vec::new(),
            rejoins: 0,
            uncreated: HashSet::new(),
            holding: HashSet::new(),
            publishes: HashMap::new(),
            published: 0,
            keeping: false,
            kept: false,
            changed: false,
            bookmarks: HashMap::new(),
            sealed: HashMap::new(),
            handled: 0,
            #[cfg(test)]
            heads_told: 0,
            host: std::marker::PhantomData,
        }
    }

    pub fn relaying(&self) -> bool {
        self.relaying > 0
    }

    pub fn leaving(&self) -> bool {
        self.left > 0
    }

    fn leave(&mut self, shared: &Arc<Mutex<H>>, peer: &Peer<H::Store>, block: Uuid) {
        let left = peer.connection().leave_session(block);
        self.left += 1;
        H::leaving(shared, true);
        let io = self.io.clone();
        H::spawn(async move {
            let _ = left.await;
            let _ = io.send(Io::Left);
            H::wake();
        });
    }

    fn relay(&mut self, shared: &Arc<Mutex<H>>, sent: Result<Answer<()>, ClientError>) {
        let answer = match sent {
            Ok(answer) => answer,
            Err(error) if unreachable(&error) => return,
            Err(error) => return H::record(shared, error),
        };
        self.relaying += 1;
        let shared = Arc::clone(shared);
        let io = self.io.clone();
        H::spawn(async move {
            if let Err(error) = answer.await
                && !unreachable(&error)
            {
                H::record(&shared, error);
            }
            let _ = io.send(Io::Relayed);
            H::wake();
        });
    }

    fn journaled(
        &mut self,
        shared: &Arc<Mutex<H>>,
        written: Result<Option<Answer<()>>, ClientError>,
    ) {
        match written {
            Ok(Some(answer)) => self.relay(shared, Ok(answer)),
            Ok(None) => {}
            Err(error) if unreachable(&error) => {}
            Err(error) => H::record(shared, error),
        }
    }

    /// How many local edits the block's replica would be given again were it opened again now.
    #[cfg(test)]
    pub(crate) fn replayed_edits(&self, block: Uuid) -> usize {
        self.replica_of(block).map_or(0, |(_, open)| {
            open.replay
                .iter()
                .filter(|replayed| replayed.local_edit())
                .count()
        })
    }

    /// Waits for an answer to a rejoin and keeps it, with what came before it, for the next
    /// `pump`, which handles them before what the modules said meanwhile.
    #[cfg(test)]
    pub(crate) async fn rejoin_answered(&mut self) {
        while let Some(io) = self.answers.recv().await {
            let rejoined = matches!(io, Io::Rejoined { .. });
            self.pending.push(io);
            if rejoined {
                return;
            }
        }
    }

    pub async fn woken(&mut self) {
        let notified = std::pin::pin!(self.modules.notified());
        let answered = std::pin::pin!(self.answers.recv());
        if let futures_util::future::Either::Right((Some(io), _)) =
            futures_util::future::select(notified, answered).await
        {
            self.pending.push(io);
        }
    }

    pub async fn pump(&mut self, ctx: Ctx<'_, H>) -> bool {
        self.drain(ctx).await;
        self.take_changed()
    }

    /// Whether what the modules said changed anything since this was last asked. A wait on
    /// one module's answer (`ask`, a seal, a search read) handles all they say meanwhile, so a
    /// caller that waited after its `pump` asks this before it settles, or the change goes
    /// unseen until something else wakes it.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    async fn drain(&mut self, ctx: Ctx<'_, H>) {
        loop {
            while let Ok(io) = self.answers.try_recv() {
                self.pending.push(io);
            }
            let answers = std::mem::take(&mut self.pending);
            let heard = self.modules.take();
            for module in self.modules.take_restarted() {
                self.restart(ctx.0, module).await;
                self.changed = true;
            }
            if answers.is_empty() && heard.is_empty() {
                // A restart settles the replicas it reopened (`settling`).
                self.release_held(ctx);
                return;
            }
            self.handled += (answers.len() + heard.len()) as u64;
            for io in answers {
                self.answered(ctx, io);
            }
            for block in std::mem::take(&mut self.leaving) {
                self.leave(ctx.1, ctx.0, block);
            }
            for (module, heard) in heard {
                self.changed |= self.heard_from(ctx, module, heard).await;
            }
            self.release_held(ctx);
        }
    }

    pub async fn ask<T, E: Decode<T>>(
        &mut self,
        ctx: Ctx<'_, H>,
        module: usize,
        call: impl FnOnce(&mut Connection) -> Ticket<T, E>,
    ) -> Result<T, OpError> {
        let Some((ticket, answered)) = self.ask_later(module, call) else {
            return Err(OpError::Canceled);
        };
        self.wait(ctx, ticket, answered).await
    }

    /// `ask` for a stateless request that may `read-part`: of the parts `scope` allows only.
    /// Answers the scope back, holding what was read and the first read that failed. A scope
    /// left behind by a wait that was dropped goes when its request completes or its module
    /// restarts.
    pub async fn ask_reading<T, E: Decode<T>>(
        &mut self,
        ctx: Ctx<'_, H>,
        module: usize,
        scope: types::Scope,
        call: impl FnOnce(&mut Connection) -> Ticket<T, E>,
    ) -> (Result<T, OpError>, types::Scope) {
        let Some((ticket, answered)) = self.ask_later(module, call) else {
            return (Err(OpError::Canceled), scope);
        };
        let key = (module, ticket.op());
        self.scopes.insert(key, scope);
        let answer = self.wait(ctx, ticket, answered).await;
        (answer, self.scopes.remove(&key).unwrap_or_default())
    }

    async fn wait<T, E: Decode<T>>(
        &mut self,
        ctx: Ctx<'_, H>,
        ticket: Ticket<T, E>,
        mut answered: tokio::sync::oneshot::Receiver<Outcome>,
    ) -> Result<T, OpError> {
        let (_, shared) = ctx;
        loop {
            self.drain(ctx).await;
            if let Some(outcome) = answer_in(&mut answered) {
                H::asking(shared, Vec::new());
                return outcome.and_then(|outcome| ticket.decode(outcome));
            }
            H::asking(shared, self.asking());
            self.woken().await;
        }
    }

    /// What is asked of the modules and not yet answered, by block.
    pub(crate) fn asking(&self) -> Vec<String> {
        self.asked
            .values()
            .map(|(replica, asked)| {
                let block = self.open.get(replica).map(|open| open.block);
                let what = match asked {
                    Asked::Content => "its content".to_owned(),
                    Asked::Seal(force) => format!("a seal (forced: {force})"),
                };
                match block {
                    Some(block) => format!("{block}: {what}"),
                    None => format!("a closed replica: {what}"),
                }
            })
            .collect()
    }

    fn ask_later<T, E: Decode<T>>(
        &mut self,
        module: usize,
        call: impl FnOnce(&mut Connection) -> Ticket<T, E>,
    ) -> Option<(Ticket<T, E>, tokio::sync::oneshot::Receiver<Outcome>)> {
        let ticket = self.modules.call(module, call)?;
        let (answer, answered) = tokio::sync::oneshot::channel();
        self.replies.insert((module, ticket.op()), answer);
        Some((ticket, answered))
    }

    /// Whether a module on this device has the type, whether or not it runs.
    pub fn has_module(&self, content_type: Uuid) -> bool {
        self.modules.module_of(content_type).is_some()
    }

    pub async fn kind(&mut self, content_type: Uuid) -> Option<types::Kind> {
        if let Some(module) = self.modules.module_of(content_type) {
            self.modules.load(module).await;
        }
        types::kind_of(content_type)
    }

    pub fn is_open(&self, block: Uuid) -> bool {
        self.blocks.contains_key(&block)
    }

    pub fn open_blocks(&self) -> Vec<Uuid> {
        self.blocks.keys().copied().collect()
    }

    fn replica_of(&self, block: Uuid) -> Option<(ReplicaId, &Open)> {
        let replica = *self.blocks.get(&block)?;
        Some((replica, self.open.get(&replica)?))
    }

    pub async fn open(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        block: Uuid,
        content_type: Uuid,
        reader: bool,
    ) {
        if let Some(replica) = self.blocks.get(&block).copied() {
            let Some(open) = self.open.get_mut(&replica) else {
                return;
            };
            open.closing = false;
            if reader && !open.reader {
                open.reader = true;
                self.attach_now(replica, HOST_EDITOR, true);
            }
            return;
        }
        let Some(kind) = self.kind(content_type).await else {
            return;
        };
        let arrivals = peer.arrivals();
        let owned = !kind.frozen && H::owned(shared, block);
        let known = peer
            .known_summary(block)
            .is_some_and(|summary| summary.content_type == content_type);
        let uncreated = cannot_join(peer, block);
        let joined = if known
            && peer.is_attached()
            && !uncreated
            && !(owned && H::reshaped(shared, block, &kind.schema))
        {
            let joining = joining(peer, block, content_type);
            let waited = self.modules.clock().sleep(EARLY_OPEN);
            match futures_util::future::select(joining, waited).await {
                futures_util::future::Either::Left((joined, _)) => {
                    joined.map(|(client, session, head, _, journal)| {
                        let journal = journal.map(|(_, opened)| opened);
                        (client, session, head, false, journal)
                    })
                }
                futures_util::future::Either::Right((_, joining)) => {
                    let head = known_head_of(peer, block);
                    if let Some(replica) = self.start(
                        peer,
                        block,
                        content_type,
                        reader,
                        OFFLINE_CLIENT,
                        joining_session(head),
                        head,
                        None,
                        false,
                        None,
                    ) {
                        self.join_later(replica, block, joining, arrivals);
                    }
                    return;
                }
            }
        } else {
            async {
                if peer.known_summary(block).is_none() || peer.is_attached() {
                    peer.ensure_type(block, content_type, BlockParent::Detached)
                        .await?;
                }
                // Until the drain sends its creation the server does not know the block, and
                // without access to it the server refuses its session: it opens as offline does
                // and joins once that changes (`created`).
                if !peer.is_attached() || uncreated {
                    return Ok((OFFLINE_CLIENT, offline_session(None), None, true, None));
                }
                if owned && H::reshaped(shared, block, &kind.schema) {
                    reset::<H>(peer, block, &kind).await?;
                }
                let (client, session, journal) = peer.connection().join_session(block).await?;
                // Reading the head lists the block, so `changes` tells its next ones.
                let head = peer.remote_head(block).await?;
                let journal = journal.and_then(|journal| peer.open_journal(journal).ok());
                Ok::<_, ClientError>((client, session, head, false, journal))
            }
            .await
        };
        let joined = match joined {
            Err(error) if unreachable(&error) && peer.known_summary(block).is_some() => {
                Ok((OFFLINE_CLIENT, offline_session(None), None, true, None))
            }
            joined => joined,
        };
        let (client, session, head, offline, journal) = match joined {
            Ok((client, _, _, true, _)) => {
                if uncreated {
                    self.uncreated.insert(block);
                }
                let head = known_head_of(peer, block);
                if head.is_some() && peer.held_head(block).is_none() {
                    // Nothing to read: the block opens once the host reconnects, and meanwhile
                    // takes no edits it could never journal or publish.
                    return;
                }
                (client, offline_session(head), head, true, None)
            }
            Ok(joined) => joined,
            Err(ClientError::Locked(_)) if peer.arrivals() == arrivals => {
                self.lock(shared, block);
                return;
            }
            // Defensive: keys arrived while it opened, after `unlock` ran, so it opens again.
            // `keys_changed` runs on the worker's loop, which this open holds, so the count
            // does not move while it runs today.
            Err(ClientError::Locked(_)) => {
                return Box::pin(self.open((peer, shared), block, content_type, reader)).await;
            }
            Err(error) => {
                H::record(shared, error);
                return;
            }
        };
        let remote = head;
        let head = match offline {
            true => head,
            false => ahead(peer, block).or(head),
        };
        let (journal, journal_kept, load_at, sealed_head) = journal_for(peer, block, journal, head);
        let Some(replica) = self.start(
            peer,
            block,
            content_type,
            reader,
            client,
            session,
            load_at,
            journal,
            journal_kept,
            sealed_head,
        ) else {
            return;
        };
        if let Some(open) = self.open.get_mut(&replica) {
            open.made_up = offline;
        }
        if offline {
            return;
        }
        let mut heads = vec![remote, H::known_head(shared, block)];
        heads.dedup();
        for moved in heads.into_iter().flatten() {
            if Some(moved) != head {
                self.moved(replica, moved, false);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        peer: &Arc<Peer<H::Store>>,
        block: Uuid,
        content_type: Uuid,
        reader: bool,
        client: ClientId,
        session: SessionState,
        head: Option<CommitId>,
        journal: Option<be_block_protocol::Journal>,
        journal_kept: bool,
        sealed_head: Option<be_sdk_type_api::SealedHead>,
    ) -> Option<ReplicaId> {
        if self.blocks.contains_key(&block) {
            return None;
        }
        let module = self.modules.module_of(content_type)?;
        self.next += 1;
        let replica = self.next;
        let organization = peer.organization();
        let joining = session.owner == Some(JOINING_OWNER);
        let tag = self.modules.tag(module);
        let kept = kept_commits(peer, block);
        let opening = Opening {
            block,
            content_type,
            organization,
            account: peer.account(),
            client,
            tag,
            head,
            session: session.clone(),
            lost_edits: lost_edits(journal.as_ref(), tag),
            journal: journal.and_then(|journal| replayable(journal, tag)),
            journal_kept,
            commits_kept: kept.0,
            refused_commits: kept.1,
            server_head: peer.known_remote_head(block),
            sealed_head,
            epoch: 0,
        };
        let handle = self
            .modules
            .call(module, |connection| Replica::new(connection, opening))?;
        self.blocks.insert(block, replica);
        self.handles.insert((module, handle.op()), replica);
        self.open.insert(
            replica,
            Open {
                block,
                module,
                handle,
                content_type,
                client,
                organization,
                account: peer.account(),
                editors: BTreeMap::new(),
                status: Status {
                    head,
                    ..Status::default()
                },
                session,
                reader,
                publishing: 0,
                moved: Vec::new(),
                published: Vec::new(),
                heard: head,
                replay: Vec::new(),
                departed: BTreeMap::new(),
                closing: false,
                reopen: false,
                epoch: 0,
                joining,
                rejoining: None,
                made_up: joining,
                early: Vec::new(),
                sent: 0,
                journaled_taken: 0,
                append_failed: false,
                reporting: None,
                held: None,
                publish_error: None,
            },
        );
        if reader {
            self.attach_now(replica, HOST_EDITOR, true);
        }
        Some(replica)
    }

    fn join_later(
        &mut self,
        replica: ReplicaId,
        block: Uuid,
        joining: Pending<Joined>,
        arrivals: u64,
    ) {
        let io = self.io.clone();
        H::spawn(async move {
            let joined = joining.await;
            let _ = io.send(Io::Joined {
                replica,
                block,
                joined: Box::new(joined),
                arrivals,
            });
            H::wake();
        });
    }

    pub fn joining(&self) -> bool {
        self.open.values().any(|open| open.joining)
    }

    fn joined(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        replica: ReplicaId,
        block: Uuid,
        joined: Joined,
        arrivals: u64,
    ) {
        let Some(open) = self.open.get_mut(&replica) else {
            if joined.is_ok() && !self.blocks.contains_key(&block) {
                self.leaving.push(block);
            }
            return;
        };
        if !open.joining || open.rejoining.is_some() {
            return;
        }
        if self.settling(replica) {
            return self.hold(
                replica,
                Io::Joined {
                    replica,
                    block,
                    joined: Box::new(joined),
                    arrivals,
                },
            );
        }
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        let block = open.block;
        let (client, session, head, ahead, journal) = match joined {
            Ok(joined) => joined,
            Err(error) if unreachable(&error) => return,
            Err(error) => {
                let locked = matches!(error, ClientError::Locked(_));
                let keyed = locked && peer.arrivals() != arrivals;
                if locked && !keyed {
                    self.locked.insert(block);
                    H::set_locked(shared, block, true);
                } else if !locked {
                    H::record(shared, error);
                }
                open.joining = false;
                self.make_up_session(replica);
                self.close_if_asked(replica);
                // Keys arrived while it joined, after `unlock` ran: it joins again.
                if keyed {
                    self.read_again(block);
                }
                return;
            }
        };
        open.joining = false;
        open.made_up = false;
        let edited = open.replay.iter().any(Replayed::local_edit);
        let generation = session.generation;
        let early: Vec<Replayed> = std::mem::take(&mut open.early)
            .into_iter()
            .filter(|replayed| match replayed {
                Replayed::Lease { session, .. } => session.generation >= generation,
                _ => true,
            })
            .collect();
        let from = match edited || ahead.is_some() {
            true => open.status.head,
            false => head,
        };
        let journal = reopening(open, peer, journal, from);
        let commits_kept = match ahead.is_some() {
            true => (be_sdk_type_api::KeptCommits::Outbox, Vec::new()),
            false => (be_sdk_type_api::KeptCommits::None, Vec::new()),
        };
        self.reopen_joined(replica, client, session, from, head, journal, commits_kept);
        for replayed in early {
            self.replay(replica, replayed);
        }
        let mut heads = vec![head, H::known_head(shared, block)];
        heads.dedup();
        for moved in heads.into_iter().flatten() {
            if Some(moved) != from {
                self.moved(replica, moved, false);
            }
        }
        self.close_if_asked(replica);
    }

    fn reopen_joined(
        &mut self,
        replica: ReplicaId,
        client: ClientId,
        session: SessionState,
        head: Option<CommitId>,
        server_head: Option<CommitId>,
        (journal, extras_lost): (Option<be_block_protocol::Journal>, bool),
        commits_kept: Kept,
    ) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.client = client;
        open.session = session.clone();
        open.epoch += 1;
        open.publishing = 0;
        open.moved.clear();
        open.published.clear();
        open.heard = head;
        open.reporting = None;
        open.status.head = head;
        open.replay.retain(Replayed::local_edit);
        open.resent();
        let (module, handle) = (open.module, open.handle);
        let opening = Opening {
            block: open.block,
            content_type: open.content_type,
            organization: open.organization,
            account: open.account,
            client,
            tag: self.modules.tag(module),
            head,
            session,
            lost_edits: extras_lost || lost_edits(journal.as_ref(), self.modules.tag(module)),
            journal: journal.and_then(|journal| replayable(journal, self.modules.tag(module))),
            journal_kept: false,
            commits_kept: commits_kept.0,
            refused_commits: commits_kept.1,
            server_head,
            sealed_head: None,
            epoch: open.epoch,
        };
        let editors: Vec<(EditorId, bool)> = open
            .editors
            .iter()
            .map(|(editor, editable)| (*editor, *editable))
            .collect();
        let mut asked = Vec::new();
        let mut waiting = HashMap::new();
        for key in self
            .asked
            .iter()
            .filter(|(_, (to, _))| *to == replica)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>()
        {
            if let Some((_, request)) = self.asked.remove(&key) {
                asked.push((request, self.replies.remove(&key)));
            }
        }
        for replayed in &open.replay {
            if let Replayed::Change { op: Some(op), .. } = replayed
                && let Some(answer) = self.replies.remove(&(module, *op))
            {
                waiting.insert(*op, answer);
            }
        }
        let (replies, pending, replay, departed) = (
            &mut self.replies,
            &mut self.asked,
            &mut open.replay,
            &open.departed,
        );
        self.modules.call(module, |connection| {
            handle.reopen(connection, opening);
            for (editor, editable) in editors {
                handle.attach(connection, editor, editable);
            }
            resend(
                handle,
                connection,
                module,
                replica,
                replay,
                departed,
                asked,
                &mut waiting,
                replies,
                pending,
            );
        });
    }

    fn make_up_session(&mut self, replica: ReplicaId) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.made_up = true;
        open.session = SessionState {
            generation: open.session.generation + 1,
            owner: Some(open.client),
            participants: vec![open.client],
            clean_at: None,
        };
        let session = open.session.clone();
        self.with_replica(replica, |handle, connection| {
            handle.lease(connection, session, false);
        });
    }

    fn close_if_asked(&mut self, replica: ReplicaId) {
        let Some(open) = self.open.get(&replica) else {
            return;
        };
        if open.closing && !open.joining {
            self.with_replica(replica, |handle, connection| {
                handle.resource_drop(connection)
            });
        }
    }

    fn attach_now(&mut self, replica: ReplicaId, editor: EditorId, editable: bool) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.editors.insert(editor, editable);
        open.departed.remove(&editor);
        let (module, handle) = (open.module, open.handle);
        self.modules.call(module, |connection| {
            handle.attach(connection, editor, editable);
        });
    }

    fn with_replica(&mut self, replica: ReplicaId, call: impl FnOnce(Replica, &mut Connection)) {
        if let Some(open) = self.open.get(&replica) {
            let (module, handle) = (open.module, open.handle);
            self.modules
                .call(module, |connection| call(handle, connection));
        }
    }

    fn replay(&mut self, replica: ReplicaId, replayed: Replayed) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        #[cfg(test)]
        if let Replayed::Moved { .. } = replayed {
            self.heads_told += 1;
        }
        open.replay.push(replayed.clone());
        let (module, handle) = (open.module, open.handle);
        let sent = self
            .modules
            .call(module, |connection| {
                replayed.send(handle, connection);
            })
            .is_some();
        // Counted once its module has it, as `change_now` counts a change (`Open::sent`).
        if sent
            && replayed.local_edit()
            && let Some(open) = self.open.get_mut(&replica)
        {
            open.sent += 1;
        }
    }

    pub fn attach(&mut self, block: Uuid, editor: EditorId, editable: bool) {
        let Some((replica, open)) = self.replica_of(block) else {
            return;
        };
        if open.editors.get(&editor) == Some(&editable) {
            return;
        }
        self.attach_now(replica, editor, editable);
    }

    pub fn detach(&mut self, block: Uuid, editor: EditorId) -> bool {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return false;
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return false;
        };
        let Some(editable) = open.editors.remove(&editor) else {
            return open.editors.is_empty();
        };
        let kept = open.replay.iter().any(|replayed| {
            matches!(replayed, Replayed::Message { from: Endpoint::Editor(from), .. } if *from == editor)
        });
        if kept {
            open.departed.insert(editor, editable);
        }
        let idle = open.editors.is_empty();
        self.with_replica(replica, |handle, connection| {
            handle.detach(connection, editor);
        });
        idle
    }

    pub fn relay_from_editor(
        &mut self,
        block: Uuid,
        editor: EditorId,
        bytes: Vec<u8>,
        at: Instant,
    ) {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return;
        };
        let Some(open) = self.open.get(&replica) else {
            return;
        };
        self.modules.tick_at(open.module, at);
        let at = self.modules.millis_at(at);
        self.replay(
            replica,
            Replayed::Message {
                from: Endpoint::Editor(editor),
                bytes,
                at,
                delivery: None,
            },
        );
    }

    pub fn send_as_host(&mut self, block: Uuid, message: &FromEditor, at: Instant) {
        if let Some((replica, open)) = self.replica_of(block)
            && !open.editors.contains_key(&HOST_EDITOR)
        {
            self.attach_now(replica, HOST_EDITOR, true);
        }
        self.relay_from_editor(block, HOST_EDITOR, message.encode(), at);
    }

    pub fn close(&mut self, block: Uuid) {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return;
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        if std::mem::replace(&mut open.closing, true) || open.joining {
            return;
        }
        self.with_replica(replica, |handle, connection| {
            handle.resource_drop(connection)
        });
    }

    fn lock(&mut self, shared: &Arc<Mutex<H>>, block: Uuid) {
        self.locked.insert(block);
        H::set_locked(shared, block, true);
    }

    /// Keys this account may open arrived (`Event::KeysChanged`, after `Peer::keys_changed`):
    /// every block found locked is no longer, an open one reads again (`read_again`), and
    /// relayed messages search for their keys again. Answers the locked blocks that are not
    /// open, for the host to open again if it still wants them.
    pub fn unlock(&mut self, shared: &Arc<Mutex<H>>) -> Vec<Uuid> {
        self.searched.clear();
        let mut closed = Vec::new();
        for block in std::mem::take(&mut self.locked) {
            H::set_locked(shared, block, false);
            match self.blocks.contains_key(&block) {
                true => self.read_again(block),
                false => closed.push(block),
            }
        }
        closed
    }

    /// Opens a replica again to read what a missing key kept from it, if that loses nothing:
    /// every edit it holds is journaled (`journal_for` replays it as it opens again) or it
    /// holds none, and it either never joined its session or holds nothing unsealed. Opening
    /// again drops the instance, so any other keeps it instead, and its next read asks for the
    /// key again. One the host is closing stays closed.
    fn read_again(&mut self, block: Uuid) {
        let Some((_, open)) = self.replica_of(block) else {
            return;
        };
        let durable = !open.unreported() && (open.status.clean || open.status.journaled);
        let settled = open.status.clean
            && !open.status.owes_seal
            && !open.replay.iter().any(Replayed::local_edit);
        if !open.closing && durable && (open.made_up || settled) {
            self.reopen(block);
        }
    }

    pub fn reopen(&mut self, block: Uuid) {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return;
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.reopen = true;
        if std::mem::replace(&mut open.closing, true) || open.joining {
            return;
        }
        self.with_replica(replica, |handle, connection| {
            handle.resource_drop(connection)
        });
    }

    pub fn moved_here(&mut self, block: Uuid, head: CommitId) {
        if let Some(replica) = self.blocks.get(&block).copied() {
            self.moved(replica, head, true);
        }
    }

    fn moved(&mut self, replica: ReplicaId, head: CommitId, from_this_device: bool) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        if open.publishing > 0 && !from_this_device {
            open.moved.push(head);
            return;
        }
        open.heard = Some(head);
        if let Some(index) = open.published.iter().position(|held| *held == head) {
            open.published.drain(..=index);
            return;
        }
        self.replay(replica, Replayed::Moved { head });
    }

    async fn on_replica<T, E: Decode<T>>(
        &mut self,
        ctx: Ctx<'_, H>,
        block: Uuid,
        asked: Asked,
        _: impl FnOnce(Replica, &mut Connection) -> Ticket<T, E>,
    ) -> Result<T, OpError> {
        let Some((replica, open)) = self.replica_of(block) else {
            return Err(OpError::Canceled);
        };
        let (module, handle) = (open.module, open.handle);
        let Some((ticket, answered)) = self.ask_later(module, |connection| {
            Ticket::<T, E>::new(asked.send(handle, connection))
        }) else {
            return Err(OpError::Canceled);
        };
        self.asked.insert((module, ticket.op()), (replica, asked));
        self.wait(ctx, ticket, answered).await
    }

    fn change_now(&mut self, block: Uuid, change: Change) -> Option<(usize, OpId)> {
        let (replica, open) = self.replica_of(block)?;
        let (module, handle) = (open.module, open.handle);
        let replayed = Replayed::Change { change, op: None };
        let op = self
            .modules
            .call(module, |connection| replayed.send(handle, connection))
            .flatten()?;
        let open = self.open.get_mut(&replica)?;
        open.sent += 1;
        open.replay.push(match replayed {
            Replayed::Change { change, .. } => Replayed::Change {
                change,
                op: Some(op),
            },
            replayed => replayed,
        });
        Some((module, op))
    }

    pub fn change_later(&mut self, block: Uuid, change: Change) {
        self.change_now(block, change);
    }

    pub async fn content(&mut self, ctx: Ctx<'_, H>, block: Uuid) -> Option<Vec<u8>> {
        self.on_replica(ctx, block, Asked::Content, Replica::content)
            .await
            .ok()
            .flatten()
    }

    pub async fn change(&mut self, ctx: Ctx<'_, H>, block: Uuid, change: Change) -> bool {
        let Some((module, op)) = self.change_now(block, change) else {
            return false;
        };
        let (answer, answered) = tokio::sync::oneshot::channel();
        self.replies.insert((module, op), answer);
        self.wait(ctx, Ticket::<bool>::new(op), answered)
            .await
            .unwrap_or(false)
    }

    pub async fn seal(&mut self, ctx: Ctx<'_, H>, block: Uuid, force: bool) {
        if self.ready_to_seal(block) {
            let _ = self
                .on_replica(ctx, block, Asked::Seal(force), |replica, connection| {
                    replica.seal(connection, force)
                })
                .await;
        }
    }

    /// Seals the block's current state as a bookmark, which history keeps whatever its age,
    /// even when it is already saved: restoring a revision keeps the state it replaces this way.
    pub async fn bookmark(&mut self, ctx: Ctx<'_, H>, block: Uuid, label: &str) {
        self.bookmarks.insert(block, label.to_owned());
        self.seal(ctx, block, true).await;
        self.bookmarks.remove(&block);
    }

    pub fn seal_later(&mut self, block: Uuid, force: bool) {
        if !self.ready_to_seal(block) {
            return;
        }
        let Some((replica, open)) = self.replica_of(block) else {
            return;
        };
        let (module, handle) = (open.module, open.handle);
        let asked = Asked::Seal(force);
        if let Some(op) = self
            .modules
            .call(module, |connection| asked.send(handle, connection))
        {
            self.asked.insert((module, op), (replica, asked));
        }
    }

    fn ready_to_seal(&mut self, block: Uuid) -> bool {
        if let Some((replica, open)) = self.replica_of(block)
            && open.joining
            && open.session.owner == Some(JOINING_OWNER)
        {
            if open.status.clean {
                return false;
            }
            self.make_up_session(replica);
        }
        true
    }

    pub async fn seal_all(&mut self, ctx: Ctx<'_, H>) {
        for block in self.open_blocks() {
            self.seal(ctx, block, false).await;
        }
    }

    pub async fn keep_all(&mut self, ctx: Ctx<'_, H>) {
        self.keeping = true;
        self.release();
        self.seal_all(ctx).await;
        self.keeping = false;
    }

    pub fn reported(&self) -> bool {
        self.open.values().all(|open| open.reporting.is_none())
    }

    /// The server holds every commit this device kept of `block`, or with `refused` refused
    /// the `commits` listed (`refused`): its replica tells its editors what they sent is held, or will not be.
    pub fn drained(&mut self, block: Uuid, refused: bool, commits: Vec<CommitId>) {
        if let Some(replica) = self.blocks.get(&block).copied() {
            self.with_replica(replica, |handle, connection| {
                handle.drained(connection, refused, commits);
            });
        }
    }

    pub fn take_kept(&mut self) -> bool {
        std::mem::take(&mut self.kept)
    }

    fn release(&mut self) {
        if !self.keeping {
            return;
        }
        let ready: Vec<(u64, CommitId)> = self
            .publishes
            .iter()
            .filter_map(|(publish, unanswered)| Some((*publish, unanswered.publishing.kept()?)))
            .collect();
        for (publish, commit) in ready {
            if !self.publishes[&publish].publishing.release() {
                continue;
            }
            if let Some(unanswered) = self.publishes.remove(&publish) {
                self.kept = true;
                self.answer_publish(unanswered, Ok(PublishOutcome::Kept(commit)), None);
            }
        }
    }

    fn sealed_here(&mut self, block: Uuid, head: CommitId) {
        let sealed = self.sealed.entry(block).or_default();
        if !sealed.contains(&head) {
            sealed.push_back(head);
        }
        while sealed.len() > SEALED_REMEMBERED {
            sealed.pop_front();
        }
    }

    fn answer_publish(
        &mut self,
        unanswered: Unanswered,
        result: Result<PublishOutcome, String>,
        published: Option<CommitId>,
    ) {
        let Unanswered {
            replica,
            module,
            generation,
            epoch,
            reply,
            ..
        } = unanswered;
        let delivered = self.modules.generation(module) == generation;
        if delivered {
            self.modules
                .call(module, |connection| reply.send(connection, &result));
        }
        if let Ok(PublishOutcome::Published(head) | PublishOutcome::Kept(head)) = result
            && let Some(block) = self.open.get(&replica).map(|open| open.block)
        {
            self.sealed_here(block, head);
        }
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.publish_error = result.as_ref().err().cloned();
        if open.epoch != epoch {
            return;
        }
        if delivered
            && let Ok(PublishOutcome::Published(head) | PublishOutcome::Kept(head)) = result
            && open.status.head != Some(head)
        {
            open.reporting = Some(head);
        }
        open.publishing = open.publishing.saturating_sub(1);
        if let Some(head) = published {
            open.published.push(head);
        }
        if open.publishing == 0 {
            for head in std::mem::take(&mut open.moved) {
                self.moved(replica, head, false);
            }
        }
    }

    /// Merges three stored forms of a block (none is the type's default) as its type does:
    /// by parts, loading only each side's first part, for a type that merges by parts, and
    /// otherwise over whole bytes. None when the type has no module or the merge failed.
    pub async fn merge(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        sides: [Option<&be_block_store::Manifest>; 3],
    ) -> Result<Option<Merged>, ClientError> {
        let peer = ctx.0;
        let Some(kind) = self.kind(content_type).await else {
            return Ok(None);
        };
        let Some(module) = self.modules.module_of(content_type) else {
            return Ok(None);
        };
        let organization = peer.organization();
        if kind.merges_parts {
            let mut loaded = Vec::new();
            for side in sides {
                loaded.push(match side {
                    Some(manifest) => Some(kind.load(peer, manifest).await?),
                    None => None,
                });
            }
            let scope = types::Scope::of(loaded.iter().flatten());
            let [base, ours, theirs]: [_; 3] = loaded.try_into().unwrap_or_default();
            let (answer, mut scope) = self
                .ask_reading(ctx, module, scope, |connection| {
                    replicas::merge_parts(
                        connection,
                        content_type,
                        base,
                        ours,
                        theirs,
                        organization,
                    )
                })
                .await;
            if let Some(failed) = scope.take_failure() {
                return Err(failed);
            }
            return Ok(match answer {
                Ok(Ok(merged)) if scope.admits(&merged.content) => Some(Merged {
                    content: merged.content,
                    conflicts: merged.conflicts as usize,
                    read: scope.read().to_vec(),
                }),
                _ => None,
            });
        }
        let mut bytes = Vec::new();
        for side in sides {
            bytes.push(match side {
                Some(manifest) => Some(peer.read_manifest(manifest).await?),
                None => None,
            });
        }
        let [base, ours, theirs]: [_; 3] = bytes.try_into().unwrap_or_default();
        let merged = match self
            .ask(ctx, module, |connection| {
                replicas::merge3(connection, content_type, base, ours, theirs)
            })
            .await
        {
            Ok(Ok(merged)) => merged,
            _ => return Ok(None),
        };
        let Some(description) = self.describe(ctx, content_type, &merged.bytes).await else {
            return Ok(None);
        };
        Ok(Some(Merged {
            content: be_sdk_type_api::Written {
                bytes: merged.bytes,
                parts: Vec::new(),
                references: description.references,
                derived: description.derived,
            },
            conflicts: merged.conflicts as usize,
            read: Vec::new(),
        }))
    }

    /// A commit's searchable text (`be_block_content::BlockContent::search_text`), as the indexer can
    /// trust it whatever the type answered: only the documents `keys` names (none for every
    /// one), each key once, a `source` only if it is a part of this commit, and no body for a
    /// document whose source is in `indexed`, the hashes of parts whose text the index holds
    /// (those that are parts of this commit). The type is handed the commit as it loads it and
    /// reads only parts of it.
    pub async fn search_text(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        manifest: &be_block_store::Manifest,
        keys: Option<Vec<String>>,
        indexed: &[[u8; 32]],
    ) -> Result<Searched, NotSearched> {
        let peer = ctx.0;
        let no_module =
            || NotSearched::Type(format!("no type module has the block type {content_type}"));
        let kind = self.kind(content_type).await.ok_or_else(no_module)?;
        let module = self.modules.module_of(content_type).ok_or_else(no_module)?;
        let content = kind.load(peer, manifest).await.map_err(NotSearched::Read)?;
        let scope = types::Scope::of([&content]);
        let asked = keys.clone();
        let indexed = scope.with_hashes(indexed);
        let wire_indexed = indexed.iter().copied().map(Into::into).collect();
        let (answer, mut scope) = self
            .ask_reading(ctx, module, scope, |connection| {
                replicas::search_text(connection, content_type, content, asked, wire_indexed)
            })
            .await;
        if let Some(failed) = scope.take_failure() {
            return Err(NotSearched::Read(failed));
        }
        let documents = match answer {
            Ok(Ok(documents)) => documents,
            Ok(Err(be_sdk_type_api::Failure::Malformed(error))) => {
                return Err(NotSearched::Type(error));
            }
            Ok(Err(be_sdk_type_api::Failure::UnknownType)) => return Err(no_module()),
            Err(error) => {
                return Err(NotSearched::Type(format!(
                    "the type module answered: {error}"
                )));
            }
        };
        let keys: Option<HashSet<String>> = keys.map(|keys| keys.into_iter().collect());
        let indexed: HashSet<be_block_store::PartRef> = indexed.iter().copied().collect();
        let mut seen = HashSet::new();
        let mut kept = Vec::with_capacity(documents.len());
        for mut document in documents {
            if keys
                .as_ref()
                .is_some_and(|keys| !keys.contains(&document.key))
            {
                continue;
            }
            if !seen.insert(document.key.clone()) {
                return Err(NotSearched::Type(format!(
                    "the type answered the document {:?} twice",
                    document.key
                )));
            }
            let source = document
                .source
                .map(be_block_store::PartRef::from)
                .filter(|source| scope.allows(*source));
            if source.is_some_and(|source| indexed.contains(&source)) {
                document.body = None;
            } else if document.body.is_none() {
                return Err(NotSearched::Type(format!(
                    "the type answered no text for the document {:?}",
                    document.key
                )));
            }
            document.source = source.map(Into::into);
            kept.push(document);
        }
        Ok(Searched {
            documents: kept,
            read: scope.read().to_vec(),
        })
    }

    /// Merges a block the drain found diverged (`Drained::Diverged`) and publishes the merge:
    /// the server's head after it, or none when nothing reached the server (no type merges the
    /// block, or the merge waits in the outbox, `Saved::Kept`, for a drain this asks for).
    pub async fn merge_diverged(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        block: Uuid,
        remote: Option<CommitId>,
    ) -> Result<Option<CommitId>, ClientError> {
        let Some(content_type) = peer
            .known_summary(block)
            .map(|summary| summary.content_type)
        else {
            return Ok(None);
        };
        let mut remote = remote;
        for _ in 0..4 {
            let (base, ours, theirs) = match peer.reconcile(block, remote).await? {
                Reconciled::Done => return Ok(peer.known_remote_head(block)),
                Reconciled::Merge { base, ours, theirs } => (base, ours, theirs),
            };
            let mut manifests = Vec::new();
            for commit in [base, Some(ours), Some(theirs)] {
                manifests.push(match commit {
                    Some(commit) => Some(peer.load_commit(commit).await?.manifest),
                    None => None,
                });
            }
            let sides = [&manifests[0], &manifests[1], &manifests[2]].map(Option::as_ref);
            let Some(merged) = self.merge((peer, shared), content_type, sides).await? else {
                return Ok(None);
            };
            let conflicts = merged.conflicts;
            let saved = peer
                .write(
                    block,
                    Write::encoded(content_type, merged.parts(), Some(theirs))
                        .described(
                            merged.content.references.clone(),
                            merged.content.derived.clone(),
                        )
                        .merging(base, ours),
                )
                .await?;
            if conflicts > 0 && !matches!(saved, Saved::Rejected { .. }) {
                H::conflict(
                    shared,
                    block,
                    crate::Conflict {
                        ours,
                        theirs,
                        count: conflicts,
                    },
                );
            }
            match saved {
                Saved::Published(head) | Saved::Unchanged(head) => remote = Some(head),
                // The merge waits in the outbox behind what goes first, and the server does not
                // have it: the next drain sends them in order.
                Saved::Kept(_) => {
                    self.kept = true;
                    return Ok(None);
                }
                Saved::Rejected { head } => remote = head,
            }
        }
        Err(ClientError::Contended)
    }

    pub async fn describe(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        bytes: &[u8],
    ) -> Option<Description> {
        let module = self.modules.module_of(content_type)?;
        let (bytes, organization) = (bytes.to_vec(), ctx.0.organization());
        match self
            .ask(ctx, module, |connection| {
                replicas::describe(connection, content_type, bytes, organization)
            })
            .await
        {
            Ok(Ok(description)) => Some(description),
            _ => None,
        }
    }

    pub async fn to_file(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        bytes: Vec<u8>,
    ) -> Result<Option<PlainFile>, String> {
        let module = self
            .modules
            .module_of(content_type)
            .ok_or_else(|| format!("no type module has the block type {content_type}"))?;
        match self
            .ask(ctx, module, |connection| {
                replicas::to_file(connection, content_type, bytes)
            })
            .await
        {
            Ok(Ok(file)) => Ok(file),
            Ok(Err(be_sdk_type_api::Failure::Malformed(error))) => Err(error),
            Ok(Err(be_sdk_type_api::Failure::UnknownType)) => {
                Err(format!("no type module has the block type {content_type}"))
            }
            Err(error) => Err(format!("the type module answered: {error}")),
        }
    }

    pub async fn from_file(
        &mut self,
        ctx: Ctx<'_, H>,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<(Uuid, Vec<u8>), String> {
        let mut claimed = None;
        for (module, claim) in self.modules.claim_requests(name, &bytes, false) {
            if let Ok(Some(content_type)) = self
                .ask(ctx, module, |connection| claim.call(connection))
                .await
            {
                claimed = Some(content_type);
                break;
            }
        }
        let content_type = claimed.ok_or_else(|| format!("No block type takes {name}."))?;
        let change = Change::FromFile(be_sdk_type_api::FileChange {
            name: name.to_owned(),
            bytes,
        });
        let content = self
            .apply(ctx, content_type, None, 0, change)
            .await?
            .ok_or_else(|| format!("{name} has no block form."))?;
        Ok((content_type, content))
    }

    pub async fn apply(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        client: ClientId,
        change: Change,
    ) -> Result<Option<Vec<u8>>, String> {
        let module = self
            .modules
            .module_of(content_type)
            .ok_or_else(|| format!("no type module has the block type {content_type}"))?;
        match self
            .ask(ctx, module, |connection| {
                replicas::apply(connection, content_type, bytes, client, change)
            })
            .await
        {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(be_sdk_type_api::Failure::Malformed(error))) => Err(error),
            Ok(Err(be_sdk_type_api::Failure::UnknownType)) => {
                Err(format!("no type module has the block type {content_type}"))
            }
            Err(error) => Err(format!("the type module answered: {error}")),
        }
    }

    /// Whether the block's open replica holds nothing unsealed: clean, owing no seal, and
    /// reported on every local edit it was sent, which its last status may predate.
    pub fn settled(&self, block: Uuid) -> bool {
        self.replica_of(block).is_none_or(|(_, open)| {
            open.status.clean && !open.status.owes_seal && !open.unreported()
        })
    }

    pub fn status(&self, block: Uuid) -> Option<Status> {
        self.replica_of(block).map(|(_, open)| open.status.clone())
    }

    pub fn session(&self, block: Uuid) -> Option<(ClientId, &SessionState)> {
        self.replica_of(block)
            .map(|(_, open)| (open.client, &open.session))
    }

    pub fn claim(&mut self, block: Uuid) {
        if let Some((replica, _)) = self.replica_of(block) {
            self.with_replica(replica, |handle, connection| {
                handle.claim(connection);
            });
        }
    }

    /// Whether what peers sent about `block` waits for keys to unseal it.
    #[cfg(test)]
    pub(crate) fn waits_for_keys(&self, block: Uuid) -> bool {
        self.unsealing.contains_key(&block)
    }

    #[cfg(test)]
    pub(crate) fn key_searches(&self) -> usize {
        self.key_searches
    }

    #[cfg(test)]
    pub(crate) fn heard_from_peers(&self) -> &[Vec<u8>] {
        &self.heard_from_peers
    }

    /// Takes what the session told this device about an open block. A relayed message sealed
    /// with a key this device does not hold waits, with the events behind it, for one search for
    /// that key: a peer seals with a block's key as soon as it has it, and a search started
    /// elsewhere (`Peer::learn`) may answer only after the message arrived. Once a search for its
    /// key has answered, a message still sealed with it is let go and recorded.
    pub fn heard(&mut self, (peer, shared): Ctx<'_, H>, event: &Event) {
        let block = match event {
            Event::SessionChanged { block, .. } | Event::Relayed { block, .. } => block,
            // A head another connection published; this connection's own publishes are the
            // replica's, which it heard of as it made them.
            Event::BlockChanged { block, own: false } if block.head.is_some() => &block.id,
            _ => return,
        };
        if let Some(waiting) = self.unsealing.get_mut(block) {
            waiting.push_back(event.clone());
            return;
        }
        match event {
            Event::SessionChanged { block, state } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                let replayed = Replayed::Lease {
                    session: state.clone(),
                    resynchronize: false,
                };
                if open.joining {
                    return open.early.push(replayed);
                }
                open.session = state.clone();
                self.replay(replica, replayed);
            }
            Event::Relayed {
                block,
                from,
                kind,
                payload,
                received,
            } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let bytes = match peer.unseal(payload) {
                    Ok(bytes) => bytes,
                    Err(ClientError::Locked(key)) if !self.searched.contains(&key) => {
                        self.unsealing
                            .insert(*block, VecDeque::from([event.clone()]));
                        if self.searching.insert(key) {
                            #[cfg(test)]
                            {
                                self.key_searches += 1;
                            }
                            let (peer, io) = (Arc::clone(peer), self.io.clone());
                            H::spawn(async move {
                                let fetched = peer.refresh_keys(None).await;
                                let _ = io.send(Io::Keyed { key, fetched });
                                H::wake();
                            });
                        }
                        return;
                    }
                    Err(error) => return H::record(shared, error),
                };
                #[cfg(test)]
                self.heard_from_peers.push(bytes.clone());
                let replayed = Replayed::Message {
                    from: Endpoint::Peer(*from),
                    bytes,
                    at: self.modules.now(),
                    delivery: Some(be_sdk_type_api::Delivery {
                        kind: *kind,
                        received: *received,
                    }),
                };
                if let Some(open) = self.open.get_mut(&replica)
                    && open.joining
                {
                    return open.early.push(replayed);
                }
                self.replay(replica, replayed);
            }
            Event::BlockChanged {
                block:
                    be_block_protocol::BlockSummary {
                        id: block,
                        head: Some(head),
                        ..
                    },
                ..
            } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                if let Some(open) = self.open.get_mut(&replica) {
                    // A change that left the head as the replica has it, or as it published
                    // or was already told it, is no news of the head.
                    if open.heard == Some(*head)
                        || open.published.contains(head)
                        || open.moved.contains(head)
                    {
                        return;
                    }
                    if open.joining {
                        open.heard = Some(*head);
                        return open.early.push(Replayed::Moved { head: *head });
                    }
                }
                self.moved(replica, *head, false);
            }
            _ => {}
        }
    }

    pub fn resynchronize(&mut self, (peer, _): Ctx<'_, H>) {
        // A search that ended as the connection dropped answered nothing, and the server may
        // have keys now that it lacked: what is relayed from here on is searched for again.
        self.searched.clear();
        for (replica, block) in self
            .open
            .iter()
            .filter(|(_, open)| !open.joining)
            .map(|(replica, open)| (*replica, open.block))
            .collect::<Vec<_>>()
        {
            let joining = peer.connection().join_session(block);
            let io = self.io.clone();
            H::spawn(async move {
                let joined = joining.await.map(|(client, session, _)| (client, session));
                let _ = io.send(Io::Resynchronized {
                    replica,
                    joined: Box::new(joined),
                });
                H::wake();
            });
        }
    }

    fn resynchronized(
        &mut self,
        shared: &Arc<Mutex<H>>,
        replica: ReplicaId,
        joined: Result<(ClientId, SessionState), ClientError>,
    ) {
        let session = match joined {
            Ok((_, session)) => session,
            Err(error) if unreachable(&error) => return,
            Err(error) => return H::record(shared, error),
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        if open.joining || session.generation < open.session.generation {
            return;
        }
        open.session = session.clone();
        self.with_replica(replica, |handle, connection| {
            handle.lease(connection, session, true);
        });
    }

    fn claimed(
        &mut self,
        shared: &Arc<Mutex<H>>,
        replica: ReplicaId,
        claimed: Result<SessionState, ClientError>,
    ) {
        let state = match claimed {
            Ok(state) => state,
            Err(error) if unreachable(&error) => return,
            // Only an editor may own a block's lease; a viewer's claim changes nothing.
            Err(ClientError::Refused(ErrorCode::PermissionDenied, _)) => return,
            Err(error) => return H::record(shared, error),
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        let replayed = Replayed::Lease {
            session: state.clone(),
            resynchronize: false,
        };
        if open.joining {
            return open.early.push(replayed);
        }
        if state.generation < open.session.generation {
            return;
        }
        open.session = state;
        self.replay(replica, replayed);
    }

    fn answered(&mut self, (peer, shared): Ctx<'_, H>, io: Io) {
        match io {
            Io::Answer {
                replica,
                module,
                generation,
                epoch,
                answer,
                published,
            } => {
                let heads = answer.heads();
                let delivered = self.modules.generation(module) == generation;
                if delivered {
                    self.modules
                        .call(module, |connection| answer.send(connection));
                }
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                if open.epoch != epoch {
                    return;
                }
                if delivered
                    && let Some(head) = published
                    && open.status.head != Some(head)
                {
                    open.reporting = Some(head);
                }
                if heads {
                    open.publishing = open.publishing.saturating_sub(1);
                }
                if let Some(head) = published {
                    open.published.push(head);
                    let block = open.block;
                    self.sealed_here(block, head);
                }
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                if open.publishing == 0 {
                    for head in std::mem::take(&mut open.moved) {
                        self.moved(replica, head, false);
                    }
                }
            }
            Io::Locked { replica, arrivals } => {
                if let Some(block) = self.open.get(&replica).map(|open| open.block) {
                    match peer.arrivals() == arrivals {
                        true => self.lock(shared, block),
                        // Keys arrived while it read, after `unlock` ran: it reads again.
                        false => self.read_again(block),
                    }
                }
            }
            Io::Published {
                publish,
                result,
                published,
                kept,
            } => {
                self.kept |= kept;
                if let Some(unanswered) = self.publishes.remove(&publish) {
                    self.answer_publish(unanswered, result, published);
                }
            }
            Io::Read {
                module,
                generation,
                request,
                read,
                result,
            } => {
                if self.modules.generation(module) == generation {
                    let mut gone = types::Scope::default();
                    let scope = self.scopes.get_mut(&(module, request)).unwrap_or(&mut gone);
                    self.modules
                        .call(module, |connection| scope.answer(connection, *read, result));
                }
            }
            Io::Relayed => self.relaying -= 1,
            Io::Left => {
                self.left -= 1;
                H::leaving(shared, self.left > 0);
            }
            Io::Sealed => self.release(),
            Io::Keyed { key, fetched } => {
                self.searching.remove(&key);
                match fetched {
                    // Keys it opened may open what earlier searches could not.
                    Ok(opened) if opened > 0 => self.searched.clear(),
                    Ok(_) => {}
                    Err(error) if unreachable(&error) => {}
                    Err(error) => H::record(shared, error),
                }
                // Keys peers name that this device cannot get, forged ones too, are kept only
                // up to a bound: past it, they are searched for again.
                if self.searched.len() >= MOST_SEARCHED_KEYS {
                    self.searched.clear();
                }
                self.searched.insert(key);
                // Each waiting message is unsealed now, let go, or waits again for its own key.
                for (_, events) in std::mem::take(&mut self.unsealing) {
                    for event in events {
                        self.heard((peer, shared), &event);
                    }
                }
            }
            Io::Joined {
                replica,
                block,
                joined,
                arrivals,
            } => self.joined((peer, shared), replica, block, *joined, arrivals),
            Io::Rejoined {
                replica,
                block,
                rejoin,
                joined,
            } => self.rejoined((peer, shared), replica, block, rejoin, *joined),
            Io::Claimed { replica, claimed } => self.claimed(shared, replica, *claimed),
            Io::Resynchronized { replica, joined } => self.resynchronized(shared, replica, *joined),
        }
    }

    pub fn disconnected(&mut self) {
        let replicas: Vec<ReplicaId> = self.open.keys().copied().collect();
        for replica in replicas {
            let Some(open) = self.open.get_mut(&replica) else {
                continue;
            };
            let joining = std::mem::replace(&mut open.joining, false);
            open.rejoining = None;
            open.held = None;
            open.early.clear();
            let block = open.block;
            self.make_up_session(replica);
            // Reconnecting reopens the replica from its head, so what it owns is committed here
            // first, rather than at its next pause.
            self.seal_later(block, false);
            if joining {
                self.close_if_asked(replica);
            }
        }
    }

    pub fn reconnect(&mut self, (peer, _): Ctx<'_, H>) {
        self.rejoins += 1;
        let rejoin = self.rejoins;
        // One whose creation the drain has not sent yet joins once it has (`created`).
        let replicas: Vec<ReplicaId> = self
            .open
            .iter()
            .filter(|(_, open)| {
                !self.uncreated.contains(&open.block) || !cannot_join(peer, open.block)
            })
            .map(|(replica, _)| *replica)
            .collect();
        for replica in replicas {
            let Some(open) = self.open.get(&replica) else {
                continue;
            };
            let block = open.block;
            let unsealed = !open.status.clean || open.unreported();
            self.uncreated.remove(&block);
            // Rejoining reopens it on its head, so what it holds past that head is committed here
            // first, as on disconnecting: one opened offline holds what it replayed from this
            // device's journal, and one the worker opened as the outbox drained may hold what it
            // sequenced since. The answer waits for that seal (`settling`).
            if unsealed {
                self.seal_later(block, false);
            }
            self.rejoin(peer, replica, rejoin);
        }
    }

    /// Whether `replica`'s head is moving past the one it last reported: a seal is asked of it,
    /// on its way, or answered and not yet reported. Reopened from `status.head` meanwhile, it
    /// would lose what that seal holds beyond the edits `replay` gives again (what it replayed
    /// from its journal, or sequenced for a follower): neither that head's journal nor `replay`
    /// holds them (`reopening`).
    fn settling(&self, replica: ReplicaId) -> bool {
        self.open
            .get(&replica)
            .is_some_and(|open| open.publishing > 0 || open.reporting.is_some())
            || self
                .asked
                .values()
                .any(|(asked, ask)| *asked == replica && matches!(ask, Asked::Seal(_)))
    }

    /// Holds a join's answer until `replica` has settled (`settling`).
    fn hold(&mut self, replica: ReplicaId, io: Io) {
        if let Some(open) = self.open.get_mut(&replica) {
            open.held = Some(io);
            self.holding.insert(replica);
        }
    }

    /// Handles the join answers held for replicas that have settled since.
    fn release_held(&mut self, ctx: Ctx<'_, H>) {
        if self.holding.is_empty() {
            return;
        }
        let settled: Vec<ReplicaId> = self
            .holding
            .iter()
            .copied()
            .filter(|replica| !self.settling(*replica))
            .collect();
        for replica in settled {
            self.holding.remove(&replica);
            if let Some(io) = self
                .open
                .get_mut(&replica)
                .and_then(|open| open.held.take())
            {
                self.answered(ctx, io);
            }
        }
    }

    /// Replicas opened while their block's creation waited in the outbox join their sessions
    /// once the server has taken it (after a drain). As on reconnecting, a replica first seals
    /// what it owns in its made-up session, so it joins owning nothing: one with edits is sealed
    /// and joins after a later drain sends that.
    pub fn created(&mut self, (peer, _): Ctx<'_, H>) {
        if !peer.is_attached() {
            return;
        }
        let sent: Vec<Uuid> = self
            .uncreated
            .iter()
            .copied()
            .filter(|block| !cannot_join(peer, *block))
            .collect();
        for block in sent {
            let Some((replica, open)) = self.replica_of(block) else {
                self.uncreated.remove(&block);
                continue;
            };
            if !open.status.clean || !peer.pending_commits(block).is_empty() {
                // This runs every pass of the loop: asked again while a seal is on its way, or
                // while it owes none (its commits wait for the drain, its seal for a timer),
                // its module would wake the loop every pass.
                let sealing = open.publishing > 0
                    || self
                        .asked
                        .values()
                        .any(|(asked, ask)| *asked == replica && matches!(ask, Asked::Seal(_)));
                if open.status.owes_seal && !sealing {
                    self.seal_later(block, false);
                }
                continue;
            }
            self.uncreated.remove(&block);
            let rejoin = self.rejoins;
            self.rejoin(peer, replica, rejoin);
        }
    }

    fn rejoin(&mut self, peer: &Arc<Peer<H::Store>>, replica: ReplicaId, rejoin: u64) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.joining = true;
        open.rejoining = Some(rejoin);
        open.held = None;
        open.early.clear();
        let block = open.block;
        let peer = Arc::clone(peer);
        let io = self.io.clone();
        H::spawn(async move {
            let joined = async {
                let (client, session, journal) = peer.connection().join_session(block).await?;
                let head = peer.remote_head(block).await?;
                let journal = opened(&peer, journal);
                Ok::<_, ClientError>((client, session, head, kept_commits(&peer, block), journal))
            }
            .await;
            let _ = io.send(Io::Rejoined {
                replica,
                block,
                rejoin,
                joined: Box::new(joined),
            });
            H::wake();
        });
    }

    fn rejoined(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        replica: ReplicaId,
        block: Uuid,
        rejoin: u64,
        joined: Rejoined,
    ) {
        let Some(open) = self.open.get_mut(&replica) else {
            if joined.is_ok() && rejoin == self.rejoins && !self.blocks.contains_key(&block) {
                self.leaving.push(block);
            }
            return;
        };
        if open.rejoining != Some(rejoin) {
            return;
        }
        if self.settling(replica) {
            return self.hold(
                replica,
                Io::Rejoined {
                    replica,
                    block,
                    rejoin,
                    joined: Box::new(joined),
                },
            );
        }
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.joining = false;
        open.rejoining = None;
        let early = std::mem::take(&mut open.early);
        let (client, session, head, commits_kept, journal) = match joined {
            Ok(joined) => joined,
            Err(error) => {
                H::record(shared, error);
                for replayed in early {
                    self.replay(replica, replayed);
                }
                self.close_if_asked(replica);
                return;
            }
        };
        open.made_up = false;
        let generation = session.generation;
        let early: Vec<Replayed> = early
            .into_iter()
            .filter(|replayed| match replayed {
                Replayed::Lease { session, .. } => session.generation >= generation,
                _ => true,
            })
            .collect();
        let known = open.status.head;
        let journal = reopening(open, peer, journal, known);
        self.reopen_joined(replica, client, session, known, head, journal, commits_kept);
        for replayed in early {
            self.replay(replica, replayed);
        }
        if let Some(head) = head
            && Some(head) != known
        {
            self.moved(replica, head, false);
        }
        self.close_if_asked(replica);
    }

    pub async fn restart(&mut self, peer: &Peer<H::Store>, module: usize) {
        let restarted: Vec<ReplicaId> = self
            .open
            .iter()
            .filter(|(_, open)| open.module == module)
            .map(|(replica, _)| *replica)
            .collect();
        self.handles.retain(|(from, _), _| *from != module);
        self.contents.retain(|(from, _), _| *from != module);
        self.scopes.retain(|(from, _), _| *from != module);
        let mut waiting: HashMap<OpId, Waiting> = HashMap::new();
        for key in self
            .replies
            .keys()
            .filter(|(from, _)| *from == module)
            .copied()
            .collect::<Vec<_>>()
        {
            if let Some(answer) = self.replies.remove(&key) {
                waiting.insert(key.1, answer);
            }
        }
        let mut asked: HashMap<ReplicaId, Vec<(Asked, Option<Waiting>)>> = HashMap::new();
        for key in self
            .asked
            .keys()
            .filter(|(from, _)| *from == module)
            .copied()
            .collect::<Vec<_>>()
        {
            if let Some((replica, request)) = self.asked.remove(&key) {
                asked
                    .entry(replica)
                    .or_default()
                    .push((request, waiting.remove(&key.1)));
            }
        }
        for replica in restarted {
            let tag = self.modules.tag(module);
            let Some(open) = self.open.get_mut(&replica) else {
                continue;
            };
            open.epoch += 1;
            open.publishing = 0;
            open.moved.clear();
            // Its old module will never report the head it was answered.
            open.reporting = None;
            let head = open.status.head;
            let (journal, _) = reopening(open, peer, None, head);
            open.resent();
            let kept = kept_commits(peer, open.block);
            let opening = Opening {
                block: open.block,
                content_type: open.content_type,
                organization: open.organization,
                account: open.account,
                client: open.client,
                tag,
                head: open.status.head,
                session: open.session.clone(),
                lost_edits: false,
                journal: journal.and_then(|journal| replayable(journal, tag)),
                journal_kept: false,
                commits_kept: kept.0,
                refused_commits: kept.1,
                server_head: peer.known_remote_head(open.block),
                sealed_head: None,
                epoch: open.epoch,
            };
            let editors: Vec<(EditorId, bool)> = open
                .editors
                .iter()
                .map(|(editor, editable)| (*editor, *editable))
                .collect();
            let closing = open.closing && !open.joining;
            let asked = asked.remove(&replica).unwrap_or_default();
            let (replies, pending, replay, departed) = (
                &mut self.replies,
                &mut self.asked,
                &mut open.replay,
                &open.departed,
            );
            let Some(handle) = self.modules.call(module, |connection| {
                let handle = Replica::new(connection, opening);
                for (editor, editable) in editors {
                    handle.attach(connection, editor, editable);
                }
                resend(
                    handle,
                    connection,
                    module,
                    replica,
                    replay,
                    departed,
                    asked,
                    &mut waiting,
                    replies,
                    pending,
                );
                if closing {
                    handle.resource_drop(connection);
                }
                handle
            }) else {
                continue;
            };
            open.handle = handle;
            self.handles.insert((module, handle.op()), replica);
        }
    }

    /// Takes a status the replica's module reported.
    async fn take_status(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        replica: ReplicaId,
        status: Status,
    ) -> bool {
        let Some(open) = self.open.get_mut(&replica) else {
            return false;
        };
        let (block, content_type) = (open.block, open.content_type);
        // Made before the replica was opened again: it says nothing of this opening.
        if status.epoch != open.epoch {
            return false;
        }
        if open.reporting.is_some() && open.status.head != status.head {
            open.reporting = None;
        }
        // What its head holds of the local edits sent it is not given again: every one it took
        // once it is clean, and before that what its seals held (`Status::sealed_taken`). A
        // status the module made before it took the edits sent since covers none of those.
        if status.head.is_some() {
            open.reported_clean(status.sealed_taken);
        }
        open.journaled_taken = status.journaled_taken;
        let became_unreadable = status.unreadable.is_some() && open.status.unreadable.is_none();
        open.status = status.clone();
        if became_unreadable {
            let owned = H::owned(shared, block);
            if let Some(kind) = types::kind_of(content_type)
                && owned
                && !kind.frozen
            {
                match reset::<H>(peer, block, &kind).await {
                    Ok(()) => {
                        if let Some(open) = self.open.get_mut(&replica) {
                            open.reopen = true;
                            open.closing = true;
                        }
                        self.with_replica(replica, |handle, connection| {
                            handle.resource_drop(connection);
                        });
                        return true;
                    }
                    Err(error) => H::record(shared, error),
                }
            }
        }
        H::status(shared, block, &status);
        self.counted(shared);
        true
    }

    /// Takes `status` as the block's replica reporting it with this opening's epoch, as a
    /// module that made it earlier would.
    #[cfg(test)]
    pub(crate) async fn report(&mut self, ctx: Ctx<'_, H>, block: Uuid, status: Status) {
        let Some((replica, open)) = self.replica_of(block) else {
            return;
        };
        let status = Status {
            epoch: open.epoch,
            ..status
        };
        self.take_status(ctx, replica, status).await;
    }

    async fn heard_from(&mut self, ctx: Ctx<'_, H>, module: usize, heard: Heard) -> bool {
        match heard {
            Heard::Hello(types) => {
                self.modules.hello(module, types).await;
                H::wake();
                false
            }
            Heard::Canceled { op } => {
                self.refuse(module, op);
                false
            }
            Heard::Completed { op, outcome } => self.completed(ctx, module, op, outcome).await,
            Heard::Call {
                op,
                parent: Some(parent),
                call,
            } if self.scopes.contains_key(&(module, parent)) => {
                let Some(scope) = self.scopes.get_mut(&(module, parent)) else {
                    return false;
                };
                let under = self
                    .modules
                    .call(module, |connection| scope.take(connection, op, call));
                match under {
                    Some(types::Under::Read(read)) => {
                        let (peer, generation, io) = (
                            Arc::clone(ctx.0),
                            self.modules.generation(module),
                            self.io.clone(),
                        );
                        H::spawn(async move {
                            let result = read.read(&peer).await;
                            let _ = io.send(Io::Read {
                                module,
                                generation,
                                request: parent,
                                read: Box::new(read),
                                result,
                            });
                            H::wake();
                        });
                    }
                    Some(types::Under::Error(message)) => {
                        H::report(ctx.1, None, format!("a type module: {message}"));
                    }
                    Some(types::Under::Answered) | None => {}
                }
                false
            }
            Heard::Call { op, parent, call } => {
                let replica = parent
                    .and_then(|parent| self.handles.get(&(module, parent)).copied())
                    .filter(|replica| self.open.contains_key(replica));
                match replica {
                    Some(replica) => self.act(ctx, module, replica, call).await,
                    None => {
                        if let Call::Error { message, .. } = call {
                            H::report(ctx.1, None, format!("a type module: {message}"));
                        }
                        self.refuse(module, op);
                        false
                    }
                }
            }
        }
    }

    fn acknowledge(&mut self, module: usize, reply: Reply<()>) {
        self.modules
            .call(module, |connection| reply.send(connection, &()));
    }

    fn refuse(&mut self, module: usize, op: OpId) {
        self.modules.call(module, |connection| {
            connection.complete(op, Outcome::Err(OpError::Canceled));
        });
    }

    async fn completed(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        module: usize,
        op: OpId,
        outcome: Outcome,
    ) -> bool {
        self.asked.remove(&(module, op));
        if let Some(answer) = self.replies.remove(&(module, op)) {
            // Its asker takes the scope back, unless it stopped waiting.
            if answer.send(outcome).is_err() {
                self.scopes.remove(&(module, op));
            }
            return false;
        }
        self.scopes.remove(&(module, op));
        if let Some((replica, ticket)) = self.contents.remove(&(module, op)) {
            let (Some(open), Ok(Some(bytes))) = (self.open.get(&replica), ticket.decode(outcome))
            else {
                return false;
            };
            H::content(shared, open.block, open.content_type, bytes);
            return true;
        }
        let Some(replica) = self.handles.remove(&(module, op)) else {
            return false;
        };
        let Some(open) = self.open.remove(&replica) else {
            return false;
        };
        self.asked.retain(|_, (to, _)| *to != replica);
        self.blocks.remove(&open.block);
        if !open.joining {
            self.leave(shared, peer, open.block);
        }
        if open.reopen {
            self.open((peer, shared), open.block, open.content_type, open.reader)
                .await;
            return true;
        }
        H::released(shared, open.block);
        self.counted(shared);
        true
    }

    fn answer_later(
        &self,
        replica: ReplicaId,
        module: usize,
        task: impl Future<Output = (Answered, Option<CommitId>)> + 'static,
    ) {
        let epoch = self.open.get(&replica).map_or(0, |open| open.epoch);
        let generation = self.modules.generation(module);
        let io = self.io.clone();
        H::spawn(async move {
            let (answer, published) = task.await;
            let _ = io.send(Io::Answer {
                replica,
                module,
                generation,
                epoch,
                answer: Box::new(answer),
                published,
            });
            H::wake();
        });
    }

    async fn act(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        module: usize,
        replica: ReplicaId,
        call: Call,
    ) -> bool {
        let Some((block, content_type, made_up, joining, current)) =
            self.open.get(&replica).map(|open| {
                (
                    open.block,
                    open.content_type,
                    open.made_up,
                    open.joining,
                    open.session.generation,
                )
            })
        else {
            return false;
        };
        match call {
            Call::Clock { .. } | Call::SleepUntil { .. } | Call::InStepAt { .. } => false,
            Call::Send {
                reply,
                to,
                kind,
                bytes,
            } => {
                self.acknowledge(module, reply);
                match to {
                    // Only to an editor attached now: one attached for its kept edits alone
                    // (`Open::departed`), or detached since the replica wrote, takes nothing.
                    Recipients::One(Endpoint::Editor(editor))
                        if editor == HOST_EDITOR
                            || self
                                .open
                                .get(&replica)
                                .is_some_and(|open| open.editors.contains_key(&editor)) =>
                    {
                        H::to_editor(shared, block, editor, bytes)
                    }
                    Recipients::One(Endpoint::Editor(_)) => false,
                    Recipients::One(Endpoint::Host) => false,
                    _ if made_up => false,
                    Recipients::One(Endpoint::Peer(client)) => {
                        self.relay(shared, peer.relay(block, Some(client), kind, &bytes).await);
                        false
                    }
                    Recipients::Peers => {
                        self.relay(shared, peer.relay(block, None, kind, &bytes).await);
                        false
                    }
                }
            }
            Call::Append {
                reply,
                generation,
                entries,
                relay,
                announcements,
            } => {
                let announcements: Vec<(u64, be_block_content::Announcement)> = announcements
                    .into_iter()
                    .map(|announced| (announced.sequence, announcement(announced.announcement)))
                    .collect();
                let keys: std::collections::HashSet<String> = announcements
                    .iter()
                    .map(|(_, announcement)| announcement.key.clone())
                    .collect();
                let tag = be_block_store::Hash::from_bytes(self.modules.tag(module).0);
                let entries = entries
                    .into_iter()
                    .map(|entry| be_block_protocol::JournalEntry {
                        generation,
                        sequence: entry.sequence,
                        payload: entry.payload,
                    })
                    .collect();
                let online = !made_up && peer.is_attached();
                let appended = peer
                    .append(
                        block,
                        generation,
                        tag,
                        entries,
                        relay,
                        online,
                        announcements,
                    )
                    .await;
                // Answered once the server holds the entries, or at once when only this
                // device does, so the replica can tell its editors what the server holds.
                if let Some(open) = self.open.get_mut(&replica) {
                    open.append_failed = appended.is_err();
                }
                match appended {
                    Ok(Some(answer)) => {
                        self.relaying += 1;
                        let (shared, io) = (Arc::clone(shared), self.io.clone());
                        let peer = Arc::clone(peer);
                        self.answer_later(replica, module, async move {
                            let held = match answer.await {
                                Ok(()) => {
                                    if !keys.is_empty() {
                                        match peer.journal_held(block, &keys) {
                                            Ok(()) => send_announcements::<H>(&peer, &shared).await,
                                            Err(error) => H::record(&shared, error),
                                        }
                                    }
                                    true
                                }
                                Err(error) => {
                                    if !unreachable(&error) {
                                        H::record_sync(&shared, error);
                                    }
                                    false
                                }
                            };
                            let _ = io.send(Io::Relayed);
                            (Answered::Append(reply, held), None)
                        });
                    }
                    written => {
                        self.journaled(shared, written);
                        self.modules
                            .call(module, |connection| reply.send(connection, &false));
                    }
                }
                false
            }
            Call::RebaseJournal {
                reply,
                generation,
                base,
                entries,
            } => {
                self.acknowledge(module, reply);
                let tag = be_block_store::Hash::from_bytes(self.modules.tag(module).0);
                let base = be_block_protocol::JournalBase {
                    commit: base.commit,
                    sequence: base.sequence,
                    state: base.state,
                };
                // A rebase the module made in a session that has since been replaced (the
                // made-up one it journals in while joining, or a lease that passed) is kept on
                // this device only: the server keeps a journal for its current generation alone,
                // and the module rebases again in the session that replaced it before it appends.
                let online = !made_up && peer.is_attached() && generation == current;
                // An own commit: one a replica of this host sealed, or one this device's outbox
                // keeps.
                let own = base.commit.is_some_and(|commit| {
                    self.sealed
                        .get(&block)
                        .is_some_and(|sealed| sealed.contains(&commit))
                        || peer.pending_commits(block).contains(&commit)
                });
                let entries = entries
                    .into_iter()
                    .map(|entry| be_block_protocol::JournalEntry {
                        generation,
                        sequence: entry.sequence,
                        payload: entry.payload,
                    })
                    .collect();
                let rebased = peer
                    .rebase_journal(block, generation, tag, base, entries, online, own)
                    .await;
                self.journaled(shared, rebased);
                let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
                H::spawn(async move { send_announcements::<H>(&peer, &shared).await });
                false
            }
            Call::Load {
                reply,
                commit,
                first_part,
            } => {
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                self.answer_later(replica, module, async move {
                    let arrivals = peer.arrivals();
                    let read = async {
                        let loaded = peer.load_commit(commit).await?;
                        let read = match first_part {
                            true => peer.load_first_part(&loaded.manifest).await,
                            false => peer.load_manifest(&loaded.manifest).await,
                        };
                        if read.is_ok() && !first_part {
                            peer.read_whole(block, commit);
                        }
                        read
                    }
                    .await;
                    if let Err(ClientError::Locked(_)) = &read {
                        let _ = io.send(Io::Locked { replica, arrivals });
                    }
                    let result = read
                        .map(be_sdk_type_api::Loaded::of)
                        .map_err(|error| error.to_string());
                    (Answered::Load(reply, result), None)
                });
                false
            }
            Call::ReadPart { reply, part, range } => {
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                self.answer_later(replica, module, async move {
                    let arrivals = peer.arrivals();
                    let read = peer
                        .read_part(part.into(), range.offset, range.length)
                        .await;
                    if let Err(ClientError::Locked(_)) = &read {
                        let _ = io.send(Io::Locked { replica, arrivals });
                    }
                    let result = read.map_err(|error| error.to_string());
                    (Answered::ReadPart(reply, result), None)
                });
                false
            }
            Call::PutPart { reply, bytes } => {
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                self.answer_later(replica, module, async move {
                    let arrivals = peer.arrivals();
                    let put = match made_up {
                        true => Err(ClientError::Disconnected(
                            "the server does not have this block yet".to_owned(),
                        )),
                        false => peer.put_part(block, &bytes).await,
                    };
                    if let Err(ClientError::Locked(_)) = &put {
                        let _ = io.send(Io::Locked { replica, arrivals });
                    }
                    let result = put
                        .map(be_sdk_type_api::PartRef::from)
                        .map_err(|error| error.to_string());
                    (Answered::PutPart(reply, result), None)
                });
                false
            }
            Call::Publish {
                reply,
                expected,
                merging,
                content,
                sealed_through,
            } => {
                let Some(open) = self.open.get_mut(&replica) else {
                    return false;
                };
                open.publishing += 1;
                let epoch = open.epoch;
                let local = made_up || self.keeping || !peer.pending_commits(block).is_empty();
                let bookmark = self.bookmarks.remove(&block);
                let publishing = Arc::new(Publishing::default());
                self.published += 1;
                let publish = self.published;
                self.publishes.insert(
                    publish,
                    Unanswered {
                        replica,
                        module,
                        generation: self.modules.generation(module),
                        epoch,
                        reply,
                        publishing: Arc::clone(&publishing),
                    },
                );
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                H::spawn(async move {
                    let sealed = io.clone();
                    let mut write = Write::encoded(
                        content_type,
                        be_sdk_type_api::written_parts(&content.bytes, &content.parts),
                        expected,
                    )
                    .described(content.references, content.derived)
                    .sealed_through(sealed_through.map(
                        |(generation, sequence)| be_block_commit::SealedThrough {
                            generation,
                            sequence,
                        },
                    ));
                    if let Some(merging) = merging {
                        write = write.merging(merging.base, merging.ours);
                    }
                    if let Some(label) = &bookmark {
                        write = write.bookmark(label);
                    }
                    let saved = commit::<H>(&peer, block, write, local, &publishing, move || {
                        let _ = sealed.send(Io::Sealed);
                    })
                    .await;
                    let kept = publishing
                        .kept()
                        .is_some_and(|commit| peer.pending_commits(block).contains(&commit));
                    let (result, published) = match saved {
                        Ok(Saved::Published(head)) => {
                            (Ok(PublishOutcome::Published(head)), Some(head))
                        }
                        // Kept in the outbox, or nothing to seal on a head that waits there.
                        Ok(Saved::Kept(head) | Saved::Unchanged(head))
                            if peer.pending_commits(block).contains(&head) =>
                        {
                            (Ok(PublishOutcome::Kept(head)), None)
                        }
                        // Kept, and a drain has sent it since; or nothing to seal.
                        Ok(Saved::Kept(head) | Saved::Unchanged(head)) => {
                            (Ok(PublishOutcome::Published(head)), None)
                        }
                        Ok(Saved::Rejected { head }) => (Ok(PublishOutcome::Rejected(head)), None),
                        Err(error) => (Err(error.to_string()), None),
                    };
                    let _ = io.send(Io::Published {
                        publish,
                        result,
                        published,
                        kept,
                    });
                    H::wake();
                });
                false
            }
            Call::Stash {
                reply,
                parent,
                content,
            } => {
                if let Some(open) = self.open.get_mut(&replica) {
                    open.publishing += 1;
                }
                let peer = Arc::clone(peer);
                self.answer_later(replica, module, async move {
                    let write = Write::encoded(
                        content_type,
                        be_sdk_type_api::written_parts(&content.bytes, &content.parts),
                        parent,
                    )
                    .described(content.references, content.derived);
                    let stashed = peer
                        .stash(block, write)
                        .await
                        .map_err(|error| error.to_string());
                    (Answered::Stash(reply, stashed), None)
                });
                false
            }
            Call::ResumeSession { reply, local } => {
                let peer = Arc::clone(peer);
                self.answer_later(replica, module, async move {
                    let resumed = async {
                        // Commits this device kept that the server has yet to take (a definition
                        // the worker wrote beside the replica) go on top of the server's head:
                        // the replica reconciles with them, as it opens on them, or it would
                        // seal beside them and the server would refuse one side.
                        let remote = match peer.is_attached() && !joining {
                            true => peer.newest_head(block).await?,
                            false => match peer.local_head(block) {
                                Some(head) => Some(head),
                                None => peer.remote_head(block).await?,
                            },
                        };
                        if let Some(remote) = remote {
                            peer.fetch_history(remote).await?;
                        }
                        if let Some(local) = local {
                            peer.fetch_history(local).await?;
                        }
                        Ok::<_, ClientError>(be_block_session::resume(
                            peer.commits(),
                            local,
                            remote,
                        )?)
                    }
                    .await
                    .map_err(|error| error.to_string());
                    (Answered::Resume(reply, resumed), None)
                });
                false
            }
            Call::ClaimLease { reply, generation } => {
                self.acknowledge(module, reply);
                if !peer.is_attached() || made_up {
                    return false;
                }
                let claiming = peer.connection().claim_ownership(block, generation);
                let io = self.io.clone();
                H::spawn(async move {
                    let claimed = claiming.await;
                    let _ = io.send(Io::Claimed {
                        replica,
                        claimed: Box::new(claimed),
                    });
                    H::wake();
                });
                false
            }
            Call::ReportClean {
                reply,
                generation,
                clean_at,
            } => {
                self.acknowledge(module, reply);
                if !peer.is_attached() || made_up {
                    return false;
                }
                let answer = peer.connection().heartbeat(block, generation, clean_at);
                let shared = Arc::clone(shared);
                H::spawn(async move {
                    if let Err(error) = answer.await
                        && !unreachable(&error)
                    {
                        H::record(&shared, error);
                    }
                });
                false
            }
            Call::Announce {
                reply,
                announcements,
            } => {
                self.acknowledge(module, reply);
                let announcements = announcements.into_iter().map(announcement).collect();
                let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
                H::spawn(async move {
                    match peer.announce_held(block, announcements).await {
                        Ok(()) => send_announcements::<H>(&peer, &shared).await,
                        Err(error) => H::record(&shared, error),
                    }
                });
                false
            }
            Call::ReportStatus { reply, status } => {
                self.acknowledge(module, reply);
                self.take_status((peer, shared), replica, status).await
            }
            Call::Described { reply, description } => {
                self.acknowledge(module, reply);
                let reader = self.open.get(&replica).is_some_and(|open| open.reader);
                let references = peer.resolve(block, description.references.clone());
                H::described(shared, block, references, description);
                if reader && let Some(open) = self.open.get(&replica) {
                    let handle = open.handle;
                    if let Some(ticket) = self
                        .modules
                        .call(module, |connection| handle.content(connection))
                    {
                        self.contents
                            .insert((module, ticket.op()), (replica, ticket));
                    }
                }
                true
            }
            Call::Conflict {
                reply,
                ours,
                theirs,
                count,
            } => {
                self.acknowledge(module, reply);
                H::conflict(
                    shared,
                    block,
                    crate::Conflict {
                        ours,
                        theirs,
                        count: count as usize,
                    },
                );
                true
            }
            Call::Error { reply, message } => {
                self.acknowledge(module, reply);
                H::report(shared, Some(block), format!("{block}: {message}"));
                false
            }
        }
    }

    /// How many open replicas have edits not yet committed, how many owe a seal, and how many
    /// hold edits that are not yet durable: not in the journal on this device (still with the
    /// module, or not yet appended). A journaled edit survives a restart, so it is only waiting
    /// for its commit.
    pub fn counts(&self) -> (usize, Vec<Owing>, Vec<Owing>) {
        let unsealed = self
            .open
            .values()
            .filter(|open| !open.status.clean || open.unreported())
            .count();
        let owing = self
            .open
            .values()
            .filter(|open| open.status.owes_seal)
            .map(Owing::of)
            .collect();
        let undurable = self
            .open
            .values()
            .filter(|open| open.unreported() || (!open.status.clean && !open.status.journaled))
            .map(Owing::of)
            .collect();
        (unsealed, owing, undurable)
    }

    fn counted(&self, shared: &Arc<Mutex<H>>) {
        let (unsealed, owing, undurable) = self.counts();
        H::counted(shared, unsealed, owing, undurable);
    }
}

/// Whether `block`'s replica stays out of its session: the server does not have the block yet
/// (`Peer::uncreated`), or this account may not view it. It joins once that changes (`created`).
fn cannot_join<S: be_block_store::ObjectStore>(peer: &Peer<S>, block: Uuid) -> bool {
    peer.uncreated(block)
        || peer
            .known_summary(block)
            .is_some_and(|summary| !summary.access.can_view())
}

/// Joins `block`'s session, for a replica that shows before it has (`Replicas::open`).
fn joining<S: be_block_store::ObjectStore + 'static>(
    peer: &Arc<Peer<S>>,
    block: Uuid,
    content_type: Uuid,
) -> Pending<Joined> {
    let peer = Arc::clone(peer);
    Box::pin(async move {
        peer.ensure_type(block, content_type, BlockParent::Detached)
            .await?;
        let (client, session, journal) = peer.connection().join_session(block).await?;
        let head = peer.remote_head(block).await?;
        Ok((
            client,
            session,
            head,
            ahead(&peer, block),
            opened(&peer, journal),
        ))
    })
}

fn known_head_of<S: be_block_store::ObjectStore>(peer: &Peer<S>, block: Uuid) -> Option<CommitId> {
    peer.held_head(block)
        .or_else(|| peer.local_head(block))
        .or_else(|| peer.known_remote_head(block))
        .or_else(|| peer.known_summary(block).and_then(|summary| summary.head))
}

async fn commit<H: Host>(
    peer: &Peer<H::Store>,
    block: Uuid,
    write: Write<'_>,
    local: bool,
    publishing: &Publishing,
    sealed: impl FnOnce(),
) -> Result<Saved, ClientError> {
    let before = peer.local_head(block);
    let commit = match peer.seal(block, write).await? {
        Sealed::Unchanged(head) => return Ok(Saved::Unchanged(head)),
        Sealed::Sealed(commit) => commit,
    };
    publishing.keep(commit);
    sealed();
    if local || !peer.is_attached() {
        return Ok(Saved::Kept(commit));
    }
    let saved = peer.publish_sealed(block, commit).await;
    if !publishing.settle() {
        return Ok(Saved::Kept(commit));
    }
    match saved {
        Ok(Saved::Rejected { head }) => {
            peer.withdraw(block, commit, before)?;
            Ok(Saved::Rejected { head })
        }
        Err(error) if unreachable(&error) => Ok(Saved::Kept(commit)),
        saved => saved,
    }
}

fn ahead<S: be_block_store::ObjectStore>(peer: &Peer<S>, block: Uuid) -> Option<CommitId> {
    match peer.pending_commits(block).is_empty() {
        true => None,
        false => peer.local_head(block),
    }
}

async fn reset<H: Host>(
    peer: &Peer<H::Store>,
    block: Uuid,
    kind: &types::Kind,
) -> Result<(), ClientError> {
    // All a block the server lacks has is this device's: nothing else to reset to, and its
    // creation is not set aside.
    if peer.uncreated(block) {
        return Ok(());
    }
    let mut expected = peer.remote_head(block).await?;
    // What this device kept of the block and has not sent is set aside among the unsent edits,
    // to be restored or discarded, before the blank is written, rather than waiting ahead of it
    // to be merged into it. It is set aside with no refusal: one would set aside the blank and
    // every later edit too (`Peer::refusal`).
    if !peer.pending_commits(block).is_empty() {
        peer.move_to_unsent(block, None)?;
        if let Some(head) = expected {
            peer.remember(block, head)?;
        }
    }
    for _ in 0..4 {
        match peer
            .write(
                block,
                Write::encoded(kind.content_type, &kind.blank, expected).described(
                    kind.described.references.clone(),
                    kind.described.derived.clone(),
                ),
            )
            .await?
        {
            Saved::Rejected { head } => expected = head,
            _ => break,
        }
    }
    H::reset_notice();
    Ok(())
}

/// The journal a replica opening at `head` loads: the server's (from joining, opened) or this
/// device's, whichever holds more on that base.
/// The commits of `block` the server refused that this device set aside in the unsent area.
fn unsent_commits<S: be_block_store::ObjectStore>(peer: &Peer<S>, block: Uuid) -> Vec<CommitId> {
    peer.unsent()
        .into_iter()
        .filter(|unsent| unsent.block == block)
        .flat_map(|unsent| unsent.commits)
        .collect()
}

/// The commits of `block` the server just refused: those set aside in the unsent area, and
/// those a refusal left in the outbox to be tried again.
pub fn refused_commits<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
) -> Vec<CommitId> {
    let mut commits = unsent_commits(peer, block);
    commits.extend(peer.pending_commits(block));
    commits
}

/// Whether this device keeps commits of `block` the server lacks, in the outbox, and the commits
/// the server refused that it holds set aside in the unsent area.
fn kept_commits<S: be_block_store::ObjectStore>(peer: &Peer<S>, block: Uuid) -> Kept {
    let refused = unsent_commits(peer, block);
    let kept = match peer.pending_commits(block).is_empty() {
        true => be_sdk_type_api::KeptCommits::None,
        false => be_sdk_type_api::KeptCommits::Outbox,
    };
    (kept, refused)
}

/// What a replica opening at `head` loads: the journal, whether it is this device's (holding
/// entries the server lacks), the commit to load it on, and the sealed head to take after.
pub type Loads = (
    Option<be_block_protocol::Journal>,
    bool,
    Option<CommitId>,
    Option<be_sdk_type_api::SealedHead>,
);

/// The journal to open `head` on (`journal_on`), or, when `head` is a seal this device made on
/// a commit its journal is still on (it stopped after the server took the seal and before it
/// heard so, `Commit::sealed_through`), that journal on that commit, with `head` as the sealed
/// head the replica takes once it replayed it.
///
/// Only this device's own journal is trusted for that, and only when it noted that very seal
/// (`Peer::sealing`, written with the seal's commit and dropped by the next rebase) and holds
/// the seal's last entry in the seal's generation (`reaches_seal`). Anything else is loaded
/// plainly at `head`.
pub fn journal_for<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    server: Option<be_block_protocol::Journal>,
    head: Option<CommitId>,
) -> Loads {
    let (journal, kept) = journal_on(peer, block, server, head);
    if journal.is_some() {
        return (journal, kept, head, None);
    }
    let Some(commit) = head else {
        return (None, false, head, None);
    };
    let own = peer
        .sealing(block)
        .filter(|(sealing, _)| *sealing == commit)
        .and_then(|_| peer.sealed_on(commit))
        .and_then(|(parent, sealed)| {
            let journal = peer.local_journal(block).ok().flatten()?;
            let reaches = journal.base.commit == parent
                && be_block_session::reaches_seal(
                    journal.base.sequence,
                    journal
                        .entries
                        .iter()
                        .map(|entry| (entry.sequence, entry.generation)),
                    sealed.generation,
                    sealed.sequence,
                );
            reaches.then_some((journal, parent, sealed))
        });
    match own {
        Some((journal, parent, sealed)) => {
            let kept = journal
                .entries
                .iter()
                .any(|entry| entry.sequence > sealed.sequence);
            (
                Some(journal),
                kept,
                parent,
                Some(be_sdk_type_api::SealedHead {
                    commit,
                    through: sealed.sequence,
                }),
            )
        }
        None => (None, false, head, None),
    }
}

/// Opens the server's journal as joining answered it, keeping it sealed too.
fn opened<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    journal: Option<be_block_protocol::Journal>,
) -> Option<ServerJournal> {
    let journal = journal?;
    let opened = peer.open_journal(journal.clone()).ok()?;
    Some((journal, opened))
}

/// The journal a replica opened again on `head` starts from, cutting from what it is given
/// again (`Open::replay`) the local edits that journal and `head` hold, so it gets each once:
///
/// - the server's, when it is on `head`: the session's, which every owner appended to. This
///   device's journal becomes it, so what the replica journals next follows its numbering. When
///   this device took part in the session (`own`), its own appends are in it.
/// - otherwise this device's whole journal on `head` (`Status::journaled_taken` says what it
///   holds of the local edits), entries included: what the replica sequenced from followers or
///   replayed after a crash is in no editor's edits.
fn reopening<S: be_block_store::ObjectStore>(
    open: &mut Open,
    peer: &Peer<S>,
    server: Option<ServerJournal>,
    head: Option<CommitId>,
) -> (Option<be_block_protocol::Journal>, bool) {
    let (journal, from, lost) = reopening_journal(peer, open.block, server, head);
    // This device's journal holds what its status says of the local edits only if its appends
    // went through; the server's holds none of this device's own: it rejoins made up.
    if from == Some(Reopening::Device) && !open.append_failed {
        open.reported_clean(open.journaled_taken);
    }
    (journal, lost)
}

/// Whose journal a replica opened again starts from (`reopening_journal`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reopening {
    Server,
    Device,
}

/// The journal on `head` a replica opened again starts from, whose it is, and whether this
/// device's journal there holds entries the replica is not given (`lost_edits`): the server's
/// when it is on `head`, otherwise this device's. This device's journal becomes the server's
/// when each entry it holds past the base is the server's too, so what the replica journals
/// next follows the session's numbering; otherwise it is left as it is, holding what no other
/// journal does.
pub(crate) fn reopening_journal<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    server: Option<ServerJournal>,
    head: Option<CommitId>,
) -> (Option<be_block_protocol::Journal>, Option<Reopening>, bool) {
    let device = peer
        .local_journal(block)
        .ok()
        .flatten()
        .filter(|journal| journal.base.commit == head);
    let Some((sealed, server)) = server.filter(|(_, opened)| opened.base.commit == head) else {
        let from = device.as_ref().map(|_| Reopening::Device);
        return (device, from, false);
    };
    let held = |journal: &be_block_protocol::Journal| -> Vec<(u64, u64, Vec<u8>)> {
        journal
            .entries
            .iter()
            .map(|entry| (entry.generation, entry.sequence, entry.payload.clone()))
            .collect()
    };
    let lost = match device {
        Some(device) if !same_journal(&device, &server) => {
            let theirs = held(&server);
            let extra = device.base.sequence != server.base.sequence
                || held(&device).iter().any(|entry| !theirs.contains(entry));
            if !extra {
                let _ = peer.keep_journal(block, &sealed);
            }
            extra
        }
        Some(_) => false,
        None => {
            let _ = peer.keep_journal(block, &sealed);
            false
        }
    };
    (Some(server), Some(Reopening::Server), lost)
}

/// Whether two opened journals hold the same base and entries.
fn same_journal(a: &be_block_protocol::Journal, b: &be_block_protocol::Journal) -> bool {
    let entries = |journal: &be_block_protocol::Journal| -> Vec<(u64, u64, Vec<u8>)> {
        journal
            .entries
            .iter()
            .map(|entry| (entry.generation, entry.sequence, entry.payload.clone()))
            .collect()
    };
    a.tag == b.tag
        && a.base.commit == b.base.commit
        && a.base.sequence == b.base.sequence
        && entries(a) == entries(b)
}

/// The journal on `head`: the server's or this device's, whichever holds more, and whether it
/// is this device's.
fn journal_on<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    server: Option<be_block_protocol::Journal>,
    head: Option<CommitId>,
) -> (Option<be_block_protocol::Journal>, bool) {
    let on_head = |journal: &be_block_protocol::Journal| journal.base.commit == head;
    let server = server.filter(on_head);
    let local = peer.local_journal(block).ok().flatten().filter(on_head);
    let last = |journal: &be_block_protocol::Journal| {
        journal
            .entries
            .last()
            .map_or(journal.base.sequence, |entry| entry.sequence)
    };
    match (server, local) {
        (Some(server), Some(local))
            if local.tag == server.tag
                && local.base.sequence == server.base.sequence
                && last(&local) > last(&server) =>
        {
            (Some(local), true)
        }
        (Some(server), _) => (Some(server), false),
        (None, local) => {
            let kept = local
                .as_ref()
                .is_some_and(|local| !local.entries.is_empty());
            (local, kept)
        }
    }
}

/// Whether edits of the block made with another build were lost: `journal` holds some this
/// module cannot replay, or the server marked them lost since the last commit.
fn lost_edits(
    journal: Option<&be_block_protocol::Journal>,
    tag: be_sdk_type_api::SessionTag,
) -> bool {
    journal.is_some_and(|journal| {
        journal.lost || (journal.tag.as_bytes() != &tag.0 && !journal.entries.is_empty())
    })
}

/// An opened journal as a replica of this module takes it, when the module wrote it.
fn replayable(
    journal: be_block_protocol::Journal,
    tag: be_sdk_type_api::SessionTag,
) -> Option<be_sdk_type_api::Journal> {
    (journal.tag.as_bytes() == &tag.0).then(|| be_sdk_type_api::Journal {
        base: be_sdk_type_api::JournalBase {
            commit: journal.base.commit,
            sequence: journal.base.sequence,
            state: journal.base.state,
        },
        entries: journal
            .entries
            .into_iter()
            .map(|entry| be_sdk_type_api::JournalEntry {
                sequence: entry.sequence,
                payload: entry.payload,
            })
            .collect(),
    })
}
