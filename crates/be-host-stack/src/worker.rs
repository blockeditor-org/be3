use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Condvar, PoisonError},
    time::{Duration, Instant},
};

use be_wasm_threads::Mutex;

use be_block::BlockMetadata;
use be_block_client::{ClientError, Credentials, Peer, PeerConfig, Write};
use be_block_client::{Event, ask};
use be_block_commit::CommitId;
use be_block_graph::{Access, BlockParent};
use be_block_protocol::{AccessList, BlockSummary, client};
use be_block_type_host::Conflict;
use be_sdk_type_api::{Change, Description, EditorId};
use be_sdk_type_guest::editor::FromEditor;
use futures_util::future::{Either, select};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use super::graph::{Graph, Listing, Node, Owner, Query};
use super::replicas::Replicas;
use super::types::{self, Modules};
use super::version::{Context, Versions};
use super::{Config, Content, platform};

mod uploads;

use uploads::Uploads;

const RECONNECT_FIRST: Duration = Duration::from_millis(500);
const RECONNECT_MOST: Duration = Duration::from_secs(30);
pub(super) const OUTBOX_RETRY: Duration = Duration::from_secs(30);

/// Doubles from `RECONNECT_FIRST` to `RECONNECT_MOST`, randomly shortened by up to half so
/// clients a server dropped together do not come back together.
pub(super) fn reconnect_delay(failures: u32, jitter: f64) -> Duration {
    let ceiling = RECONNECT_FIRST
        .saturating_mul(1 << failures.min(16))
        .min(RECONNECT_MOST);
    ceiling.mul_f64(0.5 + 0.5 * jitter.clamp(0.0, 1.0))
}

/// When to try failed version statuses again (`Versions::refresh`), from the deadline
/// already armed and how many came due in a row: armed once and kept until it comes due, then
/// moved on by `reconnect_delay`; cleared when nothing transient awaits it or the line is down.
pub(crate) fn next_retry(
    awaiting: bool,
    now: Duration,
    armed: Option<Duration>,
    failures: u32,
    jitter: f64,
) -> (Option<Duration>, u32) {
    let due = armed.is_some_and(|at| at <= now);
    match (awaiting, due || armed.is_none()) {
        (true, true) => (
            Some(now + reconnect_delay(failures, jitter)),
            failures.saturating_add(1),
        ),
        (true, false) => (armed, failures),
        (false, _) => (None, 0),
    }
}

fn jitter() -> f64 {
    (Uuid::new_v4().as_u128() as u64 >> 11) as f64 / (1u64 << 53) as f64
}

pub(super) struct Inbox {
    pub(super) commands: UnboundedReceiver<Command>,
    pub(super) nudged: super::nudge::Nudged,
}

type AccessReply = Box<dyn FnOnce(Result<AccessList, String>) + Send>;
type NotifyModeReply = Box<dyn FnOnce(Result<be_block_protocol::NotifyMode, String>) + Send>;

type ExportReply = Box<dyn FnOnce(Result<(String, Vec<u8>), String>) + Send>;

type HeadReply = Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>;

type PlainReply = Box<dyn FnOnce(Result<be_block_type_host::PlainFile, String>) + Send>;

type ContentReply = Box<dyn FnOnce(Result<(Uuid, Vec<u8>), String>) + Send>;

pub(super) enum Command {
    Unsent {
        block: Uuid,
        restore: bool,
    },
    PreviewChild {
        alias: Uuid,
        source: Uuid,
        content_type: Uuid,
    },
    Revision {
        block: Uuid,
        request: super::revisions::Asked,
        reply: Box<dyn FnOnce(super::revisions::Answered) + Send>,
    },
    Open {
        block: Uuid,
        content_type: Uuid,
        owned: bool,
    },
    #[cfg(any(test, feature = "testing"))]
    Close(Uuid),
    /// Start or stop following the account's storage, as `Shared::storage_watched` says.
    Storage,
    Operate(Uuid, Vec<u8>, Instant),
    ChangeChild {
        block: Uuid,
        content_type: Uuid,
        change: be_sdk_type_api::ChildChange,
    },
    FromFile {
        block: Uuid,
        content_type: Uuid,
        name: String,
        bytes: Vec<u8>,
    },
    Attach {
        block: Uuid,
        content_type: Uuid,
        editor: EditorId,
        editable: bool,
    },
    Detach {
        block: Uuid,
        editor: EditorId,
    },
    FromEditor {
        block: Uuid,
        editor: EditorId,
        bytes: Vec<u8>,
        at: Instant,
    },
    HoldClock(Option<Instant>),
    Duplicate {
        from: Uuid,
        to: Uuid,
        content_type: Uuid,
        metadata: BlockMetadata,
    },
    Seed {
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
    },
    Replace {
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
    },
    Flush(std::sync::mpsc::Sender<()>),
    History {
        block: Uuid,
        redo: bool,
    },
    #[cfg(test)]
    Presence {
        block: Uuid,
        kind: Uuid,
        value: Option<Vec<u8>>,
    },
    #[cfg(test)]
    HearHead {
        block: Uuid,
        head: CommitId,
    },
    #[cfg(test)]
    SendToPeers {
        block: Uuid,
        to: Option<u64>,
        channel: Uuid,
        payload: Vec<u8>,
    },
    Create {
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
        metadata: BlockMetadata,
        bytes: Option<Vec<u8>>,
        owner_only: bool,
    },
    SetParent {
        block: Uuid,
        parent: BlockParent,
    },
    Define {
        block: Uuid,
        changes: Vec<be_block::Definition>,
    },
    SetAccess {
        block: Uuid,
        account: Uuid,
        access: Access,
    },
    ListAccess {
        block: Uuid,
        reply: AccessReply,
    },
    SetNotifyMode {
        block: Uuid,
        mode: be_block_protocol::NotifyMode,
        reply: NotifyModeReply,
    },
    ReadHead {
        block: Uuid,
        commit: CommitId,
        reply: HeadReply,
    },
    Export {
        block: Uuid,
        reply: ExportReply,
    },
    PlainFile {
        block: Uuid,
        reply: PlainReply,
    },
    Import(Vec<(be_block_client::transfer::Saved, BlockParent)>),
    ContentOfFile {
        name: String,
        bytes: Vec<u8>,
        reply: ContentReply,
    },
    Version {
        block: Uuid,
        command: be_sdk_api::VersionCommand,
    },
    WatchVersion(Uuid),
    ApprovePairing {
        from: u64,
        code: String,
    },
    DismissPairing(u64),
    Contact(ContactCommand),
    Leave(Uuid),
    Members(MembersCommand),
    Push(PushCommand),
    Invite {
        email: String,
        role: be_block_protocol::OrganizationRole,
        reply: LinkReply,
    },
    CreateAgent {
        name: String,
        reply: AgentReply,
    },
    /// An app's `create-block-agent`: an agent bound to `block`, its key sealed to it.
    CreateBlockAgent {
        block: Uuid,
        name: String,
        reply: BlockAgentReply,
    },
    /// An app's `rotate-agent-token` or `remove-agent`, refused for an agent of the person's
    /// own (one the Agents menu lists).
    BlockAgent {
        agent: Uuid,
        change: AgentChange,
    },
    /// An app's `sign` or `verify`.
    Authorship(crate::authorship::AuthorshipCommand),
    Sync,
}

pub(super) enum AgentChange {
    Rotate(Box<dyn FnOnce(Result<String, String>) + Send>),
    Remove(Box<dyn FnOnce(Result<(), String>) + Send>),
}

impl AgentChange {
    fn fail(self, error: String) {
        match self {
            Self::Rotate(reply) => reply(Err(error)),
            Self::Remove(reply) => reply(Err(error)),
        }
    }
}

impl Command {
    fn is_quick(&self) -> bool {
        matches!(
            self,
            Command::Operate(..)
                | Command::FromEditor { .. }
                | Command::HoldClock(_)
                | Command::Attach { .. }
                | Command::Detach { .. }
                | Command::Open { .. }
        )
    }

    fn needs(&self) -> Vec<Uuid> {
        match self {
            Command::Create { block, parent, .. } | Command::SetParent { block, parent } => {
                [Some(*block), parent.block()]
                    .into_iter()
                    .flatten()
                    .collect()
            }
            Command::Duplicate { from, to, .. } => vec![*from, *to],
            Command::Export { block, .. } => vec![*block],
            command => command.subject().into_iter().collect(),
        }
    }

    fn subject(&self) -> Option<Uuid> {
        match self {
            Command::PreviewChild { source, .. } => Some(*source),
            Command::Revision { block, .. }
            | Command::Open { block, .. }
            | Command::Operate(block, ..)
            | Command::Seed { block, .. }
            | Command::Replace { block, .. }
            | Command::History { block, .. }
            | Command::Attach { block, .. }
            | Command::ChangeChild { block, .. }
            | Command::FromFile { block, .. }
            | Command::Detach { block, .. }
            | Command::FromEditor { block, .. }
            | Command::Create { block, .. }
            | Command::SetParent { block, .. }
            | Command::Define { block, .. }
            | Command::SetAccess { block, .. }
            | Command::ListAccess { block, .. }
            | Command::SetNotifyMode { block, .. }
            | Command::ReadHead { block, .. }
            | Command::CreateBlockAgent { block, .. }
            | Command::PlainFile { block, .. }
            | Command::Version { block, .. }
            | Command::WatchVersion(block)
            | Command::Leave(block)
            | Command::Contact(ContactCommand::Link {
                offer: Some((block, _)),
                ..
            }) => Some(*block),
            Command::Duplicate { to, .. } => Some(*to),
            #[cfg(any(test, feature = "testing"))]
            Command::Close(block) => Some(*block),
            #[cfg(test)]
            Command::Presence { block, .. }
            | Command::SendToPeers { block, .. }
            | Command::HearHead { block, .. } => Some(*block),
            Command::Export { .. }
            | Command::Unsent { .. }
            | Command::Import(_)
            | Command::ContentOfFile { .. }
            | Command::Flush(_)
            | Command::ApprovePairing { .. }
            | Command::DismissPairing(_)
            | Command::Contact(_)
            | Command::Members(_)
            | Command::Invite { .. }
            | Command::CreateAgent { .. }
            | Command::BlockAgent { .. }
            | Command::Push(_)
            | Command::Authorship(_)
            | Command::Sync
            | Command::Storage
            | Command::HoldClock(_) => None,
        }
    }

    fn rekeys(&self) -> Option<Uuid> {
        match self {
            Command::SetParent { block, .. }
            | Command::SetAccess { block, .. }
            | Command::CreateBlockAgent { block, .. }
            | Command::Leave(block)
            | Command::Contact(ContactCommand::Link {
                offer: Some((block, _)),
                ..
            }) => Some(*block),
            _ => None,
        }
    }
}

type MembersReply = Box<dyn FnOnce(Result<Vec<be_block_protocol::Member>, String>) + Send>;

/// What a device does with push notifications; a reply is the peer's error as text.
pub(super) enum PushCommand {
    Key(Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>),
    Register {
        endpoint: be_block_protocol::PushEndpoint,
        reply: Box<dyn FnOnce(Result<(), String>) + Send>,
    },
    Unregister {
        address: String,
        reply: Box<dyn FnOnce(Result<(), String>) + Send>,
    },
    /// Opens one a push carried, offline, from the keys the device keeps.
    Open {
        message: be_block_protocol::PushMessage,
        now: i64,
        reply: Box<dyn FnOnce(Result<be_block_client::OpenedNotification, String>) + Send>,
    },
}

pub(super) enum MembersCommand {
    List(MembersReply),
    SetRole {
        account: Uuid,
        role: be_block_protocol::OrganizationRole,
        reply: MembersReply,
    },
    Remove {
        account: Uuid,
        reply: MembersReply,
    },
}

type LinkReply = Box<dyn FnOnce(Result<String, String>) + Send>;

type AgentReply = Box<dyn FnOnce(Result<crate::accounts::AgentCredentials, String>) + Send>;

type BlockAgentReply =
    Box<dyn FnOnce(Result<(Uuid, crate::accounts::AgentCredentials), String>) + Send>;

type CardReply = Box<dyn FnOnce(Result<be_block_keys::ContactCard, String>) + Send>;

