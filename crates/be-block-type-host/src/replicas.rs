use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Arc,
    time::Instant,
};

use std::sync::Mutex;
use std::sync::PoisonError;

use be_block_client::{Answer, ClientError, Event, Peer, Reconciled, Saved, Sealed, Write};
use be_block_commit::CommitId;
use be_block_graph::BlockParent;
use be_block_protocol::{ClientId, ErrorCode, Participant, SessionState};
use be_block_session::Membership;
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
use be_sdk_type_guest::holds::{Holds, This, Took};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use uuid::Uuid;

pub(crate) mod reach;

pub use reach::OUTSIDE_REACH;
use reach::Reach;

use crate::{
    Host,
    modules::{self as types, Heard, Modules},
};

pub use be_sdk_type_api::HOST_EDITOR;

/// Why a block whose type no module serves does not open.
const NO_MODULE: &str = "no app here edits this kind of block";

const JOINING_OWNER: ClientId = ClientId::MAX;

pub const EARLY_OPEN: std::time::Duration = std::time::Duration::from_millis(250);

/// A session the host makes up for a replica outside the server's (offline, or while it joins):
/// `clients`, each an editor, the first holding the lease. Its index is 0, which no session the
/// server answers has (a joiner's own entry is its first at least), and its participants speak
/// the build `start` gives them, the replica's own.
fn made_up(clients: &[ClientId]) -> SessionState {
    SessionState {
        index: 0,
        after: 0,
        participants: clients
            .iter()
            .map(|client| Participant {
                client: *client,
                tag: be_block_store::Hash::from_bytes([0; 32]),
                may_edit: true,
                claimed: None,
            })
            .collect(),
    }
}

fn session_of(client: ClientId) -> SessionState {
    made_up(&[client])
}

/// The client a replica with no session edits as, until it joins one (or always, for a local
/// account's peer, which never joins): a fresh client of its own each time it opens, as a server
/// hands each session one, with the top bit set so no client a server numbers shares it, nor
/// another device's offline edits. Never 0, the sequence library's `LOADED`, whose edits
/// `Sequence::replace` refuses: a text made offline would take no replaced child, file or
/// carried-over edit.
fn unjoined_client() -> ClientId {
    loop {
        let client = Uuid::new_v4().as_u64_pair().0 | 1 << 63;
        if client != JOINING_OWNER {
            break client;
        }
    }
}

/// While a replica joins, a stand-in holds the lease, so it seals nothing yet.
fn joining_session(client: ClientId) -> SessionState {
    made_up(&[JOINING_OWNER, client])
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

/// An entry of the order, opened, as a type module takes it. `received` stays on the server's
/// clock: every replica applies the same stamp, so they hold the same content.
fn api_entry(entry: be_block_protocol::JournalEntry) -> be_sdk_type_api::JournalEntry {
    be_sdk_type_api::JournalEntry {
        sequence: entry.sequence,
        origin: entry.origin,
        counter: entry.counter,
        base: entry.base,
        floor: entry.floor,
        received: entry.received,
        payload: entry.payload,
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

/// Entries of `block`'s signed log as a replica of the app `app` is handed them: opened, and
/// who signed each as that app's, judged by its arrival (`Peer::verify_entry`); `unknown` where
/// that cannot be told now (offline, or unopened), `redacted` for an entry whose payload is
/// gone, and `bad` for one whose payload opens to no salt. An author that cannot be told of is asked about once a page, not once an entry.
async fn log_items<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    app: &str,
    entries: Vec<be_block_client::LoggedEntry>,
) -> Vec<be_sdk_type_api::LogItem> {
    let mut untold = HashSet::new();
    let mut items = Vec::with_capacity(entries.len());
    for logged in entries {
        let entry = logged.entry;
        let signer = match &logged.opened {
            None if entry.payload.is_none() => be_sdk_type_api::Signer::Redacted,
            None if logged.malformed => be_sdk_type_api::Signer::Bad,
            None => be_sdk_type_api::Signer::Unknown,
            Some(_) if untold.contains(&entry.author) => be_sdk_type_api::Signer::Unknown,
            Some(said) => match peer
                .verify_entry(
                    block,
                    app,
                    entry.author,
                    entry.id,
                    entry.arrival,
                    said,
                    &entry.signature,
                )
                .await
            {
                Ok(be_block_client::Signer::Verified) => be_sdk_type_api::Signer::Verified,
                Ok(be_block_client::Signer::Unverified) => be_sdk_type_api::Signer::Unverified,
                Ok(be_block_client::Signer::Changed) => be_sdk_type_api::Signer::Changed,
                Ok(be_block_client::Signer::Replaced) => be_sdk_type_api::Signer::Replaced,
                Ok(be_block_client::Signer::Bad) => be_sdk_type_api::Signer::Bad,
                Err(_) => {
                    untold.insert(entry.author);
                    be_sdk_type_api::Signer::Unknown
                }
            },
        };
        items.push(be_sdk_type_api::LogItem {
            seq: entry.seq,
            id: entry.id,
            author: entry.author,
            arrival: entry.arrival,
            payload: logged.opened.map(|said| said.bytes),
            signer,
        });
    }
    items
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

/// A submission refused for a key this device does not hold yet (`ClientError::Locked`): its
/// replica keeps waiting for the answer, and it is submitted again once keys arrive (`unlock`).
/// A replica opened again since gives its edits again itself, so one held for an earlier
/// opening is dropped: its reply answers `Unanswered`, which the reopened replica ignores as from
/// an earlier opening (its epoch).
struct Refused {
    replica: ReplicaId,
    module: usize,
    generation: u64,
    epoch: u64,
    reply: Reply<be_sdk_type_api::host::Submitted>,
    submission: be_block_session::Submission,
    announcements: Vec<be_block_content::Announcement>,
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
    Append(
        Reply<Result<be_sdk_type_api::host::Appended, String>>,
        Result<be_sdk_type_api::host::Appended, String>,
    ),
    ReadLog(
        Reply<Result<be_sdk_type_api::LogPage, String>>,
        Result<be_sdk_type_api::LogPage, String>,
    ),
    Stash(Reply<Result<CommitId, String>>, Result<CommitId, String>),
    Resume(Reply<Result<Resume, String>>, Result<Resume, String>),
    Submit(
        Reply<be_sdk_type_api::host::Submitted>,
        be_sdk_type_api::host::Submitted,
    ),
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
            Answered::Append(reply, result) => reply.send(connection, &result),
            Answered::ReadLog(reply, result) => reply.send(connection, &result),
            Answered::Stash(reply, result) => reply.send(connection, &result),
            Answered::Resume(reply, result) => reply.send(connection, &result),
            Answered::Submit(reply, submitted) => reply.send(connection, &submitted),
        }
    }
}

pub enum Io {
    /// Entries appended to an open block's signed log, verified (`Replicas::appended`).
    Logged {
        block: Uuid,
        items: Vec<be_sdk_type_api::LogItem>,
    },
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
    Left(Uuid),
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
        claimed: Result<(), ClientError>,
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

/// Why `Replicas::search_text` (or `unread`) has no answer.
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
pub(crate) enum Replayed {
    Message {
        from: Endpoint,
        bytes: Vec<u8>,
        at: u64,
        /// The serial an editor's message is held under while a tentative gesture holds it
        /// (`Holds`): not a local edit, and kept where it arrived, out of any cut, until the
        /// layer commits and its operation counts, or its gesture ends without it.
        held: Option<u64>,
    },
    Moved {
        head: CommitId,
    },
    Lease {
        session: SessionState,
    },
    /// A membership entry heard while the replica joined: applied to the session the join
    /// answers, and never sent to the replica itself.
    Membership {
        entry: be_block_protocol::MembershipEntry,
    },
    /// A seal moved the head (`replica.sealed`).
    Sealed {
        commit: CommitId,
        through: u64,
        run: u64,
    },
    /// Entries of the order, opened. Given only to the replica that heard them: one opened
    /// again reads them in the journal.
    Sequenced {
        entries: Vec<be_sdk_type_api::JournalEntry>,
    },
    /// Entries appended to the block's signed log, verified. Like `Sequenced`, given only to the
    /// replica that heard them: one opened again reads the log afresh.
    Logged {
        items: Vec<be_sdk_type_api::LogItem>,
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
                held: None,
                ..
            } | Replayed::Change { .. }
        )
    }

    /// Whether a tentative gesture holds it (`Replayed::Message::held`).
    fn held(&self) -> bool {
        matches!(self, Replayed::Message { held: Some(_), .. })
    }

    fn send(&self, replica: Replica, connection: &mut Connection) -> Option<OpId> {
        match self.clone() {
            Replayed::Change { change, .. } => {
                return Some(replica.change(connection, change).op());
            }
            Replayed::Message {
                from, bytes, at, ..
            } => {
                replica.message(connection, from, bytes, at);
            }
            Replayed::Moved { head } => {
                replica.head_moved(connection, head);
            }
            Replayed::Lease { session } => {
                replica.lease(connection, session);
            }
            Replayed::Membership { .. } => {}
            Replayed::Sealed {
                commit,
                through,
                run,
            } => {
                replica.sealed(connection, commit, through, run);
            }
            Replayed::Sequenced { entries } => {
                replica.sequenced(connection, entries);
            }
            Replayed::Logged { items } => {
                replica.logged(connection, items);
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
        handle.attach(connection, *editor, *editable, 0);
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

/// Whether an editor's frame is an operation, which its replica counts as the editor's.
fn operates(bytes: &[u8]) -> bool {
    matches!(FromEditor::decode(bytes), Some(FromEditor::Operate { .. }))
}

/// Uncounts the editors' operations nothing opened after a replica that ended holds: the
/// local edits it was sent past the `held` ones its journal or seals hold, the last of those in
/// its replay (which a status already cut to what its seals did not hold).
pub(crate) fn untaken(
    forwarded: &mut HashMap<(Uuid, EditorId), u64>,
    block: Uuid,
    (sent, held): (u64, u64),
    replay: &[Replayed],
) {
    let mut untaken = sent.saturating_sub(held);
    for replayed in replay.iter().rev() {
        if untaken == 0 {
            break;
        }
        if !replayed.local_edit() {
            continue;
        }
        untaken -= 1;
        if let Replayed::Message {
            from: Endpoint::Editor(editor),
            bytes,
            ..
        } = replayed
            && operates(bytes)
            && let Some(count) = forwarded.get_mut(&(block, *editor))
        {
            *count = count.saturating_sub(1);
        }
    }
}

/// How many of `editor`'s operations a replica opened now holds before the host replays what it
/// kept: all it sent, less those still in `replay`, which the replica is given again after.
pub(crate) fn acknowledged(
    forwarded: &HashMap<(Uuid, EditorId), u64>,
    block: Uuid,
    replay: &[Replayed],
    editor: EditorId,
) -> u64 {
    let replayed = replay
        .iter()
        .filter(|replayed| {
            matches!(replayed, Replayed::Message { from: Endpoint::Editor(from), bytes, .. }
                if *from == editor && operates(bytes))
        })
        .count() as u64;
    forwarded
        .get(&(block, editor))
        .copied()
        .unwrap_or_default()
        .saturating_sub(replayed)
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
    /// Submissions sent to the server and not yet answered: a join's answer waits for them
    /// (`settling`), so the entries they make reach the replica it opens before its edits are
    /// given again.
    submitting: usize,
    moved: Vec<CommitId>,
    published: Vec<CommitId>,
    /// The head the replica opened at or was last told moved to: news of a block whose head is
    /// still this (another device renamed or shared it) moves nothing.
    heard: Option<CommitId>,
    replay: Vec<Replayed>,
    /// Which of its editors' messages tentative gestures hold, followed as its replica follows
    /// them.
    holds: Holds,
    /// Editors that detached while `replay` holds edits of theirs, and whether they could edit:
    /// what a replica opened again is given their edits as (`resend`).
    departed: BTreeMap<EditorId, bool>,
    closing: bool,
    reopen: bool,
    epoch: u64,
    joining: bool,
    rejoining: Option<u64>,
    /// Whether its made-up session held nothing past its head as its join began: what it took
    /// while the join was on its way is then sealed before the answer is taken (`rejoined`).
    joined_settled: bool,
    /// A seal its join's answer waits for, of what its made-up session took meanwhile.
    sealed_joining: bool,
    /// Whether a seal of what its made-up session held past its head was asked as its join began
    /// (`reconnect`): the join's answer waits until one holds its journaled edits, as for
    /// `sealed_joining`, since opening on the server's journal would drop them.
    sealed_before_joining: bool,
    /// Its local edits were discarded and no join has said since that this account may edit
    /// it (`Peer::discarded`): its editors attach read-only, as nothing they made would be kept.
    marked: bool,
    made_up: bool,
    /// The parts its module may read and reuse (`reach`).
    reach: Arc<Mutex<Reach>>,
    early: Vec<Replayed>,
    /// How many local edits (`Replayed::local_edit`) were sent its module since the replica
    /// was opened or opened again, which the module counts too: those past what its status
    /// says it took (`Status::taken`) a status does not yet reflect.
    sent: u64,
    /// How many of those the session's order holds, as its last status said
    /// (`Status::journaled_taken`): a replica opened again on the journal is given only those
    /// past it.
    journaled_taken: u64,
    /// What its submissions name it (`opening.origin`), kept across its reopenings, so one
    /// opened again finds its own edits in the order.
    origin: u64,
    /// How many local edits sent its module were dropped from `replay` since it was first
    /// opened (`opening.taken-before`): one opened again counts its edits on from there.
    given_up: u64,
    /// The last local edit submitted to the server online, which it may hold though the
    /// replica never heard so (`refs::OpenEdits::submitted_through`).
    submitted_through: u64,
    /// How far `submitted_through` reached when this device last kept the replica's edits: a
    /// submission past it waits until they are kept again (`Call::Submit`).
    kept_submitted_through: u64,
    /// The operations it submitted online, oldest first, while they hold edits of `replay`
    /// (`refs::OpenEdits::submissions`): one opened again submits them as they were.
    submissions: Vec<be_block_client::KeptSubmission>,
    /// Whether one of `submissions` is not kept on the device yet: the next submission writes
    /// them ahead even if it submits no edit past those kept.
    unkept_submissions: bool,
    /// The most restarts of the order this device has seen of the block: those its edits were
    /// kept under, or a replica reported since. `submissions` were made under it, and a restart
    /// past it passes them over. It only grows, though a replica reports none before it loads,
    /// so the edits are kept under it (`OpenEdits::restarts`), not under `status`.
    restarts: u64,
    /// Why keeping its edits last failed, until they are kept (`Owing::keep_error`).
    keep_error: Option<String>,
    /// The last of its local edits kept when the host last stopped (`opening.kept-through`),
    /// told every opening with `restarts` (`opening.kept-restart`).
    kept_through: u64,
    /// The last number of the order this device heard for the block, from the journal it
    /// opened on and each entry since: an entry past the next means one was lost, and the
    /// replica rejoins to read them again. None until it is known.
    heard_through: Option<u64>,
    /// Whether `replay`'s local edits or the counts beside them changed since they were kept
    /// on the device (`Replicas::keep_edits`).
    edits_changed: bool,
    reporting: Option<CommitId>,
    /// A join's answer held while the replica settles (`Replicas::settling`), handled once it
    /// has.
    held: Option<Io>,
    /// Why its last publish failed, if it did: what a host stuck owing a seal names.
    publish_error: Option<String>,
}

impl Open {
    /// The submissions holding edits it would give a replica opened again
    /// (`opening.kept-submissions`).
    fn kept_submissions(&self) -> Vec<be_sdk_type_api::KeptSubmission> {
        self.submissions
            .iter()
            .filter(|submission| submission.through > self.given_up)
            .map(|submission| be_sdk_type_api::KeptSubmission {
                counter: submission.counter,
                through: submission.through,
                payload: submission.payload.clone(),
            })
            .collect()
    }

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
        // What is kept changes only if something is dropped.
        self.edits_changed |= covered > 0 && local > 0;
        if covered >= local {
            self.given_up += local;
            // What tentative gestures hold stays, where it arrived, until they end.
            self.replay.retain(Replayed::held);
            if self.replay.is_empty() {
                self.departed.clear();
            }
            return;
        }
        self.given_up += covered;
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
        let mut at = 0;
        self.replay.retain(|replayed| {
            at += 1;
            at > cut || replayed.held()
        });
    }

    /// Takes what a message, an undo or an editor leaving did to the tentative gestures
    /// (`Holds`): drops the held messages let go, and puts the held operations that count now
    /// at the end of `replay` as local edits, in the order they arrived, as its replica counts
    /// them there. How many that is.
    fn took(&mut self, took: &Took) -> u64 {
        if took.released.is_empty() {
            return 0;
        }
        let mut counted: Vec<Replayed> = Vec::new();
        self.replay.retain(|replayed| match replayed {
            Replayed::Message {
                held: Some(serial), ..
            } if took.released.contains(serial) => {
                if took.counted.contains(serial) {
                    let mut replayed = replayed.clone();
                    if let Replayed::Message { held, .. } = &mut replayed {
                        *held = None;
                    }
                    counted.push(replayed);
                }
                false
            }
            _ => true,
        });
        self.edits_changed = true;
        let count = counted.len() as u64;
        self.replay.extend(counted);
        count
    }
}

/// An open block that owes a seal, and what its seal may be waiting on: what a host names when
/// it stays busy sealing.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Owing {
    pub block: Uuid,
    pub content_type: Uuid,
    /// Why its last publish failed, if it did.
    pub publish_error: Option<String>,
    pub journaled: bool,
    /// Whether its module reports all its edits committed (`Status::clean`).
    pub clean: bool,
    /// Whether its module says it owes a seal (`Status::owes_seal`).
    pub owes_seal: bool,
    /// Whether its module says it diverged from the order (`Status::diverged`).
    pub diverged: bool,
    /// What its module says keeps its edits out of the journal (`Status::unjournaled`).
    pub unjournaled: Option<be_sdk_type_api::Unjournaled>,
    /// The modules' clock as the host last counted (`Modules::now`), which the timers in
    /// `unjournaled` read on.
    pub now: u64,
    /// Whether an editor sent it something its module has not reported on yet.
    pub unreported: bool,
    /// Publishes of it not yet answered.
    pub publishing: usize,
    pub joining: bool,
    /// While its rejoin's answer is held because no seal can take the journaled edits it would
    /// lose (`settling`), how many there are.
    pub rejoin_held: Option<u64>,
    pub closing: bool,
    /// Why keeping its edits on this device last failed (`Replicas::keep_edits`), if it did.
    pub keep_error: Option<String>,
}

impl Owing {
    fn of(open: &Open, now: u64) -> Self {
        Self {
            block: open.block,
            content_type: open.content_type,
            publish_error: open.publish_error.clone(),
            journaled: open.status.journaled,
            clean: open.status.clean,
            owes_seal: open.status.owes_seal,
            diverged: open.status.diverged,
            unjournaled: open.status.unjournaled.clone(),
            now,
            unreported: open.unreported(),
            publishing: open.publishing,
            joining: open.joining,
            rejoin_held: rejoin_held(open)
                .then(|| open.status.journaled_taken - open.status.sealed_taken),
            closing: open.closing,
            keep_error: open.keep_error.clone(),
        }
    }
}

/// What keeps a replica's edits out of the journal (`Unjournaled`), in words, with each timer
/// read against the modules' clock `now`.
fn unjournaled(held: &be_sdk_type_api::Unjournaled, now: u64) -> String {
    let mut what = Vec::new();
    if !held.loaded {
        what.push("not loaded".to_owned());
    }
    if held.pending > 0 {
        what.push(format!("{} submitted, not yet numbered", held.pending));
    }
    if held.unsent > 0 {
        what.push(format!("{} not yet submitted", held.unsent));
    }
    if held.doubtful > 0 {
        what.push(format!("{} held until it joins", held.doubtful));
    }
    if held.putting {
        what.push("a part being put".to_owned());
    }
    for (timer, at) in [
        ("put retry", held.put_retry_at),
        ("merge retry", held.merge_retry_at),
        ("seal retry", held.seal_retry_at),
    ] {
        if let Some(at) = at {
            what.push(match at.checked_sub(now) {
                Some(left) => format!("{timer} in {left} ms"),
                None => format!("{timer} due {} ms ago", now - at),
            });
        }
    }
    if held.merge_failures > 0 {
        what.push(format!("{} merge failure(s)", held.merge_failures));
    }
    if let Some(through) = held.resubmits_through {
        what.push(format!(
            "an earlier opening's submission waits for the local edits through {through}"
        ));
    }
    format!("out of the journal: {}", what.join(", "))
}

impl std::fmt::Display for Owing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} of type {} (clean: {}, owes a seal: {}, diverged: {}, journaled: {}{}, unreported: {}, publishes unanswered: {}, joining: {}{}, closing: {}, last publish failed: {:?}, last keep failed: {:?})",
            self.block,
            self.content_type,
            self.clean,
            self.owes_seal,
            self.diverged,
            self.journaled,
            self.unjournaled
                .as_ref()
                .map_or_else(String::new, |held| format!(
                    ", {}",
                    unjournaled(held, self.now)
                )),
            self.unreported,
            self.publishing,
            self.joining,
            self.rejoin_held.map_or_else(String::new, |edits| format!(
                " (rejoin held: a seal cannot take {edits} journaled edit(s))"
            )),
            self.closing,
            self.publish_error,
            self.keep_error
        )
    }
}