pub(super) enum ContactCommand {
    Link {
        offer: Option<(Uuid, Access)>,
        reply: LinkReply,
    },
    Open {
        code: be_block_keys::InviteCode,
        reply: CardReply,
    },
    Accept {
        code: be_block_keys::InviteCode,
        reply: CardReply,
    },
    Remove(Uuid),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingRequest {
    pub from: u64,
    pub device: String,
    message: Vec<u8>,
}

const PAIRING_REQUEST_LIMIT: usize = 4;

pub(super) const PEER_MESSAGE_LIMIT: usize = 4096;

#[derive(Default)]
pub struct Shared {
    pub revision_previews: HashMap<Uuid, super::revisions::Preview>,
    pub preview_aliases: HashMap<(Uuid, Uuid), Uuid>,
    pub released_previews: Vec<Uuid>,
    pub graph: Graph,
    pub blocks: HashMap<Uuid, Content>,
    pub described: HashMap<Uuid, Description>,
    pub editor_frames: HashMap<EditorId, Vec<(Uuid, Vec<u8>)>>,
    pub histories: HashMap<Uuid, History>,
    pub presence: HashMap<Uuid, (u64, Vec<Presence>)>,
    pub presence_revision: u64,
    pub peer_messages: HashMap<Uuid, std::collections::VecDeque<(u64, PeerMessage)>>,
    pub peer_message_count: u64,
    pub wakes: u64,
    pub unsealed: usize,
    pub saves: u64,
    pub owing: usize,
    /// Open replicas with edits that are not in the device's journal yet.
    pub undurable: usize,
    pub connected: bool,
    pub connection: Option<Arc<be_block_client::connection::Connection>>,
    #[cfg(test)]
    pub connection_due: Option<tokio::sync::watch::Receiver<Duration>>,
    /// The clock and the moment the stack waits on to try the server again. A moment the clock
    /// has not reached means the stack is waiting for it; `None` while it is connected, and while
    /// the first connect is in flight. A wait on a driven clock (`wait_until`) moves the clock on
    /// to it.
    #[cfg(any(test, feature = "testing"))]
    pub reconnect: Option<(be_block_client::Clock, Duration)>,
    #[cfg(test)]
    pub parked: Option<Option<Duration>>,
    pub reached: bool,
    /// Between connecting and having asked for everything the device must catch up on: set as
    /// `Session::arrive` starts, cleared once `arrive` has opened the wanted blocks and the
    /// outbox has drained and the open replicas have rejoined their sessions.
    pub arriving: bool,
    /// Set when a command ran or a seal moved edits into the outbox, until `report` has read
    /// the outbox into `waiting` again: `waiting` is stale meanwhile.
    pub recounting: bool,
    pub draining: bool,
    /// Counts finished drains of the outbox.
    pub drains: u64,
    pub leaving: bool,
    pub error: Option<String>,
    pub module_error: Option<(Uuid, String)>,
    pub conflicts: HashMap<Uuid, Vec<Conflict>>,
    pub pairing: Vec<PairingRequest>,
    pub contacts: Vec<be_block_client::Contact>,
    pub contacts_revision: u64,
    /// The account's storage, while an editor watches it (`storage_watched`).
    pub storage: Option<be_block_protocol::AccountStorage>,
    pub storage_revision: u64,
    pub storage_watched: bool,
    pub escrow_warning: Option<&'static str>,
    /// The account's notification mode for each block it set to other than `all`, once read
    /// (`Tasks::notify_modes`), and how often it changed, for the watch the host publishes.
    pub notify_modes: Option<HashMap<Uuid, be_block_protocol::NotifyMode>>,
    pub notify_modes_revision: u64,
    pub locked: std::collections::HashSet<Uuid>,
    pub version_watch: std::collections::HashSet<Uuid>,
    pub versions: HashMap<Uuid, super::version::VersionState>,
    pub touched: be_lib_stable_hash::HashSet<Uuid>,
    pub pending: usize,
    pub typing: bool,
    pub uploading: usize,
    pub listing_revisions: usize,
    pub importing: usize,
    pub previewed: std::collections::HashSet<Uuid>,
    pub staged: HashMap<Uuid, (Uuid, Vec<u8>)>,
    pub owned: std::collections::HashSet<Uuid>,
    pub unreadable: HashMap<Uuid, String>,
    pub stale_revision: u64,
    pub waiting: usize,
    pub unsent: Vec<be_block_client::UnsentBlock>,
    pub comparison: Option<super::conflicts::Comparison>,
    pub wall: Wall,
    pub search: super::search::Searching,
}

#[derive(Clone, Default)]
pub struct Wall {
    clock: be_block_client::Clock,
    anchor: Option<(i64, std::time::Duration)>,
}

impl Wall {
    pub fn new(clock: be_block_client::Clock) -> Self {
        let anchor = Some((be_block_commit::now_milliseconds(), clock.now()));
        Self { clock, anchor }
    }

    pub fn now(&self) -> i64 {
        let Some((wall, at)) = self.anchor else {
            return be_block_commit::now_milliseconds();
        };
        let since = self.clock.now().saturating_sub(at);
        wall.saturating_add(i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
    }
}

impl Shared {
    pub fn touch(&mut self, block: Uuid) {
        self.touched.insert(block);
    }

    /// What keeps the device from being in step with the server, apart from `busy`: drive's
    /// `synced` waits until there is nothing.
    pub fn unsynced(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        if !self.connected {
            reasons.push(match &self.error {
                Some(error) => format!("the block store is not connected to the server: {error}"),
                None => "the block store is not connected to the server".to_owned(),
            });
        }
        if self.arriving {
            reasons.push("the block store catches up with the server after connecting".to_owned());
        }
        if self.draining {
            reasons.push("the block store sends the edits it kept".to_owned());
        }
        if self.recounting {
            reasons.push("the block store has yet to count what waits to be sent".to_owned());
        }
        let waiting = self.unsealed + self.waiting;
        if waiting > 0 {
            reasons.push(format!("{waiting} edit(s) wait to be sent"));
        }
        reasons
    }

    pub fn busy(&self) -> Vec<&'static str> {
        [
            (
                self.pending > 0,
                "the block store has commands to carry out",
            ),
            (self.typing, "a block type module has yet to settle"),
            (self.draining, "the block store sends sealed edits"),
            (self.uploading > 0, "the block store uploads files"),
            (
                self.listing_revisions > 0,
                "the block store lists a block's revisions",
            ),
            (self.owing > 0, "the block store owes edits a seal"),
            (
                self.undurable > 0,
                "the block store keeps edits the device's journal does not hold yet",
            ),
            (
                self.graph.fetching(),
                "the block store loads the organization's graph",
            ),
            (
                self.search.busy(super::search::wants_index(self)),
                "the block store indexes edits or answers a search",
            ),
        ]
        .into_iter()
        .filter_map(|(busy, reason)| busy.then_some(reason))
        .collect()
    }
}

pub(super) type Store = be_block_store::FileStore;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerMessage {
    pub from: u64,
    pub channel: Uuid,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Presence {
    pub client: u64,
    pub kind: Uuid,
    pub value: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct History {
    pub can_undo: bool,
    pub can_redo: bool,
    /// Undo and redo stopped at the session restart that committed this.
    pub restarted_at: Option<be_block_commit::CommitId>,
    /// Edits by another build of the block's type were dropped when it opened.
    pub lost_edits: bool,
}

pub(super) async fn serve<S: Fn() -> Result<Store, String>>(
    config: Config,
    make_store: S,
    Inbox {
        mut commands,
        mut nudged,
    }: Inbox,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
) {
    let mut replicas = Replicas::new(Modules::new(types::sources().await, config.clock.clone()));
    replicas.modules.hold_clock(super::held_clock());
    replicas.modules.ready().await;
    be_host_base::host::wake();
    let started = match cached(&config, &make_store) {
        Some(peer) => Some((peer, None)),
        None => first_connect(&config, &make_store, &mut nudged, &shared, &changed).await,
    };
    if let Some((peer, open)) = started {
        forget_what_is_not_kept(&peer);
        resolve_through_graph(&peer, &shared);
        let mut session = Session {
            config: &config,
            peer: &peer,
            commands: &mut commands,
            nudged: &mut nudged,
            shared: &shared,
            changed: &changed,
            wanted: Wanted::default(),
            replicas: &mut replicas,
            uploads: Uploads::new(&shared),
            versions: Versions::default(),
            line: Line::down(&config, 0, Duration::ZERO),
            tasks: Tasks::new(),
            drained_at: Duration::ZERO,
            versions_due: None,
            version_failures: 0,
            rejoin: false,
            in_arrive: false,
        };
        session.run(open).await;
        forget_what_is_not_kept(&peer);
    }
    shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .connected = false;
    changed.notify_all();
}

/// Notes the moment the stack next tries the server, for a test's wait to move a manual clock to
/// (`Shared::reconnect`); `None` once it tries.
#[cfg(any(test, feature = "testing"))]
fn reconnecting_at(shared: &Arc<Mutex<Shared>>, at: Option<(be_block_client::Clock, Duration)>) {
    shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .reconnect = at;
}

fn forget_what_is_not_kept(peer: &Peer<Store>) {
    let _ = peer.forget_offline(|workspace| crate::offline_choice::choice(workspace).keep);
}

fn peer_config(config: &Config, refs: be_block_client::RefsDb) -> PeerConfig {
    PeerConfig::new(
        config.socket_url(),
        be_block_keys::RecoverySecret::from_bytes(config.identity),
        Credentials::Token(config.token.clone()),
    )
    .organization(Some(config.organization))
    .refs(refs)
}

fn cached<S: Fn() -> Result<Store, String>>(
    config: &Config,
    make_store: &S,
) -> Option<Arc<Peer<Store>>> {
    let refs = super::refs(config).ok()?;
    if !refs
        .knows(&config.socket_url(), config.account, config.organization)
        .unwrap_or(false)
    {
        return None;
    }
    let store = make_store().ok()?;
    Peer::cached(peer_config(config, refs), config.account, store)
        .ok()
        .map(Arc::new)
}

async fn first_connect<S: Fn() -> Result<Store, String>>(
    config: &Config,
    make_store: &S,
    nudged: &mut super::nudge::Nudged,
    shared: &Arc<Mutex<Shared>>,
    changed: &Arc<Condvar>,
) -> Option<(Arc<Peer<Store>>, Option<Opened>)> {
    let mut failures = 0;
    loop {
        #[cfg(any(test, feature = "testing"))]
        reconnecting_at(shared, None);
        match make_store().and_then(|store| Ok((store, super::refs(config)?))) {
            Ok((store, refs)) => {
                match unless_stopped(connect(config, store, refs), nudged).await? {
                    Ok((peer, open)) => {
                        nudged.mark_unchanged();
                        return Some((Arc::new(peer), Some(open)));
                    }
                    Err(error) => fail(shared, error.to_string()),
                }
            }
            Err(error) => fail(shared, error),
        }
        let delay = reconnect_delay(failures, jitter());
        failures = failures.saturating_add(1);
        // One moment for the sleep and for the wait that moves a clock to it.
        let at = config.clock.now() + delay;
        #[cfg(any(test, feature = "testing"))]
        reconnecting_at(shared, Some((config.clock.clone(), at)));
        changed.notify_all();
        be_host_base::host::wake();
        let waited = config.clock.sleep_until(at);
        let nudge = std::pin::pin!(nudged.changed());
        if let Either::Right((Err(_), _)) = select(waited, nudge).await {
            return None;
        }
    }
}

fn resolve_through_graph(peer: &Peer<Store>, shared: &Arc<Mutex<Shared>>) {
    let resolving = Arc::clone(shared);
    peer.set_resolver(Arc::new(move |block, references| {
        let held = resolving.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(scope) = held.graph.scope_of(block) else {
            return references;
        };
        references
            .into_iter()
            .map(|reference| {
                held.graph
                    .to_real(scope, reference, be_sdk_api::BlockIdRole::Existing)
            })
            .collect()
    }));
}

async fn unless_stopped<T>(
    work: impl Future<Output = T>,
    nudged: &mut super::nudge::Nudged,
) -> Option<T> {
    let mut work = std::pin::pin!(work);
    loop {
        let nudge = std::pin::pin!(nudged.changed());
        match select(work.as_mut(), nudge).await {
            Either::Left((done, _)) => return Some(done),
            Either::Right((Ok(()), _)) => {}
            Either::Right((Err(_), _)) => return None,
        }
    }
}

type Opened = tokio::sync::OwnedMutexGuard<()>;

type Reaching = std::pin::Pin<Box<dyn Future<Output = Result<crate::link::Link, ClientError>>>>;

enum Line {
    Up {
        events: tokio::sync::broadcast::Receiver<Event>,
        gone: tokio::sync::watch::Receiver<bool>,
        _open: Opened,
    },
    Down {
        failures: u32,
        /// When it tries again, on the config's clock.
        #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
        at: Duration,
        reaching: Reaching,
    },
}

impl Line {
    fn up(peer: &Peer<Store>, open: Opened) -> Self {
        let connection = peer.connection();
        Self::Up {
            events: connection.subscribe(),
            gone: connection.closed(),
            _open: open,
        }
    }

    /// Down for `delay` more on the config's clock. The moment it tries again is fixed here, not
    /// when the future first runs: a clock moved on between the two (a test's wait moving it to
    /// the moment `Shared::reconnect` says) would put the sleep's end past the moment it moved to.
    fn down(config: &Config, failures: u32, delay: Duration) -> Self {
        let clock = config.clock.clone();
        let url = config.server_url.clone();
        let token = config.token.clone();
        let at = clock.now() + delay;
        Self::Down {
            failures,
            at,
            reaching: Box::pin(async move {
                clock.sleep_until(at).await;
                crate::link::signed_in(&url, &token, &clock).await
            }),
        }
    }

    fn is_up(&self) -> bool {
        matches!(self, Self::Up { .. })
    }

    fn quiet(&self) -> bool {
        match self {
            Self::Up { events, .. } => events.is_empty(),
            Self::Down { .. } => true,
        }
    }

    async fn next(&mut self) -> Woken {
        match self {
            Self::Up { events, gone, .. } => {
                let event = std::pin::pin!(events.recv());
                let closed = std::pin::pin!(gone.changed());
                match select(event, closed).await {
                    Either::Left((Ok(event), _)) => Woken::Event(Some(event)),
                    Either::Left((Err(tokio::sync::broadcast::error::RecvError::Lagged(_)), _)) => {
                        Woken::Event(None)
                    }
                    Either::Left((Err(_), _)) | Either::Right(_) => Woken::Disconnected,
                }
            }
            Self::Down { reaching, .. } => Woken::Reached(reaching.as_mut().await),
        }
    }
}

struct Session<'a> {
    config: &'a Config,
    peer: &'a Arc<Peer<Store>>,
    commands: &'a mut UnboundedReceiver<Command>,
    nudged: &'a mut super::nudge::Nudged,
    shared: &'a Arc<Mutex<Shared>>,
    changed: &'a Arc<Condvar>,
    wanted: Wanted,
    replicas: &'a mut Replicas,
    uploads: Uploads,
    versions: Versions,
    line: Line,
    tasks: (Tasks, UnboundedReceiver<()>),
    drained_at: Duration,
    /// When to refresh the version statuses again, set while one could not be computed and
    /// the line is up: backing off as reconnecting does (`reconnect_delay`), from
    /// `version_failures` refreshes in a row that failed.
    versions_due: Option<Duration>,
    version_failures: u32,
    rejoin: bool,
    /// While `arrive` runs, which says the device arrived itself once it has asked for
    /// everything.
    in_arrive: bool,
}

impl Session<'_> {
    async fn run(&mut self, open: Option<Opened>) {
        let (peer, shared) = (self.peer, self.shared);
        self.uploads.connect(peer);
        {
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            let my_files = be_block::my_files(peer.organization(), peer.account());
            let unfiled = be_block::unfiled(peer.organization(), peer.account());
            held.graph.want(
                Owner::Host,
                vec![Query::Roots, Query::Block(my_files), Query::Block(unfiled)],
            );
        }
        match open {
            Some(open) => self.arrive(open).await,
            None => {
                self.depart(0);
                want_open(shared, &self.wanted.blocks());
                relist(shared);
                spawn_listings(peer, shared, &self.tasks.0);
                self.refresh_versions().await;
                report(peer, shared);
                self.changed.notify_all();
                be_host_base::host::wake();
            }
        }
        let mut flushes = Vec::new();
        let mut kept_for_flushes = false;
        let mut keyed: HashMap<Uuid, Uuid> = HashMap::new();
        let mut stopping = false;
        loop {
            if stopping && (self.uploads.is_empty() || !self.line.is_up()) {
                self.replicas.keep_all((peer, shared)).await;
                report(peer, shared);
                return;
            }
            let wake_at = [
                self.replicas.modules.wake_at(),
                self.retry_at(),
                self.versions_due,
            ]
            .into_iter()
            .flatten()
            .min();
            let woken = {
                let waited = std::pin::pin!(wait(
                    self.commands,
                    &mut self.line,
                    self.nudged,
                    &mut self.uploads,
                    &mut *self.replicas,
                    &mut self.tasks.1,
                    stopping
                ));
                let timer = wake_at.map(|at| self.config.clock.sleep_until(at));
                let woken = async move {
                    match timer {
                        Some(timer) => match select(waited, timer).await {
                            Either::Left((woken, _)) => woken,
                            Either::Right(_) => Woken::Deadline,
                        },
                        None => waited.await,
                    }
                };
                #[cfg(test)]
                let woken = parked(shared, self.changed, wake_at, woken);
                woken.await
            };
            let handled = self.replicas.handled;
            let pumped = matches!(woken, Woken::Module) && self.replicas.pump((peer, shared)).await;
            if !matches!(woken, Woken::Module | Woken::Finished) || self.replicas.handled != handled
            {
                shared.lock().unwrap_or_else(PoisonError::into_inner).wakes += 1;
            }
            let mut moved = match woken {
                Woken::Stopped => {
                    stopping = true;
                    false
                }
                Woken::Nudged => {
                    if let Line::Down { failures, .. } = self.line {
                        self.line = Line::down(self.config, failures, Duration::ZERO);
                        #[cfg(any(test, feature = "testing"))]
                        reconnecting_at(shared, None);
                    }
                    false
                }
                Woken::Disconnected => {
                    self.depart(0);
                    true
                }
                Woken::Reached(Ok(link)) => {
                    #[cfg(any(test, feature = "testing"))]
                    reconnecting_at(shared, None);
                    shared
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .connection = Some(Arc::clone(&link.connection));
                    let open = link.open_organization().await;
                    match peer
                        .attach(link.connection, self.config.token.clone())
                        .await
                    {
                        Ok(()) => {
                            self.nudged.mark_unchanged();
                            self.arrive(open).await;
                        }
                        Err(error) => {
                            peer.detach();
                            let failures = self.failures();
                            fail(shared, error.to_string());
                            self.depart(failures + 1);
                        }
                    }
                    true
                }
                Woken::Reached(Err(error)) => {
                    let failures = self.failures();
                    fail(shared, error.to_string());
                    self.depart(failures + 1);
                    false
                }
                Woken::Command(command) => {
                    let mut ran = Ran {
                        peer,
                        replicas: &mut *self.replicas,
                        wanted: &mut self.wanted,
                        shared,
                        config: self.config,
                        versions: &mut self.versions,
                        flushes: &mut flushes,
                        tasks: &self.tasks.0,
                    };
                    let mut next = Some(command);
                    let mut applied = false;
                    while let Some(command) = next.take() {
                        let quick = command.is_quick();
                        match command {
                            Command::Import(blocks) => {
                                self.uploads.add(blocks);
                                ran.done();
                            }
                            command => {
                                if let Some(command) = self.uploads.defer(command) {
                                    applied |= ran.run(command).await;
                                }
                            }
                        }
                        if quick {
                            next = self.commands.try_recv().ok();
                        }
                    }
                    applied
                }
                Woken::Uploaded => {
                    let mut ran = Ran {
                        peer,
                        replicas: &mut *self.replicas,
                        wanted: &mut self.wanted,
                        shared,
                        config: self.config,
                        versions: &mut self.versions,
                        flushes: &mut flushes,
                        tasks: &self.tasks.0,
                    };
                    let mut applied = false;
                    for command in self.uploads.runnable() {
                        applied |= ran.run(command).await;
                    }
                    be_host_base::host::wake();
                    applied
                }
                Woken::Event(event) => {
                    let mut heard_all = vec![event];
                    if let Line::Up { events, .. } = &mut self.line {
                        loop {
                            match events.try_recv() {
                                Ok(event) => heard_all.push(Some(event)),
                                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                                    heard_all.push(None);
                                }
                                Err(_) => break,
                            }
                        }
                    }
                    for event in heard_all {
                        if matches!(
                            event,
                            Some(Event::HeadChanged { .. } | Event::BlockChanged(_)) | None
                        ) {
                            self.versions.announced();
                        }
                        match &event {
                            Some(message) => self.replicas.heard((peer, shared), message),
                            None => self.replicas.resynchronize((peer, shared)),
                        }
                        heard(peer, shared, &self.tasks.0, self.replicas, event).await;
                    }
                    false
                }
                Woken::Module | Woken::Deadline => {
                    self.replicas.modules.tick_due();
                    false
                }
                Woken::Finished => true,
            };
            if self
                .retry_at()
                .is_some_and(|at| at <= self.config.clock.now())
            {
                self.drained_at = self.config.clock.now();
                self.tasks.0.sync(peer, shared);
                moved = true;
            }
            moved |= pumped;
            moved |= self.replicas.pump((peer, shared)).await;
            moved |= self.settle_outbox().await;
            rekey_changed(peer, self.replicas, &mut keyed);
            let before = version_revisions(shared);
            self.refresh_versions().await;
            moved |= version_revisions(shared) != before;
            want_open(shared, &self.wanted.blocks());
            moved |= super::search::step(peer, shared, self.replicas, &self.tasks.0).await;
            spawn_listings(peer, shared, &self.tasks.0);
            if self.line.quiet()
                && (self.uploads.is_empty() || !self.line.is_up())
                && !flushes.is_empty()
            {
                if !std::mem::replace(&mut kept_for_flushes, true) {
                    self.replicas.keep_all((peer, shared)).await;
                    if self.replicas.take_kept() {
                        self.tasks.0.sync(peer, shared);
                    }
                }
                if (!self.tasks.0.syncing() || !self.line.is_up()) && self.replicas.reported() {
                    kept_for_flushes = false;
                    for done in flushes.drain(..) {
                        let _ = done.send(());
                    }
                }
            }
            moved |= report(peer, shared);
            let typing = !self.replicas.modules.settled();
            let was_typing = std::mem::replace(
                &mut shared.lock().unwrap_or_else(PoisonError::into_inner).typing,
                typing,
            );
            self.changed.notify_all();
            if moved || was_typing != typing {
                be_host_base::host::wake();
            }
        }
    }

    async fn refresh_versions(&mut self) {
        let (peer, shared) = (self.peer, self.shared);
        let now = self.config.clock.now();
        let due = self.versions_due.is_some_and(|at| at <= now);
        let awaiting = self
            .versions
            .refresh(
                &mut Context {
                    peer,
                    shared,
                    replicas: &mut *self.replicas,
                },
                due,
            )
            .await;
        // A status whose inputs change is computed again at once; one that failed for want of
        // the server (`Versions::refresh`) is tried again only when this deadline comes due,
        // less often each time, and not while the line is down: reconnecting forgets those
        // failures and refreshes.
        (self.versions_due, self.version_failures) = next_retry(
            awaiting && self.line.is_up(),
            now,
            self.versions_due,
            self.version_failures,
            jitter(),
        );
    }

    fn retry_at(&self) -> Option<Duration> {
        (self.line.is_up() && !self.tasks.0.syncing() && self.peer.pending().operations > 0)
            .then(|| self.drained_at + OUTBOX_RETRY)
    }