/// How many commits sealed here `Replicas` remembers per block.
const SEALED_REMEMBERED: usize = 16;

/// The most keys `Replicas::searched` holds before it starts over.
const MOST_SEARCHED_KEYS: usize = 4096;

/// What the host tells the replicas of a block: a session's event, or the head the server holds.
#[derive(Clone)]
enum Told {
    Event(Event),
    Head { block: Uuid, head: CommitId },
}

pub struct Replicas<H: Host> {
    pub modules: Modules,
    /// Per block and editor, how many operations the editor sent its replicas, kept across
    /// replicas reopened under it until it detaches: what a reopened replica is told of the
    /// editor's operations it holds (`acknowledged`).
    forwarded: HashMap<(Uuid, EditorId), u64>,
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
    /// The blocks whose sessions those leaves are leaving, each with how many.
    left_blocks: HashMap<Uuid, usize>,
    /// Per block, what was told behind a relayed message whose key is being searched for.
    unsealing: HashMap<Uuid, VecDeque<Told>>,
    /// Keys a search is on its way for, which relayed messages wait on.
    searching: HashSet<Uuid>,
    /// Keys a search answered for since a search last opened any or this device last
    /// reconnected or keys arrived (`unlock`): a message still sealed with one is let go.
    searched: HashSet<Uuid>,
    /// Blocks a read or a submission here found locked (`H::set_locked`), until keys arrive
    /// (`unlock`).
    locked: HashSet<Uuid>,
    /// Submissions refused for a missing key, submitted again once keys arrive (`unlock`).
    refused: Vec<Refused>,
    /// How many searches for keys relayed messages started.
    #[cfg(test)]
    key_searches: usize,
    /// Holds back the idle pass's `keep_edits`, so a test can stop the host between handling a
    /// submission and keeping its edits, as a crash there would.
    #[cfg(test)]
    pub(crate) hold_kept_edits: bool,
    /// How many seals to come fail as a store write refused for a missing key would
    /// (`ClientError::Locked` from `Peer::seal`), so a test can make a seal really fail.
    #[cfg(test)]
    pub(crate) failing_seals: usize,
    /// Each opening of a replica, as `(block, kept_restart, through of each kept submission)`.
    #[cfg(test)]
    pub(crate) openings: Vec<(Uuid, u64, Vec<u64>)>,
    /// Blocks `open` gives up on with this reason, as one whose join the server refused would.
    #[cfg(test)]
    pub(crate) refuse_opening: HashMap<Uuid, String>,
    /// Blocks `start` once finds kept edits of that stopped opening since `open` checked them.
    #[cfg(test)]
    pub(crate) kept_stop_opening: HashSet<Uuid>,
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
            left_blocks: HashMap::new(),
            unsealing: HashMap::new(),
            searching: HashSet::new(),
            searched: HashSet::new(),
            locked: HashSet::new(),
            refused: Vec::new(),
            forwarded: HashMap::new(),
            #[cfg(test)]
            key_searches: 0,
            #[cfg(test)]
            hold_kept_edits: false,
            #[cfg(test)]
            failing_seals: 0,
            #[cfg(test)]
            openings: Vec::new(),
            #[cfg(test)]
            refuse_opening: HashMap::new(),
            #[cfg(test)]
            kept_stop_opening: HashSet::new(),
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

    /// Whether a replica of `block` is about to leave its session, or leaving it.
    pub fn is_leaving(&self, block: Uuid) -> bool {
        self.left_blocks.contains_key(&block) || self.leaving.contains(&block)
    }