    async fn settle_outbox(&mut self) -> bool {
        let (peer, shared) = (self.peer, self.shared);
        if self.replicas.take_kept() {
            self.tasks.0.sync(peer, shared);
        }
        // A seal kept since the drain answered keeps its edits sending.
        for (block, refused) in self.tasks.0.take_published() {
            if refused || peer.pending_commits(block).is_empty() {
                let commits = match refused {
                    true => be_block_type_host::replicas::refused_commits(peer, block),
                    false => Vec::new(),
                };
                self.replicas.drained(block, refused, commits);
            }
        }
        let diverged = self.tasks.0.take_diverged();
        let merged = !diverged.is_empty();
        for (block, remote) in diverged {
            match self
                .replicas
                .merge_diverged((peer, shared), block, remote)
                .await
            {
                Ok(_) if peer.pending_commits(block).is_empty() => {
                    self.replicas.drained(block, false, Vec::new());
                }
                Ok(_) => {}
                Err(error) => record(shared, error),
            }
        }
        if !self.rejoin || self.tasks.0.syncing() {
            return merged;
        }
        self.rejoin = false;
        if self.line.is_up() {
            self.replicas.reconnect((peer, shared));
        }
        if !self.in_arrive {
            arrived(shared);
        }
        true
    }

    fn failures(&self) -> u32 {
        match self.line {
            Line::Down { failures, .. } => failures,
            Line::Up { .. } => 0,
        }
    }

    fn depart(&mut self, failures: u32) {
        let was_up = self.line.is_up();
        let delay = match failures {
            0 => Duration::ZERO,
            failures => reconnect_delay(failures - 1, jitter()),
        };
        self.line = Line::down(self.config, failures, delay);
        #[cfg(any(test, feature = "testing"))]
        if let Line::Down { at, .. } = &self.line {
            reconnecting_at(
                self.shared,
                (!delay.is_zero()).then(|| (self.config.clock.clone(), *at)),
            );
        }
        self.rejoin = false;
        if was_up {
            self.replicas.disconnected();
        }
        shared_offline(self.shared);
        self.changed.notify_all();
        be_host_base::host::wake();
    }

    async fn arrive(&mut self, open: Opened) {
        let (peer, shared) = (self.peer, self.shared);
        self.line = Line::up(peer, open);
        {
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            held.connected = true;
            held.connection = Some(peer.connection());
            #[cfg(test)]
            {
                held.connection_due = Some(peer.connection().due());
            }
            held.reached = true;
            held.arriving = true;
            held.error = None;
            #[cfg(any(test, feature = "testing"))]
            {
                held.reconnect = None;
            }
        }
        self.changed.notify_all();
        be_host_base::host::wake();
        self.drained_at = self.config.clock.now();
        self.tasks.0.sync(peer, shared);
        want_open(shared, &self.wanted.blocks());
        relist(shared);
        spawn_listings(peer, shared, &self.tasks.0);
        self.tasks.0.contacts(peer, shared);
        self.tasks.0.notify_modes(peer, shared);
        self.tasks.0.storage(peer, shared);
        self.rejoin = true;
        self.in_arrive = true;
        self.settle_outbox().await;
        for (block, content_type) in self.wanted.blocks() {
            open_wanted(
                peer,
                shared,
                &self.wanted,
                self.replicas,
                block,
                content_type,
            )
            .await;
        }
        self.replicas.pump((peer, shared)).await;
        self.versions.reconnected();
        self.refresh_versions().await;
        self.in_arrive = false;
        // Rejoined already when the outbox had nothing to drain; otherwise `settle_outbox` says
        // so once the drain ends and the replicas rejoin.
        if !self.rejoin {
            arrived(shared);
        }
        report(peer, shared);
        self.changed.notify_all();
        be_host_base::host::wake();
    }
}

/// The device has asked for everything it missed while away (`Shared::arriving`).
fn arrived(shared: &Arc<Mutex<Shared>>) {
    shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .arriving = false;
}

fn shared_offline(shared: &Arc<Mutex<Shared>>) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    held.connected = false;
    held.arriving = false;
    held.connection = None;
}

/// Publishes the outbox: answers the blocks that diverged, and those whose kept commits all
/// reached the server (false) or that the server refused (true, `Replicas::drained`).
async fn drain_outbox(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
) -> (Vec<(Uuid, Option<CommitId>)>, Vec<(Uuid, bool)>) {
    let mut diverged = Vec::new();
    let mut published = Vec::new();
    if peer.pending().operations == 0 {
        // What waited for the server to be reached goes now.
        be_block_type_host::replicas::send_announcements::<Shared>(peer, shared).await;
        return (diverged, published);
    }
    match peer.drain(|_| false).await {
        Ok(drained) => {
            // A block any of whose commits was refused or set aside keeps them unsent; one
            // that failed or diverged tries again.
            let refused: std::collections::HashSet<Uuid> = drained
                .iter()
                .filter_map(|outcome| match outcome {
                    be_block_client::Drained::Refused { block, .. }
                    | be_block_client::Drained::Unsent { block } => Some(*block),
                    _ => None,
                })
                .collect();
            let missed: std::collections::HashSet<Uuid> = drained
                .iter()
                .filter_map(|outcome| match outcome {
                    be_block_client::Drained::Diverged { block, .. }
                    | be_block_client::Drained::Failed { block } => Some(*block),
                    _ => None,
                })
                .chain(refused.iter().copied())
                .collect();
            published.extend(refused.iter().map(|block| (*block, true)));
            for outcome in drained {
                match outcome {
                    be_block_client::Drained::Published { block, .. }
                    | be_block_client::Drained::Created { block }
                        if !missed.contains(&block) && peer.pending_commits(block).is_empty() =>
                    {
                        published.push((block, false));
                    }
                    be_block_client::Drained::Diverged { block, remote, .. } => {
                        diverged.push((block, remote));
                    }
                    be_block_client::Drained::Refused { block, code } => record(
                        shared,
                        ClientError::Refused(
                            code,
                            format!("a change made offline to block {block} was refused"),
                        ),
                    ),
                    be_block_client::Drained::Failed { block } => {
                        shared.lock().unwrap_or_else(PoisonError::into_inner).error = Some(
                            format!("a change made offline to block {block} could not be sent yet"),
                        );
                    }
                    _ => {}
                }
            }
        }
        Err(error) => record(shared, error),
    }
    // The drain published commits holding announced edits.
    be_block_type_host::replicas::send_announcements::<Shared>(peer, shared).await;
    report(peer, shared);
    (diverged, published)
}

fn set_draining(shared: &Arc<Mutex<Shared>>, draining: bool) {
    {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.draining = draining;
        if !draining {
            held.drains += 1;
        }
    }
    be_host_base::host::wake();
}

pub(super) fn expired_unsent(
    unsent: &[be_block_client::UnsentBlock],
    company: bool,
    choice: &be_block::OfflineChoice,
    now: i64,
) -> Vec<Uuid> {
    if !company || choice.on_lost_access != be_block::LostAccess::Delete {
        return Vec::new();
    }
    let grace = i64::try_from(choice.grace.as_millis()).unwrap_or(i64::MAX);
    unsent
        .iter()
        .filter(|unsent| now.saturating_sub(unsent.since) >= grace)
        .map(|unsent| unsent.block)
        .collect()
}

pub(super) fn expire_unsent(peer: &Peer<Store>, now: i64) {
    let company = peer.kind() == be_block_protocol::OrganizationKind::Company;
    let choice = crate::offline_choice::choice(Uuid::nil());
    for block in expired_unsent(&peer.unsent(), company, &choice, now) {
        let _ = peer.discard_unsent(block);
        let _ = peer.forget_cached(block);
    }
}

fn report(peer: &Peer<Store>, shared: &Arc<Mutex<Shared>>) -> bool {
    let now = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .wall
        .now();
    expire_unsent(peer, now);
    let counts = peer.pending();
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let mut changed = held.waiting != counts.operations || held.recounting;
    held.waiting = counts.operations;
    held.recounting = false;
    if held.unsent.len() != counts.unsent {
        held.unsent = peer.unsent();
        changed = true;
    }
    changed
}

#[derive(Default)]
pub(super) struct Wanted {
    pub(super) open: HashMap<Uuid, Uuid>,
    pub(super) readers: HashSet<Uuid>,
    pub(super) editing: HashMap<Uuid, (Uuid, BTreeMap<EditorId, bool>)>,
}

impl Wanted {
    fn content_type(&self, block: Uuid) -> Option<Uuid> {
        self.open.get(&block).copied().or_else(|| {
            self.editing
                .get(&block)
                .map(|(content_type, _)| *content_type)
        })
    }

    fn wanted(&self, block: Uuid) -> bool {
        self.open.contains_key(&block)
            || self
                .editing
                .get(&block)
                .is_some_and(|(_, editors)| !editors.is_empty())
    }

    fn blocks(&self) -> HashMap<Uuid, Uuid> {
        let mut blocks = self.open.clone();
        for (block, (content_type, editors)) in &self.editing {
            if !editors.is_empty() {
                blocks.entry(*block).or_insert(*content_type);
            }
        }
        blocks
    }
}

async fn open_wanted(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    wanted: &Wanted,
    replicas: &mut Replicas,
    block: Uuid,
    content_type: Uuid,
) {
    #[cfg(feature = "testing")]
    let tested = cfg!(test) || crate::testing::in_process().is_some();
    #[cfg(not(feature = "testing"))]
    let tested = cfg!(test);
    let reader = tested || wanted.readers.contains(&block);
    replicas
        .open((peer, shared), block, content_type, reader)
        .await;
    if let Some((_, editors)) = wanted.editing.get(&block) {
        for (editor, editable) in editors {
            replicas.attach(block, *editor, *editable);
        }
    }
}

fn rekey_changed(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    keyed: &mut HashMap<Uuid, Uuid>,
) {
    let open = replicas.open_blocks();
    keyed.retain(|block, _| open.contains(block));
    for block in open {
        let Some(key) = peer.current_key(block) else {
            continue;
        };
        if keyed
            .insert(block, key)
            .is_some_and(|previous| previous != key)
        {
            replicas.seal_later(block, true);
        }
    }
}

enum Woken {
    Command(Command),
    Finished,
    Uploaded,
    Event(Option<Event>),
    Module,
    Deadline,
    Disconnected,
    Reached(Result<crate::link::Link, ClientError>),
    Nudged,
    Stopped,
}

#[cfg(test)]
async fn parked<T>(
    shared: &Mutex<Shared>,
    changed: &Condvar,
    armed: Option<Duration>,
    woken: impl std::future::Future<Output = T>,
) -> T {
    let mut woken = std::pin::pin!(woken);
    let woken = std::future::poll_fn(|context| {
        let polled = woken.as_mut().poll(context);
        if polled.is_pending() {
            shared.lock().unwrap_or_else(PoisonError::into_inner).parked = Some(armed);
            changed.notify_all();
        }
        polled
    })
    .await;
    shared.lock().unwrap_or_else(PoisonError::into_inner).parked = None;
    woken
}

async fn wait(
    commands: &mut UnboundedReceiver<Command>,
    line: &mut Line,
    nudged: &mut super::nudge::Nudged,
    uploads: &mut Uploads,
    replicas: &mut Replicas,
    finished: &mut UnboundedReceiver<()>,
    stopping: bool,
) -> Woken {
    let command = std::pin::pin!(async {
        match stopping {
            true => std::future::pending().await,
            false => match commands.recv().await {
                Some(command) => Woken::Command(command),
                None => Woken::Stopped,
            },
        }
    });
    let nudge = std::pin::pin!(async {
        match stopping {
            true => std::future::pending().await,
            false => match nudged.changed().await {
                Ok(()) => Woken::Nudged,
                Err(_) => Woken::Stopped,
            },
        }
    });
    let heard = std::pin::pin!(line.next());
    let uploaded = std::pin::pin!(async {
        uploads.next().await;
        Woken::Uploaded
    });
    let module = std::pin::pin!(async {
        replicas.woken().await;
        Woken::Module
    });
    let fetched = std::pin::pin!(async {
        match finished.recv().await {
            Some(()) => Woken::Finished,
            None => std::future::pending().await,
        }
    });
    let first = select(
        select(command, select(nudge, fetched)),
        select(heard, select(uploaded, module)),
    );
    match first.await {
        Either::Left((
            Either::Left((woken, _))
            | Either::Right((Either::Left((woken, _)) | Either::Right((woken, _)), _)),
            _,
        ))
        | Either::Right((
            Either::Left((woken, _))
            | Either::Right((Either::Left((woken, _)) | Either::Right((woken, _)), _)),
            _,
        )) => woken,
    }
}

struct Ran<'a> {
    peer: &'a Arc<Peer<Store>>,
    replicas: &'a mut Replicas,
    wanted: &'a mut Wanted,
    shared: &'a Arc<Mutex<Shared>>,
    config: &'a Config,
    versions: &'a mut Versions,
    flushes: &'a mut Vec<std::sync::mpsc::Sender<()>>,
    tasks: &'a Tasks,
}

const NEEDS_CONNECTION: &str =
    "This needs a connection to the server. Try again once be-host is back online.";

fn refuse_offline(command: Command, shared: &Arc<Mutex<Shared>>) -> Option<Command> {
    match command {
        // Opening a push needs only the keys the device kept.
        Command::Push(PushCommand::Open { .. }) => return Some(command),
        Command::Push(PushCommand::Key(reply)) => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::Push(
            PushCommand::Register { reply, .. } | PushCommand::Unregister { reply, .. },
        ) => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::SetAccess { .. } | Command::Leave(_) | Command::ApprovePairing { .. } => {
            shared.lock().unwrap_or_else(PoisonError::into_inner).error =
                Some(NEEDS_CONNECTION.to_owned());
        }
        Command::ListAccess { reply, .. } => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::SetNotifyMode { reply, .. } => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::Members(
            MembersCommand::List(reply)
            | MembersCommand::SetRole { reply, .. }
            | MembersCommand::Remove { reply, .. },
        ) => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::Invite { reply, .. } => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::CreateAgent { reply, .. } => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::CreateBlockAgent { reply, .. } => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::BlockAgent { change, .. } => change.fail(NEEDS_CONNECTION.to_owned()),
        Command::Contact(ContactCommand::Link { reply, .. }) => {
            reply(Err(NEEDS_CONNECTION.to_owned()))
        }
        Command::Contact(
            ContactCommand::Open { reply, .. } | ContactCommand::Accept { reply, .. },
        ) => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::Contact(ContactCommand::Remove(_)) => {
            shared.lock().unwrap_or_else(PoisonError::into_inner).error =
                Some(NEEDS_CONNECTION.to_owned());
        }
        command => return Some(command),
    }
    be_host_base::host::wake();
    None
}

impl Ran<'_> {
    async fn run(&mut self, command: Command) -> bool {
        let command = match self.peer.is_attached() {
            true => Some(command),
            false => refuse_offline(command, self.shared),
        };
        let Some(command) = command else {
            self.done();
            return false;
        };
        let applied = match command {
            Command::Contact(command) => {
                contact(self.peer, self.shared, self.config, self.tasks, command).await;
                false
            }
            Command::Authorship(command) => {
                crate::authorship::run(self.peer, self.tasks, command);
                false
            }
            Command::Storage => {
                self.tasks.storage(self.peer, self.shared);
                false
            }
            Command::Members(command) => {
                members(self.peer, self.shared, command).await;
                false
            }
            Command::Invite { email, role, reply } => {
                reply(
                    invite(self.peer, self.config, email, role)
                        .await
                        .map_err(|error| error.to_string()),
                );
                be_host_base::host::wake();
                false
            }
            Command::CreateAgent { name, reply } => {
                let created = create_agent(self.peer, self.config, name, None)
                    .await
                    .map(|(_, credentials)| credentials);
                if created.is_ok() {
                    self.tasks.sync_contacts(self.peer, self.shared).await;
                }
                reply(created.map_err(|error| error.to_string()));
                be_host_base::host::wake();
                false
            }
            Command::CreateBlockAgent { block, name, reply } => {
                let created = create_agent(self.peer, self.config, name, Some(block)).await;
                if created.is_ok() {
                    self.tasks.sync_contacts(self.peer, self.shared).await;
                    self.tasks.refresh(self.peer, self.shared, block);
                }
                reply(created.map_err(|error| error.to_string()));
                be_host_base::host::wake();
                false
            }
            // Not a task of the worker's: its end changes nothing the worker keeps.
            Command::SetNotifyMode { block, mode, reply } => {
                let (peer, shared) = (Arc::clone(self.peer), Arc::clone(self.shared));
                platform::spawn_local(async move {
                    let set = peer.set_notify_mode(block, mode).await;
                    if set.is_ok() {
                        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                        if let Some(modes) = &mut held.notify_modes {
                            match mode {
                                be_block_protocol::NotifyMode::All => modes.remove(&block),
                                mode => modes.insert(block, mode),
                            };
                            held.notify_modes_revision += 1;
                        }
                    }
                    reply(set.map(|()| mode).map_err(|error| error.to_string()));
                    be_host_base::host::wake();
                });
                false
            }
            Command::Push(command) => {
                let peer = Arc::clone(self.peer);
                platform::spawn_local(async move {
                    match command {
                        PushCommand::Key(reply) => {
                            reply(peer.push_key().await.map_err(|error| error.to_string()))
                        }
                        PushCommand::Register { endpoint, reply } => reply(
                            peer.register_endpoint(endpoint)
                                .await
                                .map_err(|error| error.to_string()),
                        ),
                        PushCommand::Unregister { address, reply } => reply(
                            peer.unregister_endpoint(address)
                                .await
                                .map_err(|error| error.to_string()),
                        ),
                        PushCommand::Open {
                            message,
                            now,
                            reply,
                        } => reply(
                            peer.open_notification(&message, now)
                                .map_err(|error| error.to_string()),
                        ),
                    }
                    be_host_base::host::wake();
                });
                false
            }
            Command::BlockAgent { agent, change } => {
                change_block_agent(self.peer, agent, change).await;
                self.tasks.sync_contacts(self.peer, self.shared).await;
                be_host_base::host::wake();
                false
            }
            command => {
                apply(
                    self.peer,
                    self.replicas,
                    self.wanted,
                    self.shared,
                    self.versions,
                    self.flushes,
                    self.tasks,
                    command,
                )
                .await
            }
        };
        self.done();
        applied
    }

    fn done(&self) {
        {
            let mut held = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
            held.pending = held.pending.saturating_sub(1);
            (held.unsealed, held.owing, held.undurable) = self.replicas.counts();
            held.recounting = true;
            held.typing = !self.replicas.modules.settled();
        }
        be_host_base::host::wake();
    }
}

fn asked_to_pair(shared: &Arc<Mutex<Shared>>, from: u64, payload: &[u8]) {
    let Ok(be_block_keys::PairingMessage::Request { message, device }) =
        be_block_keys::PairingMessage::decode(payload)
    else {
        return;
    };
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    held.pairing.retain(|request| request.from != from);
    held.pairing.push(PairingRequest {
        from,
        device,
        message,
    });
    if held.pairing.len() > PAIRING_REQUEST_LIMIT {
        held.pairing.remove(0);
    }
    drop(held);
    be_host_base::host::wake();
}

async fn approve_pairing(
    peer: &Peer<Store>,
    shared: &Arc<Mutex<Shared>>,
    from: u64,
    code: &str,
) -> Result<(), ClientError> {
    let request = {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(index) = held.pairing.iter().position(|request| request.from == from) else {
            return Ok(());
        };
        held.pairing.remove(index)
    };
    let Some(code) = be_block_keys::normalize_code(code) else {
        return Err(ClientError::Encoding(
            "that is not a pairing code".to_owned(),
        ));
    };
    let (pairing, message) = be_block_keys::Pairing::start(&code, peer.account().as_bytes());
    let key = pairing
        .finish(&request.message)
        .map_err(|error| ClientError::Encoding(error.to_string()))?;
    let sealed = key.seal(&peer.keys().identity().to_bytes());
    let payload = be_block_keys::PairingMessage::Reply { message, sealed }.encode();
    ask(client::account::pair(
        &peer.connection(),
        Some(from),
        payload,
    ))
    .await?;
    Ok(())
}

async fn sync_contacts(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) {
    let identity = peer.keys().identity();
    match be_block_client::contacts::finish_links(&peer.connection(), identity, peer.organization())
        .await
    {
        Ok(finished) => {
            for done in finished {
                let Some(grant) = done
                    .grant
                    .filter(|grant| grant.organization == peer.organization())
                else {
                    continue;
                };
                let account = Uuid::from_bytes(done.card.account);
                if let Err(error) = peer
                    .seal_share(grant.block, account, &done.card.public)
                    .await
                {
                    record(shared, error);
                }
            }
        }
        Err(error) => record(shared, error),
    }
    match peer.refresh_contacts().await {
        Ok(contacts) => {
            let changed = {
                let mut shared = shared.lock().unwrap_or_else(PoisonError::into_inner);
                let changed = shared.contacts != contacts;
                if changed {
                    shared.contacts = contacts;
                    shared.contacts_revision += 1;
                }
                changed
            };
            if changed {
                relist(shared);
            }
        }
        Err(error) => record(shared, error),
    }
    tend(peer, shared).await;
    be_host_base::host::wake();
}

async fn tend(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) {
    if let Err(error) = peer.tend().await {
        record(shared, error);
    }
    shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .escrow_warning = peer
        .keys()
        .escrow_warning()
        .map(be_block_client::EscrowWarning::message);
    be_host_base::host::wake();
}

async fn members(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, command: MembersCommand) {
    let (changed, reply) = match command {
        MembersCommand::List(reply) => (Ok(()), reply),
        MembersCommand::SetRole {
            account,
            role,
            reply,
        } => (peer.set_role(account, role).await, reply),
        MembersCommand::Remove { account, reply } => (peer.remove_member(account).await, reply),
    };
    if changed.is_ok() {
        tend(peer, shared).await;
    }
    let listed = match changed {
        Ok(()) => peer.members().await,
        Err(error) => Err(error),
    };
    reply(listed.map_err(|error| error.to_string()));
    be_host_base::host::wake();
}

async fn invite(
    peer: &Peer<Store>,
    config: &Config,
    email: String,
    role: be_block_protocol::OrganizationRole,
) -> Result<String, ClientError> {
    let identity = peer.keys().identity();
    let me = be_block_client::contacts::card(
        peer.account(),
        identity,
        &config.display_name,
        &config.email,
    );
    let code = be_block_client::contacts::invite(
        &peer.connection(),
        identity,
        &me,
        peer.organization(),
        email,
        role,
        peer.keys().escrow().as_ref(),
    )
    .await?;
    Ok(crate::invite_link::link(
        &crate::accounts::link_base(&config.server_url),
        &code,
    ))
}

/// Makes an agent, a contact of the person's from the start; one `bound_to` a block also gets
/// the block's key sealed to it, as sharing would.
async fn create_agent(
    peer: &Peer<Store>,
    config: &Config,
    name: String,
    bound_to: Option<Uuid>,
) -> Result<(Uuid, crate::accounts::AgentCredentials), ClientError> {
    let identity = peer.keys().identity();
    let me = be_block_client::contacts::card(
        peer.account(),
        identity,
        &config.display_name,
        &config.email,
    );
    let organization = peer.organization();
    let phrase = be_block_keys::RecoveryPhrase::generate();
    let secret = phrase.secret();
    let public = secret.public();
    let be_block_protocol::CreatedAgent { agent, token } = ask(client::members::create_agent(
        &peer.connection(),
        organization,
        name,
        public.0,
        secret.signing().0,
        me.seal_own(&secret),
        peer.keys().escrow().map_or_else(Vec::new, |escrow| {
            be_block_client::contacts::seal_pin(&secret, organization, &escrow)
        }),
        bound_to,
    ))
    .await?;
    let card = be_block_keys::ContactCard {
        account: agent.account.into_bytes(),
        public,
        signing: secret.signing(),
        display_name: agent.name.clone(),
        email: agent.email.clone(),
    };
    let finished = async {
        ask(client::contacts::put_contact(
            &peer.connection(),
            agent.account,
            card.seal_own(identity),
        ))
        .await?;
        if let Some(block) = bound_to {
            peer.seal_share(block, agent.account, &public).await?;
        }
        Ok::<_, ClientError>(())
    }
    .await;
    if let Err(error) = finished {
        // An agent the person never got the credentials of is of no use to anyone.
        let _ = ask(client::members::remove_agent(
            &peer.connection(),
            organization,
            agent.account,
        ))
        .await;
        return Err(error);
    }
    Ok((
        agent.account,
        crate::accounts::AgentCredentials {
            name: agent.name,
            server: config.server_url.clone(),
            organization,
            token,
            phrase: phrase.to_string(),
        },
    ))
}