    fn leave(&mut self, shared: &Arc<Mutex<H>>, peer: &Peer<H::Store>, block: Uuid) {
        let left = peer.connection().leave_session(block);
        self.left += 1;
        *self.left_blocks.entry(block).or_insert(0) += 1;
        H::leaving(shared, true);
        let io = self.io.clone();
        H::spawn(async move {
            let _ = left.await;
            let _ = io.send(Io::Left(block));
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

    /// Puts `block`'s replica in a rejoin whose answer came back after its first seal, with
    /// the order holding `journaled` local edits of which a seal holds `sealed`, and whether a
    /// seal would take the rest: the answer, an error, waits for the next `pump`. It sets
    /// `Open::journaled_taken` too, as a status saying so would.
    #[cfg(test)]
    pub(crate) fn rejoined_after_a_seal(
        &mut self,
        block: Uuid,
        journaled: u64,
        sealed: u64,
        owes_seal: bool,
    ) {
        let rejoin = self.rejoins;
        let Some(&replica) = self.blocks.get(&block) else {
            return;
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.joining = true;
        open.rejoining = Some(rejoin);
        open.joined_settled = true;
        open.sealed_joining = true;
        open.status.journaled_taken = journaled;
        open.journaled_taken = journaled;
        open.status.sealed_taken = sealed;
        open.status.clean = journaled == sealed;
        open.status.owes_seal = owes_seal;
        self.pending.push(Io::Rejoined {
            replica,
            block,
            rejoin,
            joined: Box::new(Err(ClientError::TimedOut("a test's rejoin"))),
        });
    }

    /// Says what `block`'s replica's status says of whether a seal would take what it owes, as
    /// a report from its module would.
    #[cfg(test)]
    pub(crate) fn set_owes_seal(&mut self, block: Uuid, owes_seal: bool) {
        if let Some(replica) = self.blocks.get(&block)
            && let Some(open) = self.open.get_mut(replica)
        {
            open.status.owes_seal = owes_seal;
        }
    }

    /// Whether a seal of `block` is asked of its module and not yet answered.
    #[cfg(test)]
    pub(crate) fn seal_asked(&self, block: Uuid) -> bool {
        self.asked.values().any(|(replica, asked)| {
            matches!(asked, Asked::Seal(_))
                && self
                    .open
                    .get(replica)
                    .is_some_and(|open| open.block == block)
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
        if self.modules.kept() {
            return;
        }
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
                self.keep_edits(ctx.0).await;
                // What is not yet kept, and why, as keeping left it.
                self.counted(ctx.1);
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

    /// The session tag of the module that holds `content_type`, as joining names it.
    fn tag_of(&self, content_type: Uuid) -> Option<be_block_store::Hash> {
        let module = self.modules.module_of(content_type)?;
        Some(be_block_store::Hash::from_bytes(self.modules.tag(module).0))
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

    /// Opens `block`'s replica, or says why it gave up: no module serves the type, or opening
    /// failed. A block waiting for a key, or out of reach with nothing here to read yet, has not
    /// given up.
    pub async fn open(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        block: Uuid,
        content_type: Uuid,
        reader: bool,
    ) -> Result<(), String> {
        if let Some(replica) = self.blocks.get(&block).copied() {
            let Some(open) = self.open.get_mut(&replica) else {
                return Ok(());
            };
            open.closing = false;
            if reader && !open.reader {
                open.reader = true;
                self.attach_now(replica, HOST_EDITOR, true);
            }
            return Ok(());
        }
        #[cfg(test)]
        if let Some(reason) = self.refuse_opening.get(&block) {
            return Err(reason.clone());
        }
        let Some(kind) = self.kind(content_type).await else {
            return Err(NO_MODULE.to_owned());
        };
        let arrivals = peer.arrivals();
        // Edits this device kept of the block that do not open while its key is not here stay
        // kept, and the block waits for keys as one whose content is locked does: opened
        // without them, the replica's next kept edits would write over them. Ones that do not
        // open otherwise (unreadable, or under a key that has not come though the block's
        // has) are set aside, never sent, and the block opens under a fresh origin.
        if let Some((kept, Err(error))) = peer.open_edits(block) {
            let content_opens = match error {
                ClientError::Locked(_) => match peer.holds_key(block).await {
                    Ok(holds) => holds,
                    // Whether the key is here cannot be told: neither locked nor opened, so
                    // the kept edits stay as they are. It opens once the host reconnects.
                    Err(error) if error.is_unreachable() => return Ok(()),
                    Err(_) => false,
                },
                _ => true,
            };
            if !content_opens {
                if peer.arrivals() == arrivals {
                    self.lock(shared, block);
                    return Ok(());
                }
                return Box::pin(self.open((peer, shared), block, content_type, reader)).await;
            }
            if let Err(error) = peer.set_aside_open_edits(block) {
                let reason = error.to_string();
                H::record(shared, error);
                return Err(reason);
            }
            H::report(
                shared,
                Some(block),
                format!(
                    "{} edits this device kept of this block could not be read and were not \
                     sent ({error})",
                    kept.edits.len()
                ),
            );
        }
        let owned = !kind.frozen && H::owned(shared, block);
        let known = peer
            .known_summary(block)
            .is_some_and(|summary| summary.content_type == content_type);
        // What this device numbered offline is in its journal alone, which joining would replace
        // with the server's: as a block made offline does, it opens on that journal, seals it,
        // and joins once a drain sent the seal (`created`).
        let joins_later = cannot_join(peer, block) || numbered_here(peer, block);
        let tag = self.tag_of(content_type);
        let joined = if known
            && peer.is_attached()
            && !joins_later
            && !(owned && H::reshaped(shared, block, &kind.schema))
        {
            let joining = joining(peer, block, content_type, tag);
            let waited = self.modules.clock().sleep(EARLY_OPEN);
            match futures_util::future::select(joining, waited).await {
                futures_util::future::Either::Left((joined, _)) => {
                    joined.map(|(client, session, head, _, journal)| {
                        (client, session, head, false, journal)
                    })
                }
                futures_util::future::Either::Right((_, joining)) => {
                    let head = known_head_of(peer, block);
                    let client = unjoined_client();
                    match self.start(
                        peer,
                        block,
                        content_type,
                        reader,
                        client,
                        joining_session(client),
                        head,
                        None,
                        false,
                        None,
                        false,
                    ) {
                        Some(replica) => {
                            self.tell_marked(shared, replica);
                            self.join_later(replica, block, joining, arrivals);
                            return Ok(());
                        }
                        None => {
                            return self
                                .unstarted((peer, shared), block, content_type, reader)
                                .await;
                        }
                    }
                }
            }
        } else {
            async {
                if peer.known_summary(block).is_none() || peer.is_attached() {
                    peer.ensure_type(block, content_type, BlockParent::Detached)
                        .await?;
                    if peer.is_attached() {
                        H::created(shared, block, BlockParent::Detached);
                    }
                }
                // Until the drain sends its creation the server does not know the block, and
                // without access to it the server refuses its session: it opens as offline does
                // and joins once that changes (`created`).
                if !peer.is_attached() || joins_later {
                    let client = unjoined_client();
                    return Ok((client, session_of(client), None, true, None));
                }
                if owned && H::reshaped(shared, block, &kind.schema) {
                    reset::<H>(peer, block, &kind).await?;
                }
                let tag = tag.ok_or(ClientError::Unexpected)?;
                let (client, session, journal) = peer.join_session(block, tag).await?;
                // Reading the head lists the block, so `changes` tells its next ones.
                let head = peer.remote_head(block).await?;
                let journal = opened(peer, block, journal);
                Ok::<_, ClientError>((client, session, head, false, journal))
            }
            .await
        };
        let joined = match joined {
            Err(error) if unreachable(&error) && peer.known_summary(block).is_some() => {
                let client = unjoined_client();
                Ok((client, session_of(client), None, true, None))
            }
            joined => joined,
        };
        let (client, session, head, offline, journal) = match joined {
            Ok((client, _, _, true, _)) => {
                if joins_later {
                    self.uncreated.insert(block);
                }
                let head = known_head_of(peer, block);
                if head.is_some() && peer.held_head(block).is_none() {
                    // Nothing to read: the block opens once the host reconnects, and meanwhile
                    // takes no edits it could never journal or publish.
                    return Ok(());
                }
                (client, session_of(client), head, true, None)
            }
            Ok(joined) => joined,
            Err(ClientError::Locked(_)) if peer.arrivals() == arrivals => {
                self.lock(shared, block);
                return Ok(());
            }
            // Defensive: keys arrived while it opened, after `unlock` ran, so it opens again.
            // `keys_changed` runs on the worker's loop, which this open holds, so the count
            // does not move while it runs today.
            Err(ClientError::Locked(_)) => {
                return Box::pin(self.open((peer, shared), block, content_type, reader)).await;
            }
            // Out of reach with nothing kept here to open from: it opens once the host
            // reconnects, and is loading till then.
            Err(error) if unreachable(&error) => {
                H::record(shared, error);
                return Ok(());
            }
            Err(error) => {
                let reason = error.to_string();
                H::record(shared, error);
                return Err(reason);
            }
        };
        let remote = head;
        let head = match offline {
            true => head,
            false => ahead(peer, block).or(head),
        };
        if let Err(error) = mirror(peer, block, journal.as_ref(), !offline, head) {
            H::record(shared, error);
        }
        let (journal, journal_kept, load_at, sealed_head) = journal_for(
            peer,
            block,
            journal.map(|(_, opened)| opened),
            !offline,
            head,
        );
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
            offline,
        ) else {
            return self
                .unstarted((peer, shared), block, content_type, reader)
                .await;
        };
        self.tell_marked(shared, replica);
        if offline {
            return Ok(());
        }
        // Told against what it loaded, not `head`: opened on the server's journal, it starts at
        // that journal's base, which may predate the head (one the outbox published while the
        // journal's holder was away), and only hearing of the head merges it in.
        let mut heads = vec![remote, H::known_head(shared, block)];
        heads.dedup();
        for moved in heads.into_iter().flatten() {
            if Some(moved) != load_at {
                self.moved(replica, moved, false);
            }
        }
        Ok(())
    }

    /// Answers `open` when `start` opened no replica of `block`: one is open already, no
    /// module here serves the type, or its kept edits stopped opening since `open` checked
    /// them, which it opens again to sort out as it does any (waiting for their keys, or
    /// setting them aside); that open finds them failing, so it does not come back here (or
    /// sets them aside, so start opens).
    async fn unstarted(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        block: Uuid,
        content_type: Uuid,
        reader: bool,
    ) -> Result<(), String> {
        if self.blocks.contains_key(&block) {
            return Ok(());
        }
        if self.modules.module_of(content_type).is_none() {
            return Err(NO_MODULE.to_owned());
        }
        Box::pin(self.open((peer, shared), block, content_type, reader)).await
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
        offline: bool,
    ) -> Option<ReplicaId> {
        if self.blocks.contains_key(&block) {
            return None;
        }
        let module = self.modules.module_of(content_type)?;
        #[cfg(test)]
        if self.kept_stop_opening.remove(&block) {
            return None;
        }
        self.next += 1;
        let replica = self.next;
        let organization = peer.organization();
        let joining = session.has(JOINING_OWNER);
        let tag = self.modules.tag(module);
        let mut session = session;
        if session.index == 0 {
            for participant in &mut session.participants {
                participant.tag = be_block_store::Hash::from_bytes(tag.0);
            }
        }
        let kept = kept_commits(peer, block);
        // Local edits no journal held when the host last stopped: given again under the same
        // origin, so those the server's order holds are found there. Before joining, those it
        // may hold only show (`opening.doubtful-through`).
        let (recovered, recovered_edits, submissions) = match peer.open_edits(block) {
            Some((recovered, Ok((edits, submissions)))) => {
                (Some(recovered), Some(edits), submissions)
            }
            // They stopped opening since `open` looked: it opens again once they do.
            Some((_, Err(_))) => return None,
            None => (None, None, Vec::new()),
        };
        let recovered_any = recovered.is_some();
        let origin = recovered.as_ref().map_or_else(
            || Uuid::new_v4().as_u64_pair().0,
            |recovered| recovered.origin,
        );
        let given_up = recovered.as_ref().map_or(0, |recovered| recovered.given_up);
        let submitted_through = recovered
            .as_ref()
            .map_or(0, |recovered| recovered.submitted_through);
        let kept_restart = recovered.as_ref().map_or(0, |recovered| recovered.restarts);
        // Made up while joining or offline: not yet in the server's order.
        let made_up = joining || offline;
        // Before a join that will settle them: a local account's peer never joins, so what it
        // submitted is in its own order and nothing it holds is doubtful.
        let before_join = joining || (offline && !peer.is_local());
        let opening_journal = journal.clone().filter(|_| !before_join);
        let replay: Vec<Replayed> = recovered_edits
            .unwrap_or_default()
            .into_iter()
            .map(|edit| match edit {
                be_block_client::KeptEdit::Frame { editor, bytes } => Replayed::Message {
                    from: Endpoint::Editor(editor),
                    bytes,
                    at: 0,
                    held: None,
                },
                be_block_client::KeptEdit::Change(change) => Replayed::Change {
                    change: api_change(change),
                    op: None,
                },
            })
            .collect();
        let departed: BTreeMap<EditorId, bool> = replay
            .iter()
            .filter_map(|replayed| match replayed {
                Replayed::Message {
                    from: Endpoint::Editor(editor),
                    ..
                } => Some((*editor, true)),
                _ => None,
            })
            .collect();
        let opening = Opening {
            block,
            content_type,
            organization,
            account: peer.account(),
            client,
            tag,
            origin,
            taken_before: given_up,
            doubtful_through: match before_join {
                true => submitted_through,
                false => 0,
            },
            kept_restart,
            kept_through: given_up + replay.len() as u64,
            kept_submitted: submitted_through,
            kept_submissions: submissions
                .iter()
                .map(|submission| be_sdk_type_api::KeptSubmission {
                    counter: submission.counter,
                    through: submission.through,
                    payload: submission.payload.clone(),
                })
                .collect(),
            head,
            session: session.clone(),
            lost_edits: lost_edits(journal.as_ref(), tag),
            journal: journal.and_then(|journal| replayable(journal, tag, head)),
            journal_kept,
            commits_kept: kept.0,
            refused_commits: kept.1,
            server_head: peer.known_remote_head(block),
            sealed_head,
            epoch: 0,
        };
        let mut replay = replay;
        let sent = replay.len() as u64;
        let replay_count = sent;
        let (replies, pending) = (&mut self.replies, &mut self.asked);
        #[cfg(test)]
        note_opening(&mut self.openings, &opening);
        let handle = self.modules.call(module, |connection| {
            let handle = Replica::new(connection, opening);
            resend(
                handle,
                connection,
                module,
                replica,
                &mut replay,
                &departed,
                Vec::new(),
                &mut HashMap::new(),
                replies,
                pending,
            );
            handle
        })?;
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
                    restarts: kept_restart,
                    ..Status::default()
                },
                session,
                reader,
                publishing: 0,
                submitting: 0,
                moved: Vec::new(),
                published: Vec::new(),
                heard: head,
                replay,
                holds: Holds::default(),
                // Editors whose kept edits are given again never attached in this run: a replica
                // opened again is given their edits as these were (`resend`).
                departed,
                closing: false,
                reopen: false,
                epoch: 0,
                joining,
                rejoining: None,
                joined_settled: false,
                marked: peer.discarded(block),
                sealed_joining: false,
                sealed_before_joining: false,
                made_up,
                reach: Arc::default(),
                early: Vec::new(),
                sent,
                journaled_taken: 0,
                origin,
                given_up,
                submitted_through,
                kept_submitted_through: submitted_through,
                submissions,
                unkept_submissions: false,
                restarts: kept_restart,
                keep_error: None,
                kept_through: given_up + replay_count,
                heard_through: journal_through(opening_journal.as_ref()),
                // A block opened for the first time on this device keeps its origin at once.
                edits_changed: !recovered_any,
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

    /// The restarts of the order `block`'s kept edits would be kept under now.
    #[cfg(test)]
    pub(crate) fn kept_restarts_of(&self, block: Uuid) -> Option<u64> {
        let open = self.open.get(self.blocks.get(&block)?)?;
        Some(kept_edits(open).0.restarts)
    }

    /// Of `block`'s open replica: how many of its submissions are unanswered, the restarts of
    /// the order it reported, and the `through` of each submission it keeps (`Open::submissions`).
    #[cfg(test)]
    pub(crate) fn submissions_of(&self, block: Uuid) -> Option<(usize, u64, Vec<u64>)> {
        let open = self.open.get(self.blocks.get(&block)?)?;
        Some((
            open.submitting,
            open.status.restarts,
            open.submissions
                .iter()
                .map(|submission| submission.through)
                .collect(),
        ))
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
        let mut session = session;
        let early: Vec<Replayed> = std::mem::take(&mut open.early)
            .into_iter()
            .filter(|replayed| match replayed {
                // Heard after the membership the join answered, or before it and skipped.
                Replayed::Membership { entry } => {
                    session.apply(entry);
                    false
                }
                Replayed::Lease { .. } => false,
                _ => true,
            })
            .collect();
        let from = match edited || ahead.is_some() {
            true => open.status.head,
            false => head,
        };
        open.marked = peer.discarded(block);
        if let Err(error) = mirror(peer, block, journal.as_ref(), true, from) {
            H::record(shared, error);
        }
        let loads = reopening(open, peer, journal, true, from);
        let from = loads.2;
        let commits_kept = match ahead.is_some() {
            true => (be_sdk_type_api::KeptCommits::Outbox, Vec::new()),
            false => (be_sdk_type_api::KeptCommits::None, Vec::new()),
        };
        let (ordered, early): (Vec<Replayed>, Vec<Replayed>) = early
            .into_iter()
            .partition(|replayed| matches!(replayed, Replayed::Sequenced { .. }));
        self.reopen_joined(replica, client, session, head, loads, commits_kept, ordered);
        self.tell_marked(shared, replica);
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
        server_head: Option<CommitId>,
        (journal, journal_kept, head, sealed_head): Loads,
        commits_kept: Kept,
        ordered: Vec<Replayed>,
    ) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.client = client;
        open.session = session.clone();
        open.heard_through = journal_through(journal.as_ref());
        open.epoch += 1;
        open.publishing = 0;
        open.moved.clear();
        open.published.clear();
        open.heard = head;
        open.reporting = None;
        open.status.head = head;
        open.replay
            .retain(|replayed| replayed.local_edit() || replayed.held());
        open.resent();
        let (module, handle) = (open.module, open.handle);
        let tag = self.modules.tag(module);
        let opening = Opening {
            block: open.block,
            content_type: open.content_type,
            organization: open.organization,
            account: open.account,
            client,
            tag,
            origin: open.origin,
            taken_before: open.given_up,
            doubtful_through: 0,
            kept_restart: open.restarts,
            kept_through: open.kept_through,
            kept_submitted: open.submitted_through,
            kept_submissions: open.kept_submissions(),
            head,
            session,
            lost_edits: lost_edits(journal.as_ref(), tag),
            journal: journal.and_then(|journal| replayable(journal, tag, head)),
            journal_kept,
            commits_kept: commits_kept.0,
            refused_commits: commits_kept.1,
            server_head,
            sealed_head,
            epoch: open.epoch,
        };
        let editors: Vec<(EditorId, bool, u64)> = open
            .editors
            .iter()
            .map(|(editor, editable)| {
                let held = acknowledged(&self.forwarded, open.block, &open.replay, *editor);
                (*editor, *editable && !open.marked, held)
            })
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
        #[cfg(test)]
        note_opening(&mut self.openings, &opening);
        self.modules.call(module, |connection| {
            handle.reopen(connection, opening);
            for &(editor, editable, held) in &editors {
                handle.attach(connection, editor, editable, held);
            }
            // What the order numbered while the join was on its way, its own submissions among
            // it, before its edits are given again: those it holds are not made twice.
            for entries in &ordered {
                entries.send(handle, connection);
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
            for &(editor, ..) in &editors {
                handle.resent(connection, editor);
            }
        });
    }

    /// Tells the host whether the replica's editors are attached read-only as its block is
    /// marked (`H::set_marked`), once they are attached as it says.
    fn tell_marked(&self, shared: &Arc<Mutex<H>>, replica: ReplicaId) {
        if let Some(open) = self.open.get(&replica) {
            H::set_marked(shared, open.block, open.marked);
        }
    }

    fn make_up_session(&mut self, replica: ReplicaId) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.made_up = true;
        let tag = be_block_store::Hash::from_bytes(self.modules.tag(open.module).0);
        let mut session = session_of(open.client);
        for participant in &mut session.participants {
            participant.tag = tag;
        }
        open.session = session.clone();
        self.with_replica(replica, |handle, connection| {
            handle.lease(connection, session);
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
        let new = open.editors.insert(editor, editable).is_none();
        open.departed.remove(&editor);
        let held = acknowledged(&self.forwarded, open.block, &open.replay, editor);
        let (module, handle) = (open.module, open.handle);
        let editable = editable && !open.marked;
        self.modules.call(module, |connection| {
            handle.attach(connection, editor, editable, held);
            // This replica was given already whatever the host kept of the editor's edits, so
            // its snapshot can go out at once.
            if new {
                handle.resent(connection, editor);
            }
        });
    }

    fn with_replica(&mut self, replica: ReplicaId, call: impl FnOnce(Replica, &mut Connection)) {
        if let Some(open) = self.open.get(&replica) {
            let (module, handle) = (open.module, open.handle);
            self.modules
                .call(module, |connection| call(handle, connection));
        }
    }

    /// Sends `replayed` to the replica's module without keeping it to give again.
    fn send_only(&mut self, replica: ReplicaId, replayed: &Replayed) {
        let Some(open) = self.open.get(&replica) else {
            return;
        };
        let (module, handle) = (open.module, open.handle);
        self.modules.call(module, |connection| {
            replayed.send(handle, connection);
        });
    }

    fn replay(&mut self, replica: ReplicaId, replayed: Replayed) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        #[cfg(test)]
        if let Replayed::Moved { .. } = replayed {
            self.heads_told += 1;
        }
        if !matches!(
            replayed,
            Replayed::Sequenced { .. } | Replayed::Logged { .. }
        ) {
            open.edits_changed |= replayed.local_edit();
            open.replay.push(replayed.clone());
        }
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

    /// Attaches `editor` to `block`'s replica. One attached while the replica closes (its last
    /// editor left a moment before) has the block opened again once it closed, and follows the
    /// replica opened in its place, rather than one that is going.
    pub fn attach(&mut self, block: Uuid, editor: EditorId, editable: bool) {
        let Some((replica, open)) = self.replica_of(block) else {
            return;
        };
        if open.editors.get(&editor) == Some(&editable) {
            return;
        }
        if open.closing
            && let Some(open) = self.open.get_mut(&replica)
        {
            open.reopen = true;
            open.editors.insert(editor, editable);
            return;
        }
        self.attach_now(replica, editor, editable);
    }

    pub fn detach(&mut self, block: Uuid, editor: EditorId) -> bool {
        self.forwarded.remove(&(block, editor));
        let Some(replica) = self.blocks.get(&block).copied() else {
            return false;
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return false;
        };
        let Some(editable) = open.editors.remove(&editor) else {
            return open.editors.is_empty();
        };
        // Its open tentative gesture is cancelled, as its replica cancels it.
        let took = open.holds.detach(editor);
        open.sent += open.took(&took);
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

    /// What an app's editor sends its block's replica. Only the app whose module serves the
    /// block's type writes in its signed log: the host signs each entry as that app
    /// (`be_block_client::Said::hash`), so an `Append` from any other app's editor is answered
    /// `Appended` with an error and goes no further. The module is sent the message as the host
    /// read it, so what it acts on is what the host checked; one the host cannot read goes no
    /// further.
    pub fn relay_from_app(
        &mut self,
        shared: &Arc<Mutex<H>>,
        block: Uuid,
        editor: EditorId,
        app: &str,
        bytes: Vec<u8>,
        at: Instant,
    ) {
        let Some(message) = FromEditor::decode(&bytes) else {
            return;
        };
        if let FromEditor::Append { token, .. } = &message
            && let Some((_, open)) = self.replica_of(block)
            && self.modules.name(open.module) != app
        {
            let refused = be_sdk_type_guest::editor::ToEditor::Appended {
                token: *token,
                result: Err(format!(
                    "only {} writes in this block's log",
                    self.modules.name(open.module)
                )),
            };
            H::to_editor(shared, block, editor, refused.encode());
            return;
        }
        self.relay_from_editor(block, editor, message.encode(), at);
    }

    /// What an editor the host trusts sends its block's replica: the host's own, or one an app
    /// sent through `relay_from_app`.
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
        if operates(&bytes) {
            *self.forwarded.entry((block, editor)).or_default() += 1;
        }
        let at = self.modules.millis_at(at);
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        let took = open
            .holds
            .message(editor, FromEditor::decode(&bytes).as_ref());
        // Its module took the held operations that count now when it was sent them.
        open.sent += open.took(&took);
        let replayed = Replayed::Message {
            from: Endpoint::Editor(editor),
            bytes,
            at,
            held: match took.this {
                This::Held(serial) => Some(serial),
                This::Counted | This::Ended => None,
            },
        };
        // A commit or cancel that ends its gesture goes to the module, and is kept nowhere.
        match took.this {
            This::Ended => self.send_only(replica, &replayed),
            This::Counted | This::Held(_) => self.replay(replica, replayed),
        }
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
        if std::mem::replace(&mut open.closing, true) {
            return;
        }
        if open.joining {
            return self.let_go_if_held(replica);
        }
        self.with_replica(replica, |handle, connection| {
            handle.resource_drop(connection)
        });
    }

    /// A rejoin held for journaled edits no seal would take (`rejoin_held`) may hold until a
    /// status says one would: closing (or reopening) its block hands its answer back, and the
    /// join is let go of (`rejoined`). The answer goes back through the io channel, not `pending`,
    /// so the loop wakes for it (`woken`): nothing else would after a close.
    fn let_go_if_held(&mut self, replica: ReplicaId) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        if !rejoin_held(open) {
            return;
        }
        if let Some(io) = open.held.take() {
            self.holding.remove(&replica);
            let _ = self.io.send(io);
            H::wake();
        }
    }

    fn lock(&mut self, shared: &Arc<Mutex<H>>, block: Uuid) {
        self.locked.insert(block);
        H::set_locked(shared, block, true);
    }

    /// Keys arrived: kept edits set aside that read now are told so, and that they were not
    /// sent (`Peer::read_set_aside`). They never are: what they were made on may be gone.
    pub fn read_set_aside(&self, peer: &Peer<H::Store>, shared: &Arc<Mutex<H>>) {
        for (block, count) in peer.read_set_aside() {
            H::report(
                shared,
                Some(block),
                format!(
                    "{count} set-aside edits of this block can now be read; they were not sent"
                ),
            );
        }
    }

    /// The block's local edits were discarded with its unsent edits (`Peer::discard_unsent`):
    /// its open replica closes, and one opened next starts afresh under a new origin.
    pub fn stop_keeping(&mut self, block: Uuid) {
        self.close(block);
    }

    /// Keys this account may open arrived (`Event::KeysChanged`, after `Peer::keys_changed`):
    /// every block found locked is no longer, an open one reads again (`read_again`), a
    /// submission refused for a missing key is submitted again (`resubmit`), and relayed
    /// messages search for their keys again. Answers the locked blocks that are not open, for
    /// the host to open again if it still wants them.
    pub async fn unlock(&mut self, (peer, shared): Ctx<'_, H>) -> Vec<Uuid> {
        self.searched.clear();
        let mut closed = Vec::new();
        for block in std::mem::take(&mut self.locked) {
            H::set_locked(shared, block, false);
            match self.blocks.contains_key(&block) {
                true => self.read_again(block),
                false => closed.push(block),
            }
        }
        self.resubmit((peer, shared)).await;
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
        if std::mem::replace(&mut open.closing, true) {
            return;
        }
        if open.joining {
            return self.let_go_if_held(replica);
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
        let undo = matches!(change, Change::Undo);
        let replayed = Replayed::Change { change, op: None };
        let op = self
            .modules
            .call(module, |connection| replayed.send(handle, connection))
            .flatten()?;
        let open = self.open.get_mut(&replica)?;
        // An undo while a tentative layer is held cancels the layer, as its replica does once
        // its module has it.
        if undo {
            let took = open.holds.undo();
            open.took(&took);
        }
        open.sent += 1;
        open.edits_changed = true;
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

    /// Sends `change` as `change_later` does, and answers whether the type made it once the
    /// replica says (`Changed`), which the pump delivers: a caller that awaits it holds up
    /// nothing else. None when the block has no replica to send it to; a replica whose module
    /// restarts is sent it again and answers then.
    pub fn change_answered(
        &mut self,
        block: Uuid,
        change: Change,
    ) -> Option<impl Future<Output = bool> + 'static> {
        let (module, op) = self.change_now(block, change)?;
        let (answer, answered) = tokio::sync::oneshot::channel();
        self.replies.insert((module, op), answer);
        Some(async move {
            answered
                .await
                .ok()
                .and_then(|outcome| Ticket::<bool>::new(op).decode(outcome).ok())
                .unwrap_or(false)
        })
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
            && open.session.has(JOINING_OWNER)
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

    /// Keeps on the device each open block's local edits no journal holds yet, as they changed,
    /// so a replica opened after the host stops is given them again (`refs::OpenEdits`).
    async fn keep_edits(&mut self, peer: &Arc<Peer<H::Store>>) {
        #[cfg(test)]
        if self.hold_kept_edits {
            return;
        }
        for open in self.open.values_mut() {
            if !std::mem::take(&mut open.edits_changed) {
                continue;
            }
            let (kept, edits, submissions) = kept_edits(open);
            let through = kept.submitted_through;
            // Kept at the next pass: this one could not seal them (no key yet) or write.
            match peer
                .keep_open_edits(open.block, kept, edits, submissions)
                .await
            {
                Ok(()) => {
                    open.kept_submitted_through = through;
                    open.unkept_submissions = false;
                    open.keep_error = None;
                }
                Err(error) => {
                    open.edits_changed = true;
                    open.keep_error = Some(error.to_string());
                }
            }
        }
    }

    /// Whether a replica in a live session waits for the server to order edits it submitted, or
    /// one joining its session has local edits to submit once it has: a seal now would leave
    /// them out.
    pub fn ordering(&self) -> bool {
        self.open.values().any(|open| {
            let edits_to_give = open.joining && open.replay.iter().any(Replayed::local_edit);
            edits_to_give
                || (!open.made_up
                    && !open.joining
                    && !open.status.journaled
                    && !open.status.incompatible
                    && !open.status.diverged)
        })
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

    /// Each block's place in the account's layers of the reads kind `content_type` counts from
    /// (`reads`, each a layer's content bytes, lowest first): `be_block_content::Unread::places`,
    /// in bytes only the type reads.
    pub async fn places(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        reads: Vec<Vec<u8>>,
    ) -> Result<Vec<be_sdk_type_api::BlockPlace>, NotSearched> {
        let no_module =
            || NotSearched::Type(format!("no type module has the block type {content_type}"));
        self.kind(content_type).await.ok_or_else(no_module)?;
        let module = self.modules.module_of(content_type).ok_or_else(no_module)?;
        match self
            .ask(ctx, module, |connection| {
                replicas::places(connection, content_type, reads)
            })
            .await
        {
            Ok(Ok(places)) => Ok(places),
            Ok(Err(be_sdk_type_api::Failure::Malformed(error))) => Err(NotSearched::Type(error)),
            Ok(Err(be_sdk_type_api::Failure::UnknownType)) => Err(no_module()),
            Err(error) => Err(NotSearched::Type(format!(
                "the type module answered: {error}"
            ))),
        }
    }

    /// How many things in a commit of a block the account has not read, by the block's place
    /// (`places`; none: the type's default place): `be_block_content::Unread`. `content` is
    /// the commit as the type loads it (`Kind::load`), which a caller reads beforehand, away
    /// from the replicas.
    /// How many things in `block` the account has not read, by its place: its type counts its
    /// commit's `content` and, for a type with a signed log, the log's latest entries
    /// (`be_block_content::UNREAD_ENTRIES`), read here.
    pub async fn unread(
        &mut self,
        ctx: Ctx<'_, H>,
        block: Uuid,
        content_type: Uuid,
        content: be_sdk_type_api::Loaded,
        place: Option<Vec<u8>>,
        account: Uuid,
    ) -> Result<u32, NotSearched> {
        let no_module =
            || NotSearched::Type(format!("no type module has the block type {content_type}"));
        let kind = self.kind(content_type).await.ok_or_else(no_module)?;
        let log = match kind.log {
            true => ctx
                .0
                .read_log_tail(block, be_block_content::UNREAD_ENTRIES)
                .await
                .map_err(NotSearched::Read)?
                .entries
                .into_iter()
                .map(|logged| be_sdk_type_api::LogItem {
                    seq: logged.entry.seq,
                    id: logged.entry.id,
                    author: logged.entry.author,
                    arrival: logged.entry.arrival,
                    payload: logged.opened.map(|said| said.bytes),
                    signer: be_sdk_type_api::Signer::Unknown,
                })
                .collect(),
            false => Vec::new(),
        };
        let module = self.modules.module_of(content_type).ok_or_else(no_module)?;
        let scope = types::Scope::of([&content]);
        let (answer, mut scope) = self
            .ask_reading(ctx, module, scope, |connection| {
                replicas::unread(connection, content_type, content, place, log, account)
            })
            .await;
        if let Some(failed) = scope.take_failure() {
            return Err(NotSearched::Read(failed));
        }
        match answer {
            Ok(Ok(count)) => Ok(count),
            Ok(Err(be_sdk_type_api::Failure::Malformed(error))) => Err(NotSearched::Type(error)),
            Ok(Err(be_sdk_type_api::Failure::UnknownType)) => Err(no_module()),
            Err(error) => Err(NotSearched::Type(format!(
                "the type module answered: {error}"
            ))),
        }
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
                    Some(commit) => Some(peer.load_head(block, commit).await?.manifest),
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

    /// Who holds the lease of `block`'s session, by its membership.
    pub fn holder(&self, block: Uuid) -> Option<ClientId> {
        self.replica_of(block)
            .and_then(|(_, open)| open.session.holder())
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
    pub fn heard(&mut self, ctx: Ctx<'_, H>, event: &Event) {
        self.told(ctx, Told::Event(event.clone()));
    }

    /// Takes the head the server holds of `block`, read from its tree (`query.tree`): one
    /// another connection published, unless it is the head the replica has, published or was
    /// told already, which is no news.
    pub fn heard_head(&mut self, ctx: Ctx<'_, H>, block: Uuid, head: CommitId) {
        self.told(ctx, Told::Head { block, head });
    }

    fn told(&mut self, (peer, shared): Ctx<'_, H>, told: Told) {
        let block = match &told {
            Told::Event(
                Event::Membership { block, .. }
                | Event::Sealed { block, .. }
                | Event::Relayed { block, .. }
                | Event::Sequenced { block, .. },
            )
            | Told::Head { block, .. } => block,
            Told::Event(Event::Appended(appended)) => {
                return self.appended(peer, appended.clone());
            }
            Told::Event(_) => return,
        };
        if let Some(waiting) = self.unsealing.get_mut(block) {
            waiting.push_back(told);
            return;
        }
        let event = match told {
            Told::Head { block, head } => return self.moved_head(block, head),
            Told::Event(event) => event,
        };
        let event = &event;
        match event {
            Event::Membership { block, entry } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                if open.joining {
                    return open.early.push(Replayed::Membership {
                        entry: entry.clone(),
                    });
                }
                // A session the host made up hears nothing of the server's.
                if open.made_up {
                    return;
                }
                if !open.session.apply(entry) {
                    // One was lost: the replica hears the membership afresh on joining again.
                    if entry.index > open.session.index + 1 {
                        let rejoin = self.rejoins;
                        self.rejoin(peer, replica, rejoin);
                    }
                    return;
                }
                let replayed = Replayed::Lease {
                    session: open.session.clone(),
                };
                self.replay(replica, replayed);
            }
            Event::Sealed {
                block,
                commit,
                through,
                run,
            } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                let replayed = Replayed::Sealed {
                    commit: *commit,
                    through: *through,
                    run: *run,
                };
                if open.joining {
                    return open.early.push(replayed);
                }
                self.replay(replica, replayed);
            }
            Event::Relayed {
                block,
                from,
                payload,
                ..
            } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let bytes = match peer.unseal(*block, payload) {
                    Ok(bytes) => bytes,
                    Err(ClientError::Locked(key)) if !self.searched.contains(&key) => {
                        self.unsealing
                            .insert(*block, VecDeque::from([Told::Event(event.clone())]));
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
                    held: None,
                };
                if let Some(open) = self.open.get_mut(&replica)
                    && open.joining
                {
                    return open.early.push(replayed);
                }
                self.replay(replica, replayed);
            }
            Event::Sequenced { block, entry } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                // A made-up session takes only its own order: what a join it let go of was sent
                // before the leave reached the server is not its.
                if self
                    .open
                    .get(&replica)
                    .is_some_and(|open| open.made_up && !open.joining)
                {
                    return;
                }
                if let Some(open) = self.open.get_mut(&replica)
                    && !open.joining
                {
                    match open.heard_through {
                        // One was lost: the replica reads them again on joining afresh.
                        Some(through) if entry.sequence > through + 1 => {
                            let rejoin = self.rejoins;
                            return self.rejoin(peer, replica, rejoin);
                        }
                        Some(through) if entry.sequence <= through => return,
                        _ => open.heard_through = Some(entry.sequence),
                    }
                }
                let tag = self
                    .open
                    .get(&replica)
                    .map(|open| self.modules.tag(open.module));
                let Some(tag) = tag else {
                    return;
                };
                // This device keeps what it heard, to open on offline.
                let tag = be_block_store::Hash::from_bytes(tag.0);
                if let Err(error) = peer.heard_entry(*block, tag, entry.clone()) {
                    H::record(shared, error);
                }
                let entry = match peer.open_entry(*block, entry.clone()) {
                    Ok(entry) => api_entry(entry),
                    Err(ClientError::Locked(key)) if !self.searched.contains(&key) => {
                        self.unsealing
                            .insert(*block, VecDeque::from([Told::Event(event.clone())]));
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
                let replayed = Replayed::Sequenced {
                    entries: vec![entry],
                };
                if let Some(open) = self.open.get_mut(&replica)
                    && open.joining
                {
                    return open.early.push(replayed);
                }
                self.replay(replica, replayed);
            }
            _ => {}
        }
    }

    /// Takes an append to the signed log of an open block (`Peer::logged`, which proves it
    /// extends the head this device holds) and hands the replica the entries it verified, as
    /// the app that wrote them.
    /// Follows the signed log of each open block of a type with one again, as the device
    /// connects: a follow ends with the connection it was made on, and one a replica's editors
    /// asked for offline was never made. Each reads on from the head this device kept, so what
    /// was appended meanwhile (this device's own entries the outbox drained among them) reaches
    /// the replica as appends told do.
    pub fn follow_logs(&mut self, (peer, _): Ctx<'_, H>) {
        for open in self.open.values() {
            if !crate::kind_of(open.content_type).is_some_and(|kind| kind.log) {
                continue;
            }
            let (block, app) = (open.block, self.modules.name(open.module).to_owned());
            let (peer, io) = (Arc::clone(peer), self.io.clone());
            H::spawn(async move {
                let from = peer.log_head(block).map_or(0, |head| head.size);
                let Ok(read) = peer.read_log(block, from, u32::MAX).await else {
                    return;
                };
                let items = log_items(&peer, block, &app, read.entries).await;
                if !items.is_empty() {
                    let _ = io.send(Io::Logged { block, items });
                    H::wake();
                }
            });
        }
    }

    fn appended(&mut self, peer: &Arc<Peer<H::Store>>, appended: be_block_protocol::Appended) {
        let block = appended.block;
        let Some(module) = self
            .blocks
            .get(&block)
            .and_then(|replica| self.open.get(replica))
            .map(|open| open.module)
        else {
            return;
        };
        let app = self.modules.name(module).to_owned();
        let (peer, io) = (Arc::clone(peer), self.io.clone());
        H::spawn(async move {
            let items = match peer.logged(appended).await {
                Ok(taken) => log_items(&peer, block, &app, taken).await,
                Err(_) => return,
            };
            if !items.is_empty() {
                let _ = io.send(Io::Logged { block, items });
                H::wake();
            }
        });
    }

    fn moved_head(&mut self, block: Uuid, head: CommitId) {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return;
        };
        if let Some(open) = self.open.get_mut(&replica) {
            // A change that left the head as the replica has it, or as it published or was
            // already told it, is no news of the head.
            if open.heard == Some(head)
                || open.published.contains(&head)
                || open.moved.contains(&head)
            {
                return;
            }
            if open.joining {
                open.heard = Some(head);
                return open.early.push(Replayed::Moved { head });
            }
        }
        self.moved(replica, head, false);
    }

    /// The host fell behind what the server sent: every replica that is not joining joins its
    /// session again and opens again on the journal, so none misses an entry of the order.
    pub fn resynchronize(&mut self, (peer, _): Ctx<'_, H>) {
        // A search that ended as the connection dropped answered nothing, and the server may
        // have keys now that it lacked: what is relayed from here on is searched for again.
        self.searched.clear();
        self.rejoins += 1;
        let rejoin = self.rejoins;
        for replica in self
            .open
            .iter()
            .filter(|(_, open)| !open.joining)
            .map(|(replica, _)| *replica)
            .collect::<Vec<_>>()
        {
            self.rejoin(peer, replica, rejoin);
        }
    }

    fn claimed(&mut self, shared: &Arc<Mutex<H>>, claimed: Result<(), ClientError>) {
        match claimed {
            Ok(()) => {}
            Err(error) if unreachable(&error) => {}
            // Only an editor may hold a block's lease; a viewer's claim changes nothing.
            Err(ClientError::Refused(ErrorCode::PermissionDenied, _)) => {}
            Err(error) => H::record(shared, error),
        }
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
                let submitted = matches!(*answer, Answered::Submit(..));
                let delivered = self.modules.generation(module) == generation;
                if delivered {
                    self.modules
                        .call(module, |connection| answer.send(connection));
                }
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                if submitted {
                    open.submitting = open.submitting.saturating_sub(1);
                }
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
            Io::Logged { block, items } => {
                if let Some(replica) = self.blocks.get(&block).copied() {
                    self.replay(replica, Replayed::Logged { items });
                }
            }
            Io::Relayed => self.relaying -= 1,
            Io::Left(block) => {
                self.left -= 1;
                if let Some(count) = self.left_blocks.get_mut(&block) {
                    *count -= 1;
                    if *count == 0 {
                        self.left_blocks.remove(&block);
                    }
                }
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
                for (_, waiting) in std::mem::take(&mut self.unsealing) {
                    for told in waiting {
                        self.told((peer, shared), told);
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
            Io::Claimed { claimed } => self.claimed(shared, claimed),
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
    ///
    /// A rejoin whose seal `reconnect` asked as it began (`sealed_before_joining`), or that
    /// sealed once, and still holds journaled edits no seal holds, which a seal cannot take now
    /// (`owes_seal` is false: a reload is on its way, its seal backs off, or a merge cannot read
    /// its parts), is settling too: its answer stays held, asking nothing, until a status says a
    /// seal would take them (`rejoined`), or its block closes. A replica in its made-up session
    /// never diverges (the session speaks its own build), and its seal goes to the outbox
    /// without merging (`merge_diverged` merges it once a drain sends it), so neither holds a
    /// rejoin.
    fn settling(&self, replica: ReplicaId) -> bool {
        self.open.get(&replica).is_some_and(|open| {
            open.publishing > 0
                || open.submitting > 0
                || open.reporting.is_some()
                || (rejoin_held(open) && !open.closing)
        }) || self
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
        open.joined_settled = open.made_up
            && open.status.clean
            && !open.unreported()
            && peer.pending_commits(open.block).is_empty();
        open.sealed_joining = false;
        open.sealed_before_joining =
            open.made_up && (!open.status.clean || open.unreported()) && !open.joined_settled;
        open.held = None;
        open.early.clear();
        let block = open.block;
        let tag = be_block_store::Hash::from_bytes(self.modules.tag(open.module).0);
        let peer = Arc::clone(peer);
        let io = self.io.clone();
        H::spawn(async move {
            let joined = async {
                let (client, session, journal) = peer.join_session(block, tag).await?;
                let head = peer.remote_head(block).await?;
                // The head may be a seal of the journal (`journal_for`), which needs the commit.
                if let Some(head) = head {
                    fetched_for_join(&peer, head).await?;
                }
                let journal = match journal {
                    Some(journal) => Some(journal),
                    None => started_from_mirror(&peer, block, tag, client, &session, head).await,
                };
                let journal = opened(&peer, block, journal);
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
        // What its made-up session took while the join was on its way, after `created` asked a
        // seal of what it held, is committed here first. Reopening gives again only the local
        // edits its order does not hold (`reopening` cuts `replay` by `journaled_taken`), and the
        // server's journal holds none of its made-up session's: those journaled and unsealed
        // would be lost. The answer comes back here once the seal is answered, and another is
        // asked only while some are left and a seal would take them; while none would, the
        // answer stays held (`settling`). An edit it has not submitted (it waits for the order to
        // number the one before) is not journaled: `replay` gives it again, and no seal commits
        // it, so asking again for it would never end. A join whose seal `reconnect` asked as it
        // began (`sealed_before_joining`) waits alike while that seal failed and backs off.
        let unsealed = match open.sealed_joining || open.sealed_before_joining {
            false => open.joined_settled && (!open.status.clean || open.unreported()),
            true => journaled_unsealed(open) && open.status.owes_seal,
        };
        if unsealed {
            open.sealed_joining |= open.joined_settled;
            self.seal_later(block, false);
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
        let letting_go = open.closing && rejoin_held(open);
        open.joining = false;
        open.rejoining = None;
        let early = std::mem::take(&mut open.early);
        // Closed (or reopened) while its rejoin was held (`rejoin_held`): it lets go of the join
        // rather than open again on the server's journal, which would lose the journaled edits
        // no seal holds, and ends, leaving the session (`ended`): the replica seals as it closes,
        // so the commit it leaves holds them (a hold for a seal backing off, or a reload on its
        // way). Its seal goes to the outbox, and `merge_diverged` merges it once a drain sends it
        // (decisions/journaled_edits_reach_the_outbox_before_a_join.md). What the session said
        // meanwhile is not its own.
        if letting_go {
            return self.close_if_asked(replica);
        }
        // What its made-up session sealed while the join was on its way is a commit the server
        // lacks, which the server's journal does not reach: as `created` asks, it joins once a
        // drain sent that, and lets go of this join meanwhile. Commits it held before joining
        // are its head, which the replica reconciles with on reopening.
        if open.made_up && open.sealed_joining && !peer.pending_commits(block).is_empty() {
            // The connection joined: it leaves, so it holds no lease nor hears the session.
            self.leave(shared, peer, block);
            self.uncreated.insert(block);
            for replayed in early {
                self.replay(replica, replayed);
            }
            self.close_if_asked(replica);
            return;
        }
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
        let mut session = session;
        let early: Vec<Replayed> = early
            .into_iter()
            .filter(|replayed| match replayed {
                Replayed::Membership { entry } => {
                    session.apply(entry);
                    false
                }
                Replayed::Lease { .. } => false,
                _ => true,
            })
            .collect();
        // The join answered whether this account may edit it (`Peer::join_session`).
        open.marked = peer.discarded(block);
        if let Err(error) = mirror(peer, block, journal.as_ref(), true, open.status.head) {
            H::record(shared, error);
        }
        let loads = reopening(open, peer, journal, true, open.status.head);
        let known = loads.2;
        let (ordered, early): (Vec<Replayed>, Vec<Replayed>) = early
            .into_iter()
            .partition(|replayed| matches!(replayed, Replayed::Sequenced { .. }));
        self.reopen_joined(replica, client, session, head, loads, commits_kept, ordered);
        self.tell_marked(shared, replica);
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

    /// Opens every replica of `module` again after it restarted, on this device's journal, and
    /// joins a live session again so the replica takes the server's journal: this device keeps
    /// only what it heard, which another device's seals never trimmed.
    pub async fn restart(&mut self, peer: &Arc<Peer<H::Store>>, module: usize) {
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
            let (journal, journal_kept, head, sealed_head) =
                reopening(open, peer, None, false, open.status.head);
            open.resent();
            let kept = kept_commits(peer, open.block);
            let opening = Opening {
                block: open.block,
                content_type: open.content_type,
                organization: open.organization,
                account: open.account,
                client: open.client,
                tag,
                origin: open.origin,
                taken_before: open.given_up,
                // Offline, what it submitted online may be in the order: it only shows.
                doubtful_through: match !open.made_up && !open.joining {
                    true => 0,
                    false => open.submitted_through,
                },
                kept_restart: open.restarts,
                kept_through: open.kept_through,
                kept_submitted: open.submitted_through,
                kept_submissions: open.kept_submissions(),
                head,
                session: open.session.clone(),
                lost_edits: false,
                journal: journal.and_then(|journal| replayable(journal, tag, head)),
                journal_kept,
                commits_kept: kept.0,
                refused_commits: kept.1,
                server_head: peer.known_remote_head(open.block),
                sealed_head,
                epoch: open.epoch,
            };
            let editors: Vec<(EditorId, bool, u64)> = open
                .editors
                .iter()
                .map(|(editor, editable)| {
                    let held = acknowledged(&self.forwarded, open.block, &open.replay, *editor);
                    (*editor, *editable && !open.marked, held)
                })
                .collect();
            let closing = open.closing && !open.joining;
            let asked = asked.remove(&replica).unwrap_or_default();
            // In a live session it is opened again on the server's journal once it rejoined,
            // and only then given its edits: one still on its way to the server when the
            // module stopped would otherwise be made again before the order shows it.
            let rejoins = !open.made_up && !open.joining;
            let mut none = Vec::new();
            let (replies, pending, replay, departed) = (
                &mut self.replies,
                &mut self.asked,
                match rejoins {
                    true => &mut none,
                    false => &mut open.replay,
                },
                &open.departed,
            );
            #[cfg(test)]
            note_opening(&mut self.openings, &opening);
            let Some(handle) = self.modules.call(module, |connection| {
                let handle = Replica::new(connection, opening);
                for &(editor, editable, held) in &editors {
                    handle.attach(connection, editor, editable, held);
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
                for &(editor, ..) in &editors {
                    handle.resent(connection, editor);
                }
                if closing {
                    handle.resource_drop(connection);
                }
                handle
            }) else {
                continue;
            };
            open.handle = handle;
            self.handles.insert((module, handle.op()), replica);
            if rejoins {
                let rejoin = self.rejoins;
                self.rejoin(peer, replica, rejoin);
            }
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
        // The order started over since the submissions kept were made: they are passed over,
        // and the edits go again as new submissions, which hold them otherwise. A replica
        // reports a restart before it submits again (`Replica::flush`), so those are kept from
        // here. One that reports fewer restarts has not loaded yet.
        if status.restarts > open.restarts {
            open.restarts = status.restarts;
            if !open.submissions.is_empty() {
                open.submissions.clear();
                open.edits_changed = true;
            }
        }
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
        let Some(mut open) = self.open.remove(&replica) else {
            return false;
        };
        // What opens after it holds the local edits the order holds, as a reopen counts them
        // (`reopening`).
        let held = open.journaled_taken.max(open.status.sealed_taken);
        untaken(
            &mut self.forwarded,
            open.block,
            (open.sent, held),
            &open.replay,
        );
        // Kept on the device only as far as nothing holds them: none, once it ended clean.
        open.reported_clean(held);
        let (kept, edits, submissions) = kept_edits(&open);
        let _ = peer
            .keep_open_edits(open.block, kept, edits, submissions)
            .await;
        self.asked.retain(|_, (to, _)| *to != replica);
        self.blocks.remove(&open.block);
        if !open.joining {
            self.leave(shared, peer, open.block);
        }
        if open.reopen {
            let block = open.block;
            // The editors it had follow the replica opened in its place, or hear why none did.
            let editors = open
                .editors
                .iter()
                .filter(|(editor, _)| **editor != HOST_EDITOR)
                .map(|(editor, editable)| (*editor, *editable));
            match self
                .open((peer, shared), block, open.content_type, open.reader)
                .await
            {
                // Editors attached to a block that opened no replica (it waits for keys or a
                // connection) hear nothing until one opens.
                Ok(()) => {
                    for (editor, editable) in editors {
                        self.attach(block, editor, editable);
                    }
                }
                Err(reason) => {
                    let failed = be_sdk_type_guest::editor::ToEditor::Failed(reason).encode();
                    for (editor, _) in editors {
                        H::to_editor(shared, block, editor, failed.clone());
                    }
                }
            }
            // What the ended replica held is gone unless one opened in its place (it may wait
            // for keys or a connection, or have failed), and either way it is no longer counted.
            if !self.blocks.contains_key(&block) {
                H::released(shared, block);
            }
            self.counted(shared);
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

    /// Hands a replica's submission to the block's order (`Peer::submit`), answering the replica
    /// once the server numbered it, or at once when offline or when it fails. One refused for a
    /// key this device does not hold yet (a share's grant reached it before the key, or before
    /// the contact that sealed it) is held unanswered and the block taken for locked, until keys
    /// arrive (`unlock`): the session is fine, so nothing would open the replica again to give
    /// it again. Keys that arrived while it was refused submit it again at once.
    #[allow(clippy::too_many_arguments)]
    async fn submit(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        module: usize,
        replica: ReplicaId,
        reply: Reply<be_sdk_type_api::host::Submitted>,
        submission: be_block_session::Submission,
        announcements: Vec<be_block_content::Announcement>,
    ) {
        let Some((block, made_up)) = self
            .open
            .get(&replica)
            .map(|open| (open.block, open.made_up))
        else {
            return;
        };
        let keys: std::collections::HashSet<String> = announcements
            .iter()
            .map(|announcement| announcement.key.clone())
            .collect();
        let tag = be_block_store::Hash::from_bytes(self.modules.tag(module).0);
        let arrivals = peer.arrivals();
        let online = !made_up && peer.is_attached();
        if online
            && let Some(submitted) = be_sdk_type_guest::wire::Submitted::decode(&submission.payload)
            && let Some(open) = self.open.get_mut(&replica)
        {
            if submitted.through > open.submitted_through {
                open.submitted_through = submitted.through;
                open.edits_changed = true;
            }
            // An operation is kept as it went, so a replica opened again submits its edits
            // under the same id; given again, it is kept already.
            let counter = submission.id.counter;
            if matches!(
                submitted.entry,
                be_sdk_type_guest::wire::Entry::Operation(_)
            ) && !open.submissions.iter().any(|kept| kept.counter == counter)
            {
                open.submissions.retain(|kept| kept.through > open.given_up);
                open.submissions.push(be_block_client::KeptSubmission {
                    counter,
                    through: submitted.through,
                    payload: submission.payload.clone(),
                });
                open.edits_changed = true;
                open.unkept_submissions = true;
            }
        }
        // Written ahead: once the server may hold an edit, a crash must not lose that it was
        // submitted, or under what id, or the edits given again would be numbered anew.
        let mut kept = Ok(());
        if let Some(open) = self.open.get_mut(&replica)
            && (open.unkept_submissions || open.submitted_through > open.kept_submitted_through)
        {
            let (held, edits, submissions) = kept_edits(open);
            let through = held.submitted_through;
            kept = peer.keep_open_edits(block, held, edits, submissions).await;
            if kept.is_ok()
                && let Some(open) = self.open.get_mut(&replica)
            {
                open.kept_submitted_through = through;
                open.unkept_submissions = false;
                open.edits_changed = false;
            }
        }
        let submitted = match kept {
            Err(error) if !matches!(error, ClientError::Locked(_)) => {
                let failed = be_sdk_type_api::host::Submitted::Failed(error.to_string());
                self.modules
                    .call(module, |connection| reply.send(connection, &failed));
                return;
            }
            Ok(()) => {
                peer.submit(
                    block,
                    tag,
                    submission.clone(),
                    online,
                    announcements.clone(),
                )
                .await
            }
            Err(error) => Err(error),
        };
        match submitted {
            Ok(be_block_client::peer::Submitted::Server(answer)) => {
                self.relaying += 1;
                if let Some(open) = self.open.get_mut(&replica) {
                    open.submitting += 1;
                }
                let (shared, io) = (Arc::clone(shared), self.io.clone());
                let peer = Arc::clone(peer);
                self.answer_later(replica, module, async move {
                    let answered = match answer.await {
                        Ok(()) => {
                            if !keys.is_empty() {
                                match peer.submitted_held(block, &keys) {
                                    Ok(()) => send_announcements::<H>(&peer, &shared).await,
                                    Err(error) => H::record(&shared, error),
                                }
                            }
                            be_sdk_type_api::host::Submitted::Held
                        }
                        // This account may not edit the block: sent again it would be refused
                        // again, so the replica gives the edit up, and what it announces is never
                        // sent. The server's refusal and a local `OpError::Denied` both arrive as
                        // this (`ClientError::from`), and both mean give it up.
                        Err(error @ ClientError::Refused(ErrorCode::PermissionDenied, _)) => {
                            if !keys.is_empty()
                                && let Err(error) = peer.submitted_refused(block, &keys)
                            {
                                H::record(&shared, error);
                            }
                            let refused =
                                be_sdk_type_api::host::Submitted::Refused(error.to_string());
                            H::record_sync(&shared, error);
                            refused
                        }
                        Err(error) => {
                            let failed =
                                be_sdk_type_api::host::Submitted::Failed(error.to_string());
                            if !unreachable(&error) {
                                H::record_sync(&shared, error);
                            }
                            failed
                        }
                    };
                    let _ = io.send(Io::Relayed);
                    (Answered::Submit(reply, answered), None)
                });
            }
            // Numbered in this device's own order: the replica hears it at once.
            Ok(be_block_client::peer::Submitted::Offline(entry)) => {
                let entry = api_entry(entry);
                self.replay(
                    replica,
                    Replayed::Sequenced {
                        entries: vec![entry],
                    },
                );
                self.modules.call(module, |connection| {
                    reply.send(connection, &be_sdk_type_api::host::Submitted::Kept)
                });
            }
            Err(ClientError::Locked(_)) => {
                let Some(epoch) = self.open.get(&replica).map(|open| open.epoch) else {
                    return;
                };
                self.refused.push(Refused {
                    replica,
                    module,
                    generation: self.modules.generation(module),
                    epoch,
                    reply,
                    submission,
                    announcements,
                });
                match peer.arrivals() == arrivals {
                    true => self.lock(shared, block),
                    // Keys arrived while it was refused, after `unlock` ran: it goes again.
                    false => Box::pin(self.resubmit((peer, shared))).await,
                }
            }
            Err(error) => {
                let failed = be_sdk_type_api::host::Submitted::Failed(error.to_string());
                if !unreachable(&error) {
                    H::record(shared, error);
                }
                self.modules
                    .call(module, |connection| reply.send(connection, &failed));
            }
        }
    }

    /// Submits again what was refused for a missing key (`Refused`), once keys arrived. One
    /// whose replica opened again since is let go unanswered, as that opening gave its edits
    /// again; one whose replica or module is gone is dropped with it.
    async fn resubmit(&mut self, ctx: Ctx<'_, H>) {
        for refused in std::mem::take(&mut self.refused) {
            if self.modules.generation(refused.module) != refused.generation {
                continue;
            }
            // One whose replica opened again is dropped: the reopening gave its edits again, and
            // the dropped reply answers `Unanswered`, which the reopened replica ignores as from
            // an earlier opening (its epoch).
            if self.open.get(&refused.replica).map(|open| open.epoch) == Some(refused.epoch) {
                self.submit(
                    ctx,
                    refused.module,
                    refused.replica,
                    refused.reply,
                    refused.submission,
                    refused.announcements,
                )
                .await;
            }
        }
    }

    async fn act(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        module: usize,
        replica: ReplicaId,
        call: Call,
    ) -> bool {
        let Some((block, content_type, made_up, joining, reach)) =
            self.open.get(&replica).map(|open| {
                (
                    open.block,
                    open.content_type,
                    open.made_up,
                    open.joining,
                    Arc::clone(&open.reach),
                )
            })
        else {
            return false;
        };
        match call {
            Call::Clock { .. } | Call::SleepUntil { .. } | Call::InStepAt { .. } => false,
            Call::Send { reply, to, bytes } => {
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
                        self.relay(shared, peer.relay(block, Some(client), &bytes).await);
                        false
                    }
                    Recipients::Peers => {
                        self.relay(shared, peer.relay(block, None, &bytes).await);
                        false
                    }
                }
            }
            Call::Submit {
                reply,
                submission,
                announcements,
            } => {
                let submission = be_block_session::Submission {
                    id: be_block_session::OpId {
                        origin: submission.origin,
                        counter: submission.counter,
                    },
                    base: submission.base,
                    barrier: submission.barrier,
                    payload: submission.payload,
                };
                let announcements = announcements.into_iter().map(announcement).collect();
                self.submit(
                    (peer, shared),
                    module,
                    replica,
                    reply,
                    submission,
                    announcements,
                )
                .await;
                false
            }
            Call::RebaseJournal {
                reply,
                base,
                restarts,
            } => {
                self.acknowledge(module, reply);
                let tag = be_block_store::Hash::from_bytes(self.modules.tag(module).0);
                let base = be_block_protocol::JournalBase {
                    commit: base.commit,
                    sequence: base.sequence,
                    state: base.state,
                };
                // The server's journal takes it from the lease holder alone, which it checks
                // itself; one the module made in a session the host made up (offline, or while
                // joining) is kept on this device only.
                let trim = !made_up && peer.is_attached();
                // An own commit: one a replica of this host sealed, or one this device's outbox
                // keeps.
                let own = base.commit.is_some_and(|commit| {
                    self.sealed
                        .get(&block)
                        .is_some_and(|sealed| sealed.contains(&commit))
                        || peer.pending_commits(block).contains(&commit)
                });
                let rebased = peer
                    .rebase_journal(block, tag, restarts, base, trim, own)
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
                        let loaded = peer.load_head(block, commit).await?;
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
                    if let Ok(loaded) = &result {
                        reach
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .hand(loaded.parts.iter().map(|part| part.part.into()));
                    }
                    (Answered::Load(reply, result), None)
                });
                false
            }
            Call::ReadPart { reply, part, range } => {
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                self.answer_later(replica, module, async move {
                    let arrivals = peer.arrivals();
                    let part = part.into();
                    let read = match reach::reaches(&reach, &peer, block, part).await {
                        Ok(true) => Some(peer.read_part(part, range.offset, range.length).await),
                        Ok(false) => None,
                        Err(error) => Some(Err(error)),
                    };
                    if let Some(Err(ClientError::Locked(_))) = &read {
                        let _ = io.send(Io::Locked { replica, arrivals });
                    }
                    let result = match read {
                        Some(read) => read.map_err(|error| error.to_string()),
                        None => Err(OUTSIDE_REACH.to_owned()),
                    };
                    (Answered::ReadPart(reply, result), None)
                });
                false
            }
            Call::Append {
                reply,
                bytes,
                along,
            } => {
                let along = be_block_content::Along {
                    announcement: along.announcement.map(announcement),
                    redacts: along.redacts,
                };
                let peer = Arc::clone(peer);
                let app = self.modules.name(module).to_owned();
                self.answer_later(replica, module, async move {
                    let appended = match peer.append(block, &app, &bytes, along).await {
                        Ok(be_block_client::Appended::Logged { id, seq, arrival }) => {
                            Ok(be_sdk_type_api::host::Appended {
                                id,
                                place: Some(be_sdk_type_api::host::LogPlace { seq, arrival }),
                            })
                        }
                        Ok(be_block_client::Appended::Kept { id }) => {
                            Ok(be_sdk_type_api::host::Appended { id, place: None })
                        }
                        Err(error) => Err(error.to_string()),
                    };
                    (Answered::Append(reply, appended), None)
                });
                false
            }
            Call::ReadLog { reply, from, limit } => {
                let peer = Arc::clone(peer);
                let app = self.modules.name(module).to_owned();
                self.answer_later(replica, module, async move {
                    let read = async {
                        let read = peer.read_log(block, from, limit).await?;
                        Ok::<_, ClientError>(be_sdk_type_api::LogPage {
                            size: read.head.size,
                            items: log_items(&peer, block, &app, read.entries).await,
                            kept: peer
                                .kept_entries(block)
                                .into_iter()
                                .map(|(id, payload)| be_sdk_type_api::KeptEntry { id, payload })
                                .collect(),
                        })
                    }
                    .await
                    .map_err(|error| error.to_string());
                    (Answered::ReadLog(reply, read), None)
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
                    if let Ok(put) = &put {
                        reach
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .hand([*put]);
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
                journal_state,
                restarts,
            } => {
                let Some(open) = self.open.get_mut(&replica) else {
                    return false;
                };
                let tag = be_block_store::Hash::from_bytes(self.modules.tag(module).0);
                let shared = Arc::clone(shared);
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
                #[cfg(test)]
                let failing = match self.failing_seals.checked_sub(1) {
                    Some(left) => {
                        self.failing_seals = left;
                        true
                    }
                    None => false,
                };
                #[cfg(not(test))]
                let failing = false;
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
                    .sealed_through(
                        sealed_through.map(|sequence| be_block_commit::SealedThrough {
                            sequence,
                            offline: made_up,
                            restarts,
                        }),
                    )
                    // The server's journal starts from a seal of its session as the seal wins
                    // the head; one made in a session the host made up is this device's alone.
                    .seals_journal(
                        sealed_through
                            .zip(journal_state.clone())
                            .filter(|_| !made_up)
                            .map(|(through, state)| be_block_client::SealsJournal {
                                tag,
                                run: restarts,
                                through,
                                state,
                            }),
                    );
                    if let Some(merging) = merging {
                        write = write.merging(merging.base, merging.ours);
                    }
                    if let Some(label) = &bookmark {
                        write = write.bookmark(label);
                    }
                    let saved = match reach::reaches_all(&reach, &peer, block, &content.parts).await
                    {
                        Ok(true) if failing => Some(Err(ClientError::Locked(block))),
                        Ok(true) => Some(
                            commit::<H>(&peer, block, write, local, &publishing, move || {
                                let _ = sealed.send(Io::Sealed);
                            })
                            .await,
                        ),
                        Ok(false) => None,
                        Err(error) => Some(Err(error)),
                    };
                    // This device's journal starts from the seal too; with nothing to commit, the
                    // server's is trimmed with no new commit.
                    if let (
                        Some(Ok(
                            Saved::Published(head) | Saved::Kept(head) | Saved::Unchanged(head),
                        )),
                        Some(sequence),
                        Some(state),
                    ) = (&saved, sealed_through, journal_state)
                    {
                        let base = be_block_protocol::JournalBase {
                            commit: Some(*head),
                            sequence,
                            state,
                        };
                        let trim = matches!(saved, Some(Ok(Saved::Unchanged(_))))
                            && !made_up
                            && peer.is_attached();
                        match peer
                            .rebase_journal(block, tag, restarts, base, trim, true)
                            .await
                        {
                            Ok(Some(trimmed)) => {
                                if let Err(error) = trimmed.await
                                    && !unreachable(&error)
                                {
                                    H::record(&shared, error);
                                }
                            }
                            Ok(None) => {}
                            Err(error) if unreachable(&error) => {}
                            Err(error) => H::record(&shared, error),
                        }
                        send_announcements::<H>(&peer, &shared).await;
                    }
                    let kept = publishing
                        .kept()
                        .is_some_and(|commit| peer.pending_commits(block).contains(&commit));
                    let (result, published) = match saved {
                        None => (Err(OUTSIDE_REACH.to_owned()), None),
                        Some(Ok(Saved::Published(head))) => {
                            (Ok(PublishOutcome::Published(head)), Some(head))
                        }
                        // Kept in the outbox, or nothing to seal on a head that waits there.
                        Some(Ok(Saved::Kept(head) | Saved::Unchanged(head)))
                            if peer.pending_commits(block).contains(&head) =>
                        {
                            (Ok(PublishOutcome::Kept(head)), None)
                        }
                        // Kept, and a drain has sent it since; or nothing to seal.
                        Some(Ok(Saved::Kept(head) | Saved::Unchanged(head))) => {
                            (Ok(PublishOutcome::Published(head)), None)
                        }
                        Some(Ok(Saved::Rejected { head })) => {
                            (Ok(PublishOutcome::Rejected(head)), None)
                        }
                        Some(Err(error)) => (Err(error.to_string()), None),
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
                    let stashed =
                        match reach::reaches_all(&reach, &peer, block, &content.parts).await {
                            Ok(true) => {
                                let write = Write::encoded(
                                    content_type,
                                    be_sdk_type_api::written_parts(&content.bytes, &content.parts),
                                    parent,
                                )
                                .described(content.references, content.derived);
                                peer.stash(block, write)
                                    .await
                                    .map_err(|error| error.to_string())
                            }
                            Ok(false) => Err(OUTSIDE_REACH.to_owned()),
                            Err(error) => Err(error.to_string()),
                        };
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
                        let resumed = be_block_session::resume(peer.commits(), local, remote)?;
                        Ok::<_, ClientError>(sealed_here(&peer, local, resumed))
                    }
                    .await
                    .map_err(|error| error.to_string());
                    (Answered::Resume(reply, resumed), None)
                });
                false
            }
            Call::Claim { reply } => {
                self.acknowledge(module, reply);
                if !peer.is_attached() || made_up {
                    return false;
                }
                let claiming = peer.connection().claim(block);
                let io = self.io.clone();
                H::spawn(async move {
                    let claimed = claiming.await;
                    let _ = io.send(Io::Claimed { claimed });
                    H::wake();
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
    /// hold edits that are not yet durable: not yet kept on this device, in a journal or in its
    /// kept edits (`refs::OpenEdits`). A kept edit survives a restart, so it is only waiting for
    /// the server and its commit.
    pub fn counts(&self) -> (Vec<Owing>, Vec<Owing>, Vec<Owing>) {
        let now = self.modules.now();
        let unsealed = self
            .open
            .values()
            .filter(|open| !open.status.clean || open.unreported())
            .map(|open| Owing::of(open, now))
            .collect();
        let owing = self
            .open
            .values()
            .filter(|open| open.status.owes_seal)
            .map(|open| Owing::of(open, now))
            .collect();
        let undurable = self
            .open
            .values()
            // Not yet kept on this device: neither a journal holds them nor are they written in
            // its kept edits (`keep_edits`, or ahead of a submission), which a crash survives.
            .filter(|open| {
                open.edits_changed
                    && (open.unreported() || (!open.status.clean && !open.status.journaled))
            })
            .map(|open| Owing::of(open, now))
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

/// Whether this device's journal of `block` holds entries it numbered offline, which the
/// server lacks.
fn numbered_here<S: be_block_store::ObjectStore>(peer: &Peer<S>, block: Uuid) -> bool {
    peer.sealed_journal(block)
        .is_some_and(|journal| journal.entries.iter().any(|entry| entry.received.is_none()))
}

/// Joins `block`'s session, for a replica that shows before it has (`Replicas::open`).
fn joining<S: be_block_store::ObjectStore + 'static>(
    peer: &Arc<Peer<S>>,
    block: Uuid,
    content_type: Uuid,
    tag: Option<be_block_store::Hash>,
) -> Pending<Joined> {
    let peer = Arc::clone(peer);
    Box::pin(async move {
        peer.ensure_type(block, content_type, BlockParent::Detached)
            .await?;
        let tag = tag.ok_or(ClientError::Unexpected)?;
        let (client, session, journal) = peer.join_session(block, tag).await?;
        let head = peer.remote_head(block).await?;
        // The head may be a seal of the journal (`journal_for`), which needs the commit here.
        if let Some(head) = head {
            fetched_for_join(&peer, head).await?;
        }
        let journal = match journal {
            Some(journal) => Some(journal),
            None => started_from_mirror(&peer, block, tag, client, &session, head).await,
        };
        Ok((
            client,
            session,
            head,
            ahead(&peer, block),
            opened(&peer, block, journal),
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

/// What a replica opening at `head` loads, in the first of these that holds:
/// - The journal is on `head`: that journal on `head`.
/// - `head` is a seal made on the commit the journal is still on (its holder stopped after the
///   server took the seal and before it trimmed the journal, `Commit::sealed_through`) and the
///   journal reaches it (`reaches_seal`): that journal on that commit, with `head` as the sealed
///   head the replica takes once it replayed it.
/// - For the server's journal, such a seal is an ancestor of `head` along seals (the server lost
///   the seal's rebase, and the outbox published the seal and those made on it since): the same,
///   with that seal as the sealed head; the replica hears of `head` after. Leases from the
///   order closed the cause; the case stays until no path is shown to reach it
///   (plans/backlog.md, "A lost seal's rebase with a merge above it").
/// - The server's journal otherwise: that journal on its own base, which the replica opens on
///   because the order goes on from there (it hears of `head` after, and its holder merges it
///   in).
/// - This device's journal on another commit is passed over: what it numbered offline was
///   sealed into the commits this device keeps.
///
/// Online, the server's journal is the order: with none, the order starts afresh at its head,
/// so this device's journal is passed over too, unless it holds edits only this device
/// numbered, which a replica opened on it seals.
pub fn journal_for<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    server: Option<be_block_protocol::Journal>,
    online: bool,
    head: Option<CommitId>,
) -> Loads {
    let device = || {
        peer.local_journal(block).ok().flatten().filter(|journal| {
            !online || journal.entries.iter().any(|entry| entry.received.is_none())
        })
    };
    let from_server = server.is_some();
    let Some(journal) = server.or_else(device) else {
        return (None, false, head, None);
    };
    // Numbered offline: the server lacks what it holds.
    let kept = !from_server && journal.entries.iter().any(|entry| entry.received.is_none());
    if journal.base.commit == head {
        return (Some(journal), kept, head, None);
    }
    // A seal of another run of the order counts its sequences there.
    let restarts = be_sdk_type_guest::wire::BaseState::decode(&journal.base.state)
        .map_or(0, |state| state.restarts);
    let mut sealed = None;
    let mut at = head;
    // A backstop to the stop below, which a journal started afresh (no state, so restarts read
    // 0) under seals of a later run never meets: giving up opens on the base, as before.
    let mut left = journal.entries.len() + SEAL_WALK;
    while let Some(commit) = at {
        if left == 0 {
            break;
        }
        left -= 1;
        let Some((parent, through)) = sealed_on(peer, commit) else {
            break;
        };
        if journal.base.commit == parent {
            // A seal made offline counts the device's own order: the server's numbers other
            // entries there (another device's), so it is a head to merge on the server's
            // journal, as `sealed_here` takes it.
            let reaches = through.restarts == restarts
                && !(from_server && through.offline)
                && be_block_session::reaches_seal(
                    journal.base.sequence,
                    journal.entries.iter().map(|entry| entry.sequence),
                    through.sequence,
                );
            sealed = reaches.then_some((commit, parent, through.sequence));
            break;
        }
        // The server's journal may still be on the base of a seal the head descends from: the
        // server lost the seal's rebase, and the outbox published it and the seals after it
        // (made offline). Its entries up to the seal are in the head; opened on the base, the
        // replica would merge them in again. It is told of the head after it loads. Seals count
        // on down the chain, those made offline on it too, so one of an earlier run, or at or
        // before the base's number, has none made on the base below it: the walk stops there
        // rather than read the block's history.
        let below_base = through.restarts < restarts
            || (through.restarts == restarts && through.sequence <= journal.base.sequence);
        if !from_server || below_base {
            break;
        }
        at = parent;
    }
    match sealed {
        Some((commit, parent, through)) => (
            Some(journal),
            kept,
            parent,
            Some(be_sdk_type_api::SealedHead { commit, through }),
        ),
        None if from_server => {
            let base = journal.base.commit;
            (Some(journal), false, base, None)
        }
        None => (None, false, head, None),
    }
}

/// How many seals past the journal's entries `journal_for` reads down the head's chain at most.
pub(crate) const SEAL_WALK: usize = 64;

#[cfg(test)]
thread_local! {
    /// How many commits `journal_for` read as seals on this thread.
    pub(crate) static SEALS_READ: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The commit `commit` was sealed on and how far it holds the order (`Peer::sealed_on`), each
/// read counted in a test.
fn sealed_on<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    commit: CommitId,
) -> Option<(Option<CommitId>, be_block_commit::SealedThrough)> {
    #[cfg(test)]
    SEALS_READ.with(|read| read.set(read.get() + 1));
    peer.sealed_on(commit)
}

/// Makes this device's copy of `block`'s order the server's journal as joining answered it, or
/// starts it afresh at `head` when the server holds none (`mirror_afresh`), before a replica is
/// opened on it (`reopening`). However a replica joined, its first join or a rejoin, the device
/// then numbers what the replica submits offline after what it opened on: a device with no
/// copy would start its own order at 0, behind the replica, which would take its entries for
/// ones it applied and never hear them. A copy that already matches is not written again.
fn mirror<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    server: Option<&ServerJournal>,
    online: bool,
    head: Option<CommitId>,
) -> Result<(), ClientError> {
    match server {
        Some((sealed, _)) if !kept_already(peer, block, sealed) => peer.keep_journal(block, sealed),
        Some(_) => Ok(()),
        None if online => mirror_afresh(peer, block, head),
        None => Ok(()),
    }
}

/// Whether this device's journal of `block` is `server`'s already: on the same base, holding
/// the same entries.
fn kept_already<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    server: &be_block_protocol::Journal,
) -> bool {
    let numbered = |journal: &be_block_protocol::Journal| {
        journal
            .entries
            .iter()
            .map(|entry| (entry.sequence, entry.origin, entry.counter))
            .collect::<Vec<_>>()
    };
    peer.sealed_journal(block).is_some_and(|kept| {
        kept.tag == server.tag
            && kept.base.commit == server.base.commit
            && kept.base.sequence == server.base.sequence
            && numbered(&kept) == numbered(server)
    })
}

/// Starts this device's copy of the order afresh at `head` when the server holds no journal,
/// so what it hears next is not kept beside an order that ended. A journal holding what only
/// this device numbered is left for the replica opened on it to seal.
fn mirror_afresh<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    head: Option<CommitId>,
) -> Result<(), ClientError> {
    let Some(mut mirror) = peer.sealed_journal(block) else {
        return Ok(());
    };
    if mirror.entries.iter().any(|entry| entry.received.is_none()) {
        return Ok(());
    }
    mirror.base = be_block_protocol::JournalBase {
        commit: head,
        sequence: 0,
        state: Vec::new(),
    };
    mirror.entries.clear();
    peer.keep_journal(block, &mirror)
}

/// For a server with no journal of `block`: when this device's journal is on the server's head
/// and the replica joining holds the lease, the server's journal starts from it, so the order
/// goes on with the session state this device sealed (what an editor shown that content numbers
/// its next edit by). Its entries were all sealed by then or heard from an order that ended.
async fn started_from_mirror<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    tag: be_block_store::Hash,
    client: ClientId,
    session: &SessionState,
    head: Option<CommitId>,
) -> Option<be_block_protocol::Journal> {
    if !session.is_holder(client) {
        return None;
    }
    let mirror = peer.local_journal(block).ok()??;
    if mirror.tag != tag
        || mirror.base.commit != head
        || (mirror.base.sequence == 0 && mirror.base.state.is_empty())
        || mirror.entries.iter().any(|entry| entry.received.is_none())
    {
        return None;
    }
    let answer = peer
        // The server holds no journal, so the run it starts at is no stale seal's: any of this
        // or a later run trims after it.
        .rebase_journal(block, tag, 0, mirror.base, true, false)
        .await
        .ok()??;
    answer.await.ok()?;
    let mut sealed = peer.sealed_journal(block)?;
    sealed.entries.clear();
    Some(sealed)
}

/// Fetches `head`'s history, so `journal_for` can tell a seal of the journal from a head to
/// merge. A head whose key this device lacks yet is opened as before, unread; any other failure
/// fails the join, which is tried again, rather than take a seal for a head to merge.
async fn fetched_for_join<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    head: CommitId,
) -> Result<(), ClientError> {
    match peer.fetch_history(head).await {
        Ok(())
        | Err(ClientError::Locked(_))
        | Err(ClientError::Store(be_block_store::StoreError::MissingKey(_))) => Ok(()),
        Err(error) => Err(error),
    }
}

/// A fast-forward to a seal of this session made on `local`, the commit the replica is on: its
/// news (`Resume::Sealed`), which the replica takes once it applied as far, not a head to merge,
/// which would count what both hold twice.
fn sealed_here<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    local: Option<CommitId>,
    resume: be_block_session::Resume,
) -> be_block_session::Resume {
    if let be_block_session::Resume::FastForward { to } = resume
        && let Some((parent, sealed)) = peer.sealed_on(to)
        && !sealed.offline
        && parent.is_some()
        && parent == local
    {
        return be_block_session::Resume::Sealed {
            commit: to,
            through: sealed.sequence,
            restarts: sealed.restarts,
        };
    }
    resume
}

/// The last number of the order `journal` holds: its last entry's, or its base's.
fn journal_through(journal: Option<&be_block_protocol::Journal>) -> Option<u64> {
    let journal = journal?;
    Some(
        journal
            .entries
            .last()
            .map_or(journal.base.sequence, |entry| entry.sequence),
    )
}

/// What the device keeps of `open`'s local edits no journal holds yet (`refs::OpenEdits`): each,
/// in the order given, so one opened again numbers them as this one did, with the submissions
/// holding them.
fn kept_edits(
    open: &Open,
) -> (
    be_block_client::OpenEdits,
    Vec<be_block_client::KeptEdit>,
    Vec<be_block_client::KeptSubmission>,
) {
    let edits = open
        .replay
        .iter()
        .filter_map(|replayed| match replayed {
            // A held message is not kept: a tentative gesture a crash interrupts leaves nothing.
            Replayed::Message {
                from: Endpoint::Editor(editor),
                bytes,
                held: None,
                ..
            } => Some(be_block_client::KeptEdit::Frame {
                editor: *editor,
                bytes: bytes.clone(),
            }),
            Replayed::Change { change, .. } => {
                Some(be_block_client::KeptEdit::Change(kept_change(change)))
            }
            _ => None,
        })
        .collect();
    let kept = be_block_client::OpenEdits {
        origin: open.origin,
        given_up: open.given_up,
        submitted_through: open.submitted_through,
        restarts: open.restarts,
        edits: Vec::new(),
        submissions: Vec::new(),
    };
    let submissions = open
        .submissions
        .iter()
        .filter(|submission| submission.through > open.given_up)
        .cloned()
        .collect();
    (kept, edits, submissions)
}

fn kept_change(change: &Change) -> be_block_client::KeptChange {
    use be_block_client::KeptChange;
    use be_sdk_type_api::ChildChange;
    match change.clone() {
        Change::Child(ChildChange::Add(block)) => KeptChange::AddChild(block),
        Change::Child(ChildChange::Delete(block)) => KeptChange::DeleteChild(block),
        Change::Child(ChildChange::Replace(replace)) => KeptChange::ReplaceChild {
            old: replace.old,
            new: replace.new,
        },
        Change::Child(ChildChange::Reorder(reorder)) => KeptChange::ReorderChild {
            block: reorder.block,
            after: reorder.after,
        },
        Change::FromFile(file) => KeptChange::FromFile {
            name: file.name,
            bytes: file.bytes,
        },
        Change::Replace(bytes) => KeptChange::Replace(bytes),
        Change::Undo => KeptChange::Undo,
        Change::Redo => KeptChange::Redo,
    }
}

fn api_change(kept: be_block_client::KeptChange) -> Change {
    use be_block_client::KeptChange;
    use be_sdk_type_api::{ChildChange, FileChange, ReorderChild, ReplaceChild};
    match kept {
        KeptChange::AddChild(block) => Change::Child(ChildChange::Add(block)),
        KeptChange::DeleteChild(block) => Change::Child(ChildChange::Delete(block)),
        KeptChange::ReplaceChild { old, new } => {
            Change::Child(ChildChange::Replace(ReplaceChild { old, new }))
        }
        KeptChange::ReorderChild { block, after } => {
            Change::Child(ChildChange::Reorder(ReorderChild { block, after }))
        }
        KeptChange::FromFile { name, bytes } => Change::FromFile(FileChange { name, bytes }),
        KeptChange::Replace(bytes) => Change::Replace(bytes),
        KeptChange::Undo => Change::Undo,
        KeptChange::Redo => Change::Redo,
    }
}

/// Opens the server's journal as joining answered it, keeping it sealed too.
fn opened<S: be_block_store::ObjectStore>(
    peer: &Peer<S>,
    block: Uuid,
    journal: Option<be_block_protocol::Journal>,
) -> Option<ServerJournal> {
    let journal = journal?;
    let opened = peer.open_journal(block, journal.clone()).ok()?;
    Some((journal, opened))
}

/// Whether the order holds local edits of `open` that no seal holds: what reopening it on its
/// head would lose (`reopening`).
fn journaled_unsealed(open: &Open) -> bool {
    open.status.journaled_taken > open.status.sealed_taken
}

/// Whether `open`'s rejoin, which sealed once, holds its answer: journaled edits no seal holds
/// are left, and no seal would take them now (`settling`).
fn rejoin_held(open: &Open) -> bool {
    open.rejoining.is_some()
        && (open.sealed_joining || open.sealed_before_joining)
        && journaled_unsealed(open)
        && !open.status.owes_seal
}

/// What a replica opened again on `head` loads (`journal_for`), the server's journal as joining
/// answered it when there is one, which this device's journal then keeps. What it is given again
/// (`Open::replay`) is cut by what its last status said the order holds of its edits: the
/// replica finds those in the order, by its origin, and would skip them anyway.
fn reopening<S: be_block_store::ObjectStore>(
    open: &mut Open,
    peer: &Peer<S>,
    server: Option<ServerJournal>,
    online: bool,
    head: Option<CommitId>,
) -> Loads {
    open.reported_clean(open.journaled_taken);
    journal_for(
        peer,
        open.block,
        server.map(|(_, opened)| opened),
        online,
        head,
    )
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

/// An opened journal as a replica of this module takes it: as it is when the module wrote it;
/// otherwise with none of its entries, which this build cannot read, on `head` at the order's
/// last number, so the replica follows the order on from there (and its holder starts the
/// journal over, `opening.lost-edits`).
fn replayable(
    journal: be_block_protocol::Journal,
    tag: be_sdk_type_api::SessionTag,
    head: Option<CommitId>,
) -> Option<be_sdk_type_api::Journal> {
    if journal.tag.as_bytes() != &tag.0 {
        let last = journal
            .entries
            .last()
            .map_or(journal.base.sequence, |entry| entry.sequence);
        return Some(be_sdk_type_api::Journal {
            base: be_sdk_type_api::JournalBase {
                commit: head,
                sequence: last,
                state: Vec::new(),
            },
            entries: Vec::new(),
        });
    }
    Some(be_sdk_type_api::Journal {
        base: be_sdk_type_api::JournalBase {
            commit: journal.base.commit,
            sequence: journal.base.sequence,
            state: journal.base.state,
        },
        entries: journal.entries.into_iter().map(api_entry).collect(),
    })
}

/// Notes what `opening` tells a replica (`Replicas::openings`).
#[cfg(test)]
fn note_opening(openings: &mut Vec<(Uuid, u64, Vec<u64>)>, opening: &Opening) {
    openings.push((
        opening.block,
        opening.kept_restart,
        opening
            .kept_submissions
            .iter()
            .map(|submission| submission.through)
            .collect(),
    ));
}