/// Rotates the token of, or removes, an agent bound to a block; refuses one of the person's
/// own agents, which only the Agents menu changes.
async fn change_block_agent(peer: &Peer<Store>, agent: Uuid, change: AgentChange) {
    let connection = peer.connection();
    let organization = peer.organization();
    let own = match ask(client::members::list_agents(&connection, organization)).await {
        Ok(agents) => agents.iter().any(|listed| listed.account == agent),
        Err(error) => return change.fail(error.to_string()),
    };
    if own {
        return change.fail("That agent is not a block's agent.".to_owned());
    }
    match change {
        AgentChange::Rotate(reply) => reply(
            ask(client::members::rotate_agent_token(
                &connection,
                organization,
                agent,
            ))
            .await
            .map_err(|error| error.to_string()),
        ),
        // An agent the server no longer has (removed, or collected with its block) is as
        // removed as asked.
        AgentChange::Remove(reply) => reply(
            match ask(client::members::remove_agent(
                &connection,
                organization,
                agent,
            ))
            .await
            {
                Ok(_)
                | Err(be_block_client::ClientError::Refused(
                    be_block_protocol::ErrorCode::AgentNotFound,
                    _,
                )) => Ok(()),
                Err(error) => Err(error.to_string()),
            },
        ),
    }
}

async fn contact(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    config: &Config,
    tasks: &Tasks,
    command: ContactCommand,
) {
    let identity = peer.keys().identity();
    let me = be_block_client::contacts::card(
        peer.account(),
        identity,
        &config.display_name,
        &config.email,
    );
    match command {
        ContactCommand::Link { offer, reply } => {
            let made = async {
                let offer = match offer {
                    Some((block, access)) => Some(peer.offer(block, access).await?),
                    None => None,
                };
                be_block_client::contacts::make_link(
                    &peer.connection(),
                    identity,
                    &me,
                    offer.as_ref(),
                )
                .await
            }
            .await;
            reply(
                made.map(|code| {
                    crate::invite_link::contact_link(
                        &crate::accounts::link_base(&config.server_url),
                        &code,
                    )
                })
                .map_err(|error| error.to_string()),
            );
        }
        ContactCommand::Open { code, reply } => {
            let opened = be_block_client::contacts::open_link(&peer.connection(), &code)
                .await
                .map(|(_, card)| card);
            reply(opened.map_err(|error| error.to_string()));
        }
        ContactCommand::Accept { code, reply } => {
            let accepted =
                be_block_client::contacts::accept_link(&peer.connection(), identity, &me, &code)
                    .await;
            if accepted.is_ok() {
                tasks.sync_contacts(peer, shared).await;
            }
            reply(accepted.map_err(|error| error.to_string()));
        }
        ContactCommand::Remove(account) => {
            match be_block_client::contacts::remove_contact(&peer.connection(), account).await {
                Ok(()) => tasks.sync_contacts(peer, shared).await,
                Err(error) => record(shared, error),
            }
        }
    }
    be_host_base::host::wake();
}

pub(super) fn set_locked(shared: &Arc<Mutex<Shared>>, block: Uuid, locked: bool) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let changed = match locked {
        true => held.locked.insert(block),
        false => held.locked.remove(&block),
    };
    if changed {
        held.stale_revision += 1;
        held.touch(block);
        drop(held);
        be_host_base::host::wake();
    }
}

pub(super) fn set_unreadable(shared: &Arc<Mutex<Shared>>, block: Uuid, error: Option<String>) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let changed = match error {
        Some(error) => held.unreadable.insert(block, error.clone()) != Some(error),
        None => held.unreadable.remove(&block).is_some(),
    };
    if changed {
        held.stale_revision += 1;
        held.touch(block);
        drop(held);
        be_host_base::host::wake();
    }
}

fn remember_head(peer: &Peer<Store>, shared: &Arc<Mutex<Shared>>, block: Uuid) {
    shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .graph
        .set_head(block, peer.local_head(block));
}

fn version_revisions(shared: &Arc<Mutex<Shared>>) -> u64 {
    shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .versions
        .values()
        .map(|state| state.revision)
        .sum()
}

fn set_busy(shared: &Arc<Mutex<Shared>>, block: Uuid, busy: bool, error: Option<String>) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let state = held.versions.entry(block).or_default();
    state.status.busy = busy;
    state.status.error = error;
    state.revision += 1;
    held.touch(block);
}

async fn apply(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    wanted: &mut Wanted,
    shared: &Arc<Mutex<Shared>>,
    versions: &mut Versions,
    flushes: &mut Vec<std::sync::mpsc::Sender<()>>,
    tasks: &Tasks,
    command: Command,
) -> bool {
    let ctx = (peer, shared);
    match command {
        Command::Contact(_)
        | Command::Members(_)
        | Command::Invite { .. }
        | Command::CreateAgent { .. }
        | Command::CreateBlockAgent { .. }
        | Command::BlockAgent { .. }
        | Command::SetNotifyMode { .. }
        | Command::Push(_)
        | Command::Authorship(_)
        | Command::Storage
        | Command::Sync => false,
        Command::Leave(block) => {
            if let Err(error) = peer.leave(block).await {
                record(shared, error);
            }
            tasks.refresh(peer, shared, block);
            true
        }
        Command::WatchVersion(block) => {
            Versions::watch(shared, block);
            false
        }
        Command::ApprovePairing { from, code } => {
            if let Err(error) = approve_pairing(peer, shared, from, &code).await {
                record(shared, error);
            }
            be_host_base::host::wake();
            false
        }
        Command::DismissPairing(from) => {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pairing
                .retain(|request| request.from != from);
            be_host_base::host::wake();
            false
        }
        Command::Version { block, command } => {
            set_busy(shared, block, true, None);
            be_host_base::host::wake();
            replicas.seal_all(ctx).await;
            let outcome = versions
                .run(
                    &mut Context {
                        peer,
                        shared,
                        replicas,
                    },
                    block,
                    command,
                )
                .await;
            set_busy(shared, block, false, outcome.err());
            true
        }
        Command::PreviewChild {
            alias,
            source,
            content_type,
        } => {
            super::revisions::load_child(peer, shared, alias, source, content_type).await;
            true
        }
        Command::Unsent { block, restore } => {
            let done = match restore {
                true => peer.restore_unsent(block),
                false => peer.discard_unsent(block),
            };
            if restore {
                tasks.sync(peer, shared);
            }
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            if let Err(error) = done {
                held.error = Some(error.to_string());
            }
            held.unsent = peer.unsent();
            held.stale_revision += 1;
            drop(held);
            be_host_base::host::wake();
            true
        }
        Command::Revision {
            block,
            request: super::revisions::Asked::List,
            reply,
        } => {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .listing_revisions += 1;
            let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
            tasks.spawn(async move {
                reply(super::revisions::Answered::Listed(
                    super::revisions::list(&peer, block).await,
                ));
                shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .listing_revisions -= 1;
                be_host_base::host::wake();
            });
            true
        }
        Command::Revision {
            block,
            request,
            reply,
        } => {
            let result = super::revisions::run(peer, replicas, shared, block, request).await;
            reply(result);
            true
        }
        Command::Open {
            block,
            content_type,
            owned,
        } => {
            if owned {
                shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .owned
                    .insert(block);
                wanted.readers.insert(block);
            }
            wanted.open.insert(block, content_type);
            want_open(shared, &wanted.blocks());
            open_wanted(peer, shared, wanted, replicas, block, content_type).await;
            false
        }
        #[cfg(any(test, feature = "testing"))]
        Command::Close(block) => {
            wanted.open.remove(&block);
            wanted.readers.remove(&block);
            want_open(shared, &wanted.blocks());
            if !wanted.wanted(block) {
                replicas.close(block);
            }
            false
        }
        Command::Attach {
            block,
            content_type,
            editor,
            editable,
        } => {
            let (_, editors) = wanted
                .editing
                .entry(block)
                .or_insert_with(|| (content_type, BTreeMap::new()));
            editors.insert(editor, editable);
            want_open(shared, &wanted.blocks());
            match replicas.is_open(block) {
                true => replicas.attach(block, editor, editable),
                false => {
                    let content_type = wanted.content_type(block).unwrap_or(content_type);
                    open_wanted(peer, shared, wanted, replicas, block, content_type).await;
                }
            }
            false
        }
        Command::Detach { block, editor } => {
            if let Some((_, editors)) = wanted.editing.get_mut(&block) {
                editors.remove(&editor);
                if editors.is_empty() {
                    wanted.editing.remove(&block);
                }
            }
            replicas.detach(block, editor);
            want_open(shared, &wanted.blocks());
            if !wanted.wanted(block) {
                replicas.close(block);
            }
            false
        }
        Command::FromEditor {
            block,
            editor,
            bytes,
            at,
        } => {
            replicas.relay_from_editor(block, editor, bytes, at);
            false
        }
        Command::HoldClock(at) => {
            replicas.modules.hold_clock(at);
            false
        }
        Command::ChangeChild {
            block,
            content_type,
            change,
        } => {
            if !replicas.is_open(block) {
                wanted.open.entry(block).or_insert(content_type);
                want_open(shared, &wanted.blocks());
                open_wanted(peer, shared, wanted, replicas, block, content_type).await;
            }
            let change = Change::Child(change);
            replicas.change_later(block, change);
            false
        }
        Command::FromFile {
            block,
            content_type,
            name,
            bytes,
        } => {
            if !replicas.is_open(block) {
                wanted.open.entry(block).or_insert(content_type);
                want_open(shared, &wanted.blocks());
                open_wanted(peer, shared, wanted, replicas, block, content_type).await;
            }
            let change = Change::FromFile(be_sdk_type_api::FileChange { name, bytes });
            replicas.change_later(block, change);
            false
        }
        Command::Operate(block, operation, at) => {
            replicas.send_as_host(block, &FromEditor::Operate(operation), at);
            false
        }
        #[cfg(test)]
        Command::Presence { block, kind, value } => {
            replicas.send_as_host(
                block,
                &FromEditor::ShowPresence { kind, value },
                Instant::now(),
            );
            false
        }
        #[cfg(test)]
        Command::HearHead { block, head } => {
            let mut events = vec![Event::HeadChanged {
                block,
                head,
                author: peer.account(),
            }];
            if let Some(mut summary) = peer.known_summary(block) {
                summary.head = Some(head);
                events.push(Event::BlockChanged(summary));
            }
            for event in events {
                heard(peer, shared, tasks, replicas, Some(event)).await;
            }
            false
        }
        #[cfg(test)]
        Command::SendToPeers {
            block,
            to,
            channel,
            payload,
        } => {
            replicas.send_as_host(
                block,
                &FromEditor::SendToPeers {
                    to,
                    channel,
                    payload,
                },
                Instant::now(),
            );
            false
        }
        Command::History { block, redo } => {
            let change = match redo {
                false => Change::Undo,
                true => Change::Redo,
            };
            replicas.change_later(block, change);
            false
        }
        Command::Duplicate {
            from,
            to,
            content_type,
            metadata,
        } => {
            if !super::is_known(content_type) {
                shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .graph
                    .settle(to);
                return false;
            }
            // Some types keep parts that their bytes cannot carry (`WHOLE_BYTES` is false: chat's
            // closed segments), and a copy through bytes would drop them: refuse until a copy of
            // parts exists.
            if replicas
                .kind(content_type)
                .await
                .is_some_and(|kind| !kind.whole_bytes)
                && has_later_parts(peer, from).await.unwrap_or(false)
            {
                record(
                    shared,
                    ClientError::Encoding(
                        "this block keeps parts after its first, which a copy does not carry yet"
                            .to_string(),
                    ),
                );
                shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .graph
                    .settle(to);
                return false;
            }
            let created = peer
                .create_block(to, content_type, BlockParent::Detached, &metadata, false)
                .await;
            if let Err(error) = settled(peer, shared, Some(tasks), to, created).await {
                record(shared, error);
                return false;
            }
            let shown = match replicas.is_open(from) {
                true => replicas.content(ctx, from).await,
                false => None,
            };
            let bytes = match shown {
                Some(bytes) => Some(bytes),
                None => stored(peer, from).await.unwrap_or_else(|error| {
                    record(shared, error);
                    None
                }),
            };
            let bytes = match bytes {
                Some(bytes) => Some(bytes),
                None => replicas.kind(content_type).await.map(|kind| kind.blank),
            };
            if let Some(bytes) = bytes
                && let Err(error) =
                    publish_bytes(peer, replicas, shared, to, content_type, bytes, true).await
            {
                record(shared, error);
            }
            remember_head(peer, shared, to);
            false
        }
        Command::Replace {
            block,
            content_type,
            bytes,
        } => {
            let unreadable = replicas
                .status(block)
                .is_some_and(|status| status.unreadable.is_some());
            let outcome = match replicas.is_open(block) && !unreadable {
                true => {
                    replicas.change(ctx, block, Change::Replace(bytes)).await;
                    Ok(())
                }
                false => match peer
                    .ensure_type(block, content_type, BlockParent::Detached)
                    .await
                {
                    Ok(()) => {
                        publish_bytes(peer, replicas, shared, block, content_type, bytes, true)
                            .await
                    }
                    Err(error) => Err(error),
                },
            };
            if let Err(error) = outcome {
                record(shared, error);
            }
            if unreadable {
                replicas.reopen(block);
            }
            remember_head(peer, shared, block);
            false
        }
        Command::Seed {
            block,
            content_type,
            bytes,
        } => {
            if replicas.is_open(block) {
                return false;
            }
            if let Err(error) = seed(peer, replicas, shared, block, content_type, bytes).await {
                record(shared, error);
            }
            remember_head(peer, shared, block);
            false
        }
        Command::Create {
            block,
            content_type,
            parent,
            mut metadata,
            bytes,
            owner_only,
        } => {
            let kind = replicas.kind(content_type).await;
            let bytes = bytes.or_else(|| kind.as_ref().map(|kind| kind.blank.clone()));
            if metadata.derived.is_empty()
                && let Some(kind) = kind
            {
                metadata.derived = match &bytes {
                    Some(bytes) => replicas
                        .describe(ctx, content_type, bytes)
                        .await
                        .map(|description| description.derived)
                        .unwrap_or_default(),
                    None => kind.described.derived,
                };
            }
            let created = peer
                .create_block(block, content_type, parent, &metadata, owner_only)
                .await;
            if let Err(error) = settled(peer, shared, Some(tasks), block, created).await {
                record(shared, error);
                return false;
            }
            if let Some(bytes) = bytes
                && let Err(error) = seed(peer, replicas, shared, block, content_type, bytes).await
            {
                record(shared, error);
            }
            remember_head(peer, shared, block);
            true
        }
        Command::SetParent { block, parent } => {
            let moved = peer.set_parent(block, parent).await;
            let settled = shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .graph
                .settle(block);
            if let Err(error) = moved {
                record(shared, error);
            }
            if settled {
                tasks.refresh(peer, shared, block);
            }
            true
        }
        Command::Define { block, changes } => {
            let defined = peer
                .define(block, |metadata| metadata.define(&changes))
                .await;
            if let Ok(summary) = &defined
                && let Some(head) = peer.local_head(block).or(summary.head)
            {
                replicas.moved_here(block, head);
            }
            if let Err(error) = settled(peer, shared, Some(tasks), block, defined).await {
                record(shared, error);
            }
            true
        }
        Command::SetAccess {
            block,
            account,
            access,
        } => {
            let verified = shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .contacts
                .iter()
                .find(|contact| contact.account == account)
                .and_then(|contact| contact.public);
            let granted = match verified {
                _ if account == be_block_graph::ANYONE_INSIDE => {
                    peer.open_inside(block, access).await
                }
                Some(public) => peer.share(block, account, access, &public).await,
                None => peer.grant(block, account, access).await,
            };
            if let Err(error) = granted {
                record(shared, error);
            }
            tend(peer, shared).await;
            true
        }
        Command::ListAccess { block, reply } => {
            let listed = peer
                .list_access(block)
                .await
                .map_err(|error| error.to_string());
            reply(listed);
            be_host_base::host::wake();
            false
        }
        Command::ReadHead { commit, reply, .. } => {
            let read = async {
                let commit = peer.load_commit(commit).await?;
                peer.read_manifest(&commit.manifest).await
            }
            .await;
            reply(read.map_err(|error| error.to_string()));
            be_host_base::host::wake();
            false
        }
        Command::Export { block, reply } => {
            let exported = export(peer, replicas, shared, block).await;
            reply(exported.map_err(|error| error.to_string()));
            be_host_base::host::wake();
            false
        }
        Command::PlainFile { block, reply } => {
            reply(plain_file(peer, replicas, shared, block).await);
            be_host_base::host::wake();
            false
        }
        Command::Import(_) => false,
        Command::ContentOfFile { name, bytes, reply } => {
            reply(replicas.from_file(ctx, &name, bytes).await);
            be_host_base::host::wake();
            false
        }
        Command::Flush(done) => {
            replicas.keep_all(ctx).await;
            if replicas.take_kept() {
                tasks.sync(peer, shared);
            }
            flushes.push(done);
            true
        }
    }
}

async fn plain_file(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
) -> Result<be_block_type_host::PlainFile, String> {
    let (content_type, name) = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .graph
        .get(block)
        .map(|node| (node.content_type, node.metadata.name().unwrap_or_default()))
        .ok_or("This block isn't known yet.")?;
    let content = match replicas.is_open(block) {
        true => replicas.content((peer, shared), block).await,
        false => None,
    };
    let content = match content {
        Some(content) => Some(content),
        None => stored(peer, block)
            .await
            .map_err(|error| error.to_string())?,
    };
    let bytes = match content {
        Some(bytes) => bytes,
        None => replicas
            .kind(content_type)
            .await
            .map(|kind| kind.blank)
            .ok_or("This block's type isn't known.")?,
    };
    let mut file = replicas
        .to_file((peer, shared), content_type, bytes)
        .await?
        .ok_or("This block has no plain-file form.")?;
    if file.name.is_none() {
        file.name = Some(be_block::plain::export_name(&name, &file.extension));
    }
    Ok(file)
}

async fn stored(peer: &Peer<Store>, block: Uuid) -> Result<Option<Vec<u8>>, ClientError> {
    let Some(head) = peer.newest_head(block).await? else {
        return Ok(None);
    };
    let commit = peer.load_commit(head).await?;
    Ok(Some(peer.read_manifest(&commit.manifest).await?))
}

/// Whether the block's newest save holds more than the first part a type loads.
pub(super) async fn has_later_parts(peer: &Peer<Store>, block: Uuid) -> Result<bool, ClientError> {
    let Some(head) = peer.newest_head(block).await? else {
        return Ok(false);
    };
    let commit = peer.load_commit(head).await?;
    let first = peer.load_first_part(&commit.manifest).await?;
    Ok((first.bytes.len() as u64) < commit.manifest.length())
}

async fn seed(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    content_type: Uuid,
    bytes: Vec<u8>,
) -> Result<(), ClientError> {
    if !super::is_known(content_type) {
        return Ok(());
    }
    peer.ensure_type(block, content_type, BlockParent::Detached)
        .await?;
    if peer.newest_head(block).await?.is_some() {
        return Ok(());
    }
    publish_bytes(peer, replicas, shared, block, content_type, bytes, false).await
}

async fn publish_bytes(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    content_type: Uuid,
    bytes: Vec<u8>,
    retry: bool,
) -> Result<(), ClientError> {
    let Some(description) = replicas
        .describe((peer, shared), content_type, &bytes)
        .await
    else {
        return Ok(());
    };
    let mut expected = peer.newest_head(block).await?;
    for _ in 0..4 {
        match peer
            .write(
                block,
                Write::encoded(content_type, &bytes, expected)
                    .described(description.references.clone(), description.derived.clone()),
            )
            .await?
        {
            be_block_client::Saved::Rejected { head } if retry => expected = head,
            _ => return Ok(()),
        }
    }
    Ok(())
}

async fn imported(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    imported: be_block_client::peer::Imported,
) {
    if let Err(error) = settled(peer, shared, None, block, imported).await {
        record(shared, error);
    }
    remember_head(peer, shared, block);
}

async fn heard(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    tasks: &Tasks,
    replicas: &Replicas,
    event: Option<Event>,
) {
    match event {
        Some(Event::BlockChanged(block)) => {
            if let Err(error) = peer.learn(std::slice::from_ref(&block)).await {
                record(shared, error);
            }
            let mut node = node_of(peer, &block);
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            if replicas.is_open(node.id)
                && let Some(reported) = held.graph.get(node.id).and_then(|node| node.head)
            {
                node.head = Some(reported);
            }
            let shared_into_view = node.access.can_view()
                && matches!(node.parent, Some(BlockParent::Block(_)))
                && !held.graph.covers(node.id);
            held.graph.put(node, block.revision);
            if shared_into_view && !held.graph.covers(block.id) {
                held.graph.relist(Listing::Shared);
            }
            if block.opens_inside {
                held.graph.relist_open_settings();
            }
        }
        Some(Event::BlockRemoved { block, revision }) => {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .graph
                .remove(block, revision);
        }
        Some(Event::HeadChanged { block, head, .. }) if !replicas.is_open(block) => {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .graph
                .set_head(block, Some(head));
        }
        Some(Event::Paired(be_block_protocol::PairingMessage { from, payload })) => {
            asked_to_pair(shared, from, &payload);
        }
        Some(Event::ContactsChanged) => tasks.contacts(peer, shared),
        Some(Event::NotifyModesChanged) => tasks.notify_modes(peer, shared),
        Some(Event::KeyWorkChanged) => {
            let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
            tasks.spawn(async move { tend(&peer, &shared).await });
        }
        Some(_) => {}
        None => relist(shared),
    }
}

async fn export(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
) -> Result<(String, Vec<u8>), ClientError> {
    let picked = peer.summary(block).await?;
    if picked.owner_only {
        return Err(ClientError::Refused(
            be_block_protocol::ErrorCode::PermissionDenied,
            "a block only you can read is not exported, since a block file opens for anyone".into(),
        ));
    }
    let mut top = picked.clone();
    if let Some(BlockParent::Block(parent)) = picked.parent
        && let Ok(whole) = peer.summary(parent).await
        && crate::discovery::exported_whole(whole.content_type)
    {
        top = whole;
    }
    let mut found = vec![(top.clone(), None)];
    let mut level = vec![(top.id, 0)];
    while !level.is_empty() {
        let below = futures_util::future::try_join_all(
            level
                .iter()
                .map(|(parent, _)| peer.children(BlockParent::Block(*parent))),
        )
        .await?;
        let mut next = Vec::new();
        for ((_, position), children) in level.into_iter().zip(below) {
            for child in children {
                if child.access.can_view()
                    && !child.owner_only
                    && found.iter().all(|(held, _)| held.id != child.id)
                {
                    next.push((child.id, found.len()));
                    found.push((child, Some(position)));
                }
            }
        }
        level = next;
    }
    let summaries: Vec<BlockSummary> = found.iter().map(|(summary, _)| summary.clone()).collect();
    peer.learn(&summaries).await?;
    let mut blocks: Vec<be_block_client::transfer::Saved> = found
        .into_iter()
        .map(|(summary, parent)| be_block_client::transfer::Saved {
            id: summary.id,
            block_type: summary.content_type,
            parent,
            metadata: node_of(peer, &summary).metadata,
            references: summary.references,
            content: None,
        })
        .collect();
    let heads: HashMap<Uuid, Option<be_block_commit::CommitId>> = summaries
        .iter()
        .map(|summary| (summary.id, summary.head))
        .collect();
    // A block file carries bytes, which drop the parts after the first of a type that loads
    // only its first part (chat's closed segments): refuse until a file carries parts.
    for block in &blocks {
        if replicas
            .kind(block.block_type)
            .await
            .is_some_and(|kind| !kind.whole_bytes)
            && has_later_parts(peer, block.id).await?
        {
            return Err(ClientError::Encoding(
                "a block that keeps parts after its first is not exported yet".to_string(),
            ));
        }
    }
    let wanted: Vec<_> = blocks
        .iter()
        .filter(|block| !replicas.is_open(block.id))
        .map(|block| (block.id, heads[&block.id]))
        .collect();
    let mut read = peer.read_contents(&wanted).await?.into_iter();
    for block in &mut blocks {
        block.content = match replicas.is_open(block.id) {
            true => match replicas.content((peer, shared), block.id).await {
                Some(bytes) => Some(bytes),
                None => stored(peer, block.id).await?,
            },
            false => match read.next() {
                Some(read) => replicas.modules.readable(read).await,
                None => None,
            },
        };
        if let Some(derived) = shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .graph
            .get(block.id)
            .map(|node| node.metadata.derived.clone())
        {
            block.metadata.derived = derived;
        }
    }
    let name = blocks[0].metadata.name().unwrap_or_default();
    Ok((
        be_block_client::transfer::file_name(&name),
        be_block_client::transfer::encode(&blocks),
    ))
}

fn relist(shared: &Arc<Mutex<Shared>>) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    held.graph.relist_all();
    held.stale_revision += 1;
}

pub(super) async fn fetch_listings(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) -> bool {
    let mut fetched = false;
    loop {
        if !peer.is_attached() {
            return fetched;
        }
        let (unlisted, wanted) = take_listings(shared);
        if wanted.is_empty() {
            futures_util::future::join_all(
                unlisted.into_iter().map(|listing| peer.unlist(listing)),
            )
            .await;
            return fetched;
        }
        fetched = true;
        list(peer, shared, unlisted, wanted).await;
    }
}

/// Starts fetching what is asked for and not yet listed, while the connection is up: a listing
/// asked for over a closed one stays asked for, and the connection that replaces it fetches it.
pub(super) fn spawn_listings(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, tasks: &Tasks) {
    if !peer.is_attached() {
        return;
    }
    let (unlisted, wanted) = take_listings(shared);
    if unlisted.is_empty() && wanted.is_empty() {
        return;
    }
    let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
    tasks.spawn(async move { list(&peer, &shared, unlisted, wanted).await });
}

#[derive(Clone)]
pub(super) struct Tasks {
    finished: tokio::sync::mpsc::UnboundedSender<()>,
    contacts: Arc<tokio::sync::Mutex<()>>,
    outbox: std::rc::Rc<std::cell::RefCell<Outbox>>,
    /// Stops the task following the account's storage, while one does.
    storage: std::rc::Rc<std::cell::RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
}

const STORAGE_FAILED: &str = "The server stopped saying how much storage this account uses.";

#[derive(Default)]
struct Outbox {
    draining: bool,
    again: bool,
    diverged: Vec<(Uuid, Option<CommitId>)>,
    /// Blocks whose kept commits all reached the server, or were refused
    /// (`Replicas::drained`).
    published: Vec<(Uuid, bool)>,
}

impl Tasks {
    fn new() -> (Self, UnboundedReceiver<()>) {
        let (finished, receiver) = tokio::sync::mpsc::unbounded_channel();
        let contacts = Arc::new(tokio::sync::Mutex::new(()));
        let outbox = Default::default();
        (
            Self {
                finished,
                contacts,
                outbox,
                storage: Default::default(),
            },
            receiver,
        )
    }

    pub(super) fn sync(&self, peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) {
        if !peer.is_attached() {
            return;
        }
        {
            let mut outbox = self.outbox.borrow_mut();
            if outbox.draining {
                outbox.again = true;
                return;
            }
            if peer.pending().operations == 0 {
                return;
            }
            outbox.draining = true;
        }
        set_draining(shared, true);
        let (peer, shared, outbox) = (
            Arc::clone(peer),
            Arc::clone(shared),
            std::rc::Rc::clone(&self.outbox),
        );
        self.spawn(async move {
            loop {
                let (diverged, published) = drain_outbox(&peer, &shared).await;
                let mut held = outbox.borrow_mut();
                held.diverged.extend(diverged);
                held.published.extend(published);
                if !std::mem::take(&mut held.again) || !peer.is_attached() {
                    held.draining = false;
                    break;
                }
            }
            set_draining(&shared, false);
        });
    }

    fn syncing(&self) -> bool {
        self.outbox.borrow().draining
    }

    fn take_published(&self) -> Vec<(Uuid, bool)> {
        std::mem::take(&mut self.outbox.borrow_mut().published)
    }

    fn take_diverged(&self) -> Vec<(Uuid, Option<CommitId>)> {
        std::mem::take(&mut self.outbox.borrow_mut().diverged)
    }

    pub(super) fn spawn(&self, task: impl Future<Output = ()> + 'static) {
        let finished = self.finished.clone();
        platform::spawn_local(async move {
            task.await;
            let _ = finished.send(());
        });
    }

    fn refresh(&self, peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, block: Uuid) {
        let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
        self.spawn(async move { refresh(&peer, &shared, block).await });
    }

    /// Reads every notification mode the account set into `Shared::notify_modes`.
    fn notify_modes(&self, peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) {
        let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
        self.spawn(async move {
            if let Ok(read) = peer.notify_modes().await {
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                held.notify_modes = Some(read.iter().map(|row| (row.block, row.mode)).collect());
                held.notify_modes_revision += 1;
                drop(held);
                be_host_base::host::wake();
            }
        });
    }

    fn contacts(&self, peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) {
        let (peer, shared, tasks) = (Arc::clone(peer), Arc::clone(shared), self.clone());
        self.spawn(async move { tasks.sync_contacts(&peer, &shared).await });
    }

    /// Follows the account's storage on the server's `storage-changed` stream while an editor
    /// watches it, and stops following once none does. A stream that ends while the connection
    /// holds is a failure the host shows.
    fn storage(&self, peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) {
        let watched = shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .storage_watched;
        let mut following = self.storage.borrow_mut();
        if !watched {
            following.take();
            return;
        }
        if following.as_ref().is_some_and(|stop| !stop.is_closed()) || !peer.is_attached() {
            return;
        }
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        *following = Some(stop);
        let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
        self.spawn(async move {
            let mut usage = client::account::storage_changed(&peer.connection());
            let mut stopped = std::pin::pin!(stopped);
            loop {
                let next = match select(std::pin::pin!(usage.recv()), stopped.as_mut()).await {
                    Either::Left((next, _)) => next,
                    Either::Right(_) => return,
                };
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                match next {
                    Some(next) => {
                        if held.storage.as_ref() != Some(&next) {
                            held.storage = Some(next);
                            held.storage_revision += 1;
                        }
                    }
                    None => {
                        if peer.is_attached() {
                            held.error = Some(STORAGE_FAILED.to_owned());
                        }
                        drop(held);
                        be_host_base::host::wake();
                        return;
                    }
                }
                drop(held);
                be_host_base::host::wake();
            }
        });
    }

    async fn sync_contacts(&self, peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) {
        let _one = self.contacts.lock().await;
        sync_contacts(peer, shared).await;
    }
}

fn take_listings(shared: &Arc<Mutex<Shared>>) -> (Vec<Listing>, Vec<Listing>) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    (held.graph.take_unlisted(), held.graph.take_wanted())
}

async fn list(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    unlisted: Vec<Listing>,
    wanted: Vec<Listing>,
) {
    let unlists =
        futures_util::future::join_all(unlisted.into_iter().map(|listing| peer.unlist(listing)));
    let lists = futures_util::future::join_all(wanted.iter().map(|listing| async move {
        match *listing {
            Listing::Children(parent) => peer.children(parent).await,
            Listing::Deleted => peer.deleted().await,
            Listing::Shared => peer.shared().await,
            Listing::Backrefs(block) => peer.backrefs(block).await,
            Listing::References(block) => peer.referenced(block).await,
            Listing::Block(block) => peer.summary(block).await.map(|summary| vec![summary]),
            Listing::OpenSettings(block) => peer.open_settings(block).await,
        }
    }));
    let (_, answers) = futures_util::future::join(unlists, lists).await;
    for (listing, answer) in wanted.into_iter().zip(answers) {
        let summaries = match answer {
            Ok(summaries) => summaries,
            Err(ClientError::Refused(..)) => Vec::new(),
            Err(error) => {
                // A request the connection could not carry says nothing of the listing: it is
                // asked for again, by the connection that is up next. Failed is for the ones the
                // server answered, and only a relisting asks for those again.
                let unreachable = error.is_unreachable();
                record(shared, error);
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                match unreachable {
                    true => held.graph.unfetch(listing),
                    false => held.graph.failed(listing, listing_failure(listing).to_owned()),
                }
                continue;
            }
        };
        if let Err(error) = peer.learn(&summaries).await {
            record(shared, error);
        }
        let revision = summaries
            .iter()
            .map(|summary| summary.revision)
            .max()
            .unwrap_or_default();
        let nodes = summaries
            .iter()
            .filter(|summary| summary.access.can_know_exists())
            .map(|summary| node_of(peer, summary))
            .collect();
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .graph
            .listed(listing, nodes, revision);
    }
}

fn listing_failure(listing: Listing) -> &'static str {
    match listing {
        Listing::Children(BlockParent::Root) => "Couldn't load your blocks",
        Listing::Children(BlockParent::Detached) | Listing::Deleted => {
            "Couldn't load Recently Deleted"
        }
        Listing::Children(BlockParent::Block(_)) => "Couldn't load what's inside this block",
        Listing::Shared => "Couldn't load what's shared with you",
        Listing::Backrefs(_) => "Couldn't load what links here",
        Listing::References(_) => "Couldn't load what this block links to",
        Listing::Block(_) => "Couldn't load this block",
        Listing::OpenSettings(_) => "Couldn't load the settings above this block",
    }
}

fn want_open(shared: &Arc<Mutex<Shared>>, open: &HashMap<Uuid, Uuid>) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let mut queries: Vec<Query> = open.keys().map(|block| Query::Parents(*block)).collect();
    queries.extend(
        held.revision_previews
            .values()
            .filter(|preview| !preview.released)
            .map(|preview| Query::Parents(preview.source)),
    );
    queries.sort_by_key(|query| match query {
        Query::Parents(block) => *block,
        _ => Uuid::nil(),
    });
    queries.dedup();
    if held.graph.wanted(Owner::Open) != queries.as_slice() {
        held.graph.want(Owner::Open, queries);
    }
}

async fn settled(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    tasks: Option<&Tasks>,
    block: Uuid,
    reply: Result<BlockSummary, ClientError>,
) -> Result<(), ClientError> {
    if !shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .graph
        .settle(block)
    {
        return reply.map(|_| ());
    }
    match reply {
        Ok(summary) => {
            if let Err(error) = peer.learn(std::slice::from_ref(&summary)).await {
                record(shared, error);
            }
            let node = node_of(peer, &summary);
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .graph
                .put(node, summary.revision);
            Ok(())
        }
        Err(error) => {
            match tasks {
                Some(tasks) => tasks.refresh(peer, shared, block),
                None => refresh(peer, shared, block).await,
            }
            Err(error)
        }
    }
}

pub(super) async fn refresh(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, block: Uuid) {
    match peer.summary(block).await {
        Ok(summary) => {
            if let Err(error) = peer.learn(std::slice::from_ref(&summary)).await {
                record(shared, error);
            }
            let node = node_of(peer, &summary);
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .graph
                .put(node, summary.revision);
        }
        Err(ClientError::Refused(..)) => shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .graph
            .forget(block),
        Err(error) => record(shared, error),
    }
}

pub(super) fn node_of(peer: &Peer<Store>, summary: &BlockSummary) -> Node {
    let metadata = peer.metadata(summary);
    let locked = summary.access.can_view() && matches!(metadata, Err(ClientError::Locked(_)));
    Node {
        locked,
        id: summary.id,
        content_type: summary.content_type,
        author: summary.author,
        parent: summary.parent,
        access: summary.access,
        references: summary.references.clone(),
        metadata: metadata.unwrap_or_default(),
        head: summary.head,
        held: summary.held,
        sets_access: summary.sets_access,
        size: summary.size,
        tree_size: summary.tree_size,
    }
}

async fn connect(
    config: &Config,
    store: Store,
    refs: be_block_client::RefsDb,
) -> Result<(Peer<Store>, tokio::sync::OwnedMutexGuard<()>), ClientError> {
    let link = crate::link::signed_in(&config.server_url, &config.token, &config.clock).await?;
    let open = link.open_organization().await;
    let peer = Peer::signed_in(
        link.connection,
        link.account,
        config.token.clone(),
        peer_config(config, refs),
        store,
    )
    .await?;
    Ok((peer, open))
}

pub(super) fn record(shared: &Arc<Mutex<Shared>>, error: ClientError) {
    if error.is_unreachable() {
        return;
    }
    shared.lock().unwrap_or_else(PoisonError::into_inner).error = Some(error.to_string());
}

fn fail(shared: &Arc<Mutex<Shared>>, error: String) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    held.connected = false;
    held.reached = true;
    held.error = Some(error);
}
