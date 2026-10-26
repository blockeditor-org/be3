use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Condvar, PoisonError},
    time::{Duration, Instant},
};

use wasi_threads::Mutex;

use be_block::BlockMetadata;
use be_client::{ClientError, Credentials, Peer, PeerConfig};
use be_commit::CommitId;
use be_graph::{Access, BlockParent};
use be_protocol::{AccessList, BlockSummary, ClientMessage, ServerMessage};
use be_types_host::Conflict;
use block_type_api::{Change, Description, EditorId};
use block_type_guest::editor::FromEditor;
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

pub(super) fn reconnect_delay(failures: u32, jitter: f64) -> Duration {
    let ceiling = RECONNECT_FIRST
        .saturating_mul(1 << failures.min(16))
        .min(RECONNECT_MOST);
    ceiling.mul_f64(0.5 + 0.5 * jitter.clamp(0.0, 1.0))
}

fn jitter() -> f64 {
    (Uuid::new_v4().as_u128() as u64 >> 11) as f64 / (1u64 << 53) as f64
}

pub(super) struct Inbox {
    pub(super) commands: UnboundedReceiver<Command>,
    pub(super) nudged: super::nudge::Nudged,
}

type AccessReply = Box<dyn FnOnce(Result<AccessList, String>) + Send>;

type ExportReply = Box<dyn FnOnce(Result<(String, Vec<u8>), String>) + Send>;

type HeadReply = Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>;

type PlainReply = Box<dyn FnOnce(Result<be_types_host::PlainFile, String>) + Send>;

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
    #[cfg(test)]
    Close(Uuid),
    Operate(Uuid, Vec<u8>, Instant),
    ChangeChild {
        block: Uuid,
        content_type: Uuid,
        change: block_type_api::ChildChange,
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
    Import(Vec<(be_client::transfer::Saved, BlockParent)>),
    Version {
        block: Uuid,
        command: block_plugin_api::VersionCommand,
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
    Invite {
        email: String,
        role: be_protocol::OrganizationRole,
        reply: LinkReply,
    },
    CreateAgent {
        name: String,
        reply: AgentReply,
    },
    Sync,
}

impl Command {
    fn is_quick(&self) -> bool {
        matches!(
            self,
            Command::Operate(..)
                | Command::FromEditor { .. }
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
            | Command::Detach { block, .. }
            | Command::FromEditor { block, .. }
            | Command::Create { block, .. }
            | Command::SetParent { block, .. }
            | Command::Define { block, .. }
            | Command::SetAccess { block, .. }
            | Command::ListAccess { block, .. }
            | Command::ReadHead { block, .. }
            | Command::PlainFile { block, .. }
            | Command::Version { block, .. }
            | Command::WatchVersion(block)
            | Command::Leave(block)
            | Command::Contact(ContactCommand::Link {
                offer: Some((block, _)),
                ..
            }) => Some(*block),
            Command::Duplicate { to, .. } => Some(*to),
            #[cfg(test)]
            Command::Close(block)
            | Command::Presence { block, .. }
            | Command::SendToPeers { block, .. } => Some(*block),
            Command::Export { .. }
            | Command::Unsent { .. }
            | Command::Import(_)
            | Command::Flush(_)
            | Command::ApprovePairing { .. }
            | Command::DismissPairing(_)
            | Command::Contact(_)
            | Command::Members(_)
            | Command::Invite { .. }
            | Command::CreateAgent { .. }
            | Command::Sync => None,
        }
    }

    fn rekeys(&self) -> Option<Uuid> {
        match self {
            Command::SetParent { block, .. }
            | Command::SetAccess { block, .. }
            | Command::Leave(block)
            | Command::Contact(ContactCommand::Link {
                offer: Some((block, _)),
                ..
            }) => Some(*block),
            _ => None,
        }
    }
}

type MembersReply = Box<dyn FnOnce(Result<Vec<be_protocol::Member>, String>) + Send>;

pub(super) enum MembersCommand {
    List(MembersReply),
    SetRole {
        account: Uuid,
        role: be_protocol::OrganizationRole,
        reply: MembersReply,
    },
    Remove {
        account: Uuid,
        reply: MembersReply,
    },
}

type LinkReply = Box<dyn FnOnce(Result<String, String>) + Send>;

type AgentReply = Box<dyn FnOnce(Result<crate::accounts::AgentCredentials, String>) + Send>;

type CardReply = Box<dyn FnOnce(Result<be_keys::ContactCard, String>) + Send>;

pub(super) enum ContactCommand {
    Link {
        offer: Option<(Uuid, Access)>,
        reply: LinkReply,
    },
    Open {
        code: be_keys::InviteCode,
        reply: CardReply,
    },
    Accept {
        code: be_keys::InviteCode,
        reply: CardReply,
    },
    Remove(Uuid),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PairingRequest {
    pub(crate) from: u64,
    pub(crate) device: String,
    message: Vec<u8>,
}

const PAIRING_REQUEST_LIMIT: usize = 4;

pub(super) const PEER_MESSAGE_LIMIT: usize = 4096;

#[derive(Default)]
pub(crate) struct Shared {
    pub(crate) revision_previews: HashMap<Uuid, super::revisions::Preview>,
    pub(crate) preview_aliases: HashMap<(Uuid, Uuid), Uuid>,
    pub(crate) released_previews: Vec<Uuid>,
    pub(crate) graph: Graph,
    pub(crate) blocks: HashMap<Uuid, Content>,
    pub(crate) described: HashMap<Uuid, Description>,
    pub(crate) editor_frames: HashMap<EditorId, Vec<(Uuid, Vec<u8>)>>,
    pub(crate) histories: HashMap<Uuid, History>,
    pub(crate) presence: HashMap<Uuid, (u64, Vec<Presence>)>,
    pub(crate) presence_revision: u64,
    pub(crate) peer_messages: HashMap<Uuid, std::collections::VecDeque<(u64, PeerMessage)>>,
    pub(crate) peer_message_count: u64,
    pub(crate) wakes: u64,
    pub(crate) unsealed: usize,
    pub(crate) saves: u64,
    pub(crate) owing: usize,
    pub(crate) connected: bool,
    pub(crate) reached: bool,
    pub(crate) draining: bool,
    pub(crate) error: Option<String>,
    pub(crate) module_error: Option<(Uuid, String)>,
    pub(crate) conflicts: HashMap<Uuid, Vec<Conflict>>,
    pub(crate) pairing: Vec<PairingRequest>,
    pub(crate) contacts: Vec<be_client::Contact>,
    pub(crate) contacts_revision: u64,
    pub(crate) escrow_warning: Option<&'static str>,
    pub(crate) locked: std::collections::HashSet<Uuid>,
    pub(crate) version_watch: std::collections::HashSet<Uuid>,
    pub(crate) versions: HashMap<Uuid, super::version::VersionState>,
    pub(crate) touched: std::collections::HashSet<Uuid>,
    pub(crate) pending: usize,
    pub(crate) typing: bool,
    pub(crate) uploading: usize,
    pub(crate) importing: usize,
    pub(crate) previewed: std::collections::HashSet<Uuid>,
    pub(crate) staged: HashMap<Uuid, (Uuid, Vec<u8>)>,
    pub(crate) owned: std::collections::HashSet<Uuid>,
    pub(crate) unreadable: HashMap<Uuid, String>,
    pub(crate) stale_revision: u64,
    pub(crate) waiting: usize,
    pub(crate) unsent: Vec<be_client::UnsentBlock>,
    pub(crate) comparison: Option<super::conflicts::Comparison>,
    pub(crate) wall: Wall,
}

#[derive(Clone, Default)]
pub(crate) struct Wall {
    clock: be_client::Clock,
    anchor: Option<(i64, std::time::Duration)>,
}

impl Wall {
    pub(crate) fn new(clock: be_client::Clock) -> Self {
        let anchor = Some((be_commit::now_milliseconds(), clock.now()));
        Self { clock, anchor }
    }

    pub(crate) fn now(&self) -> i64 {
        let Some((wall, at)) = self.anchor else {
            return be_commit::now_milliseconds();
        };
        let since = self.clock.now().saturating_sub(at);
        wall.saturating_add(i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
    }
}

impl Shared {
    pub(crate) fn touch(&mut self, block: Uuid) {
        self.touched.insert(block);
    }
}

pub(super) type Store = be_store::FileStore;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PeerMessage {
    pub(crate) from: u64,
    pub(crate) channel: Uuid,
    pub(crate) payload: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Presence {
    pub(crate) client: u64,
    pub(crate) kind: Uuid,
    pub(crate) value: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct History {
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
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
    replicas.modules.ready().await;
    crate::host::wake();
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
            listed: tokio::sync::mpsc::unbounded_channel(),
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

fn forget_what_is_not_kept(peer: &Peer<Store>) {
    let _ = peer.forget_offline(|workspace| crate::offline_choice::choice(workspace).keep);
}

fn peer_config(config: &Config, refs: be_client::RefsDb) -> PeerConfig {
    PeerConfig::new(
        config.socket_url(),
        be_keys::RecoverySecret::from_bytes(config.identity),
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
        changed.notify_all();
        crate::host::wake();
        let delay = reconnect_delay(failures, jitter());
        failures = failures.saturating_add(1);
        let waited = config.clock.sleep(delay);
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
                    .to_real(scope, reference, block_plugin_api::BlockIdRole::Existing)
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
        events: tokio::sync::broadcast::Receiver<ServerMessage>,
        gone: tokio::sync::watch::Receiver<bool>,
        _open: Opened,
    },
    Down {
        failures: u32,
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

    fn down(config: &Config, failures: u32, delay: Duration) -> Self {
        let clock = config.clock.clone();
        let url = config.server_url.clone();
        let token = config.token.clone();
        Self::Down {
            failures,
            reaching: Box::pin(async move {
                clock.sleep(delay).await;
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
    listed: (
        tokio::sync::mpsc::UnboundedSender<()>,
        UnboundedReceiver<()>,
    ),
}

impl Session<'_> {
    async fn run(&mut self, open: Option<Opened>) {
        let (peer, shared) = (self.peer, self.shared);
        self.uploads.connect(peer);
        {
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            let my_files = be_block::my_files(peer.organization(), peer.account());
            held.graph
                .want(Owner::Host, vec![Query::Roots, Query::Block(my_files)]);
        }
        match open {
            Some(open) => self.arrive(open).await,
            None => {
                self.depart(0);
                want_open(shared, &self.wanted.blocks());
                relist(shared);
                fetch_listings(peer, shared).await;
                self.versions
                    .refresh(&mut Context {
                        peer,
                        shared,
                        replicas: &mut *self.replicas,
                    })
                    .await;
                report(peer, shared, false);
                self.changed.notify_all();
                crate::host::wake();
            }
        }
        let mut flushes = Vec::new();
        let mut keyed: HashMap<Uuid, Uuid> = HashMap::new();
        let mut stopping = false;
        loop {
            if stopping && (self.uploads.is_empty() || !self.line.is_up()) {
                self.replicas.seal_all((peer, shared)).await;
                report(peer, shared, false);
                return;
            }
            let wake_at = self.replicas.modules.wake_at();
            let woken = {
                let waited = std::pin::pin!(wait(
                    self.commands,
                    &mut self.line,
                    self.nudged,
                    &mut self.uploads,
                    &mut *self.replicas,
                    &mut self.listed.1,
                    stopping
                ));
                match wake_at {
                    Some(at) => {
                        let timer = self.config.clock.sleep_until(at);
                        match select(waited, timer).await {
                            Either::Left((woken, _)) => woken,
                            Either::Right(_) => Woken::Deadline,
                        }
                    }
                    None => waited.await,
                }
            };
            let handled = self.replicas.handled;
            let pumped = matches!(woken, Woken::Module) && self.replicas.pump((peer, shared)).await;
            if !matches!(woken, Woken::Module | Woken::Listed) || self.replicas.handled != handled {
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
                    }
                    false
                }
                Woken::Disconnected => {
                    self.depart(0);
                    true
                }
                Woken::Reached(Ok(link)) => {
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
                    };
                    let mut applied = false;
                    for command in self.uploads.runnable() {
                        applied |= ran.run(command).await;
                    }
                    crate::host::wake();
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
                        match &event {
                            Some(message) => self.replicas.heard((peer, shared), message).await,
                            None => self.replicas.resynchronize((peer, shared)).await,
                        }
                        heard(peer, shared, event).await;
                    }
                    false
                }
                Woken::Module | Woken::Deadline => {
                    self.replicas.modules.tick_due();
                    false
                }
                Woken::Listed => true,
            };
            moved |= pumped;
            moved |= self.replicas.pump((peer, shared)).await;
            rekey_changed(peer, self.replicas, &mut keyed, shared).await;
            let before = version_revisions(shared);
            self.versions
                .refresh(&mut Context {
                    peer,
                    shared,
                    replicas: &mut *self.replicas,
                })
                .await;
            moved |= version_revisions(shared) != before;
            want_open(shared, &self.wanted.blocks());
            spawn_listings(peer, shared, &self.listed.0);
            if self.line.quiet()
                && (self.uploads.is_empty() || !self.line.is_up())
                && !flushes.is_empty()
            {
                self.replicas.seal_all((peer, shared)).await;
                for done in flushes.drain(..) {
                    let _ = done.send(());
                }
            }
            moved |= report(peer, shared, false);
            let typing = !self.replicas.modules.settled();
            let was_typing = std::mem::replace(
                &mut shared.lock().unwrap_or_else(PoisonError::into_inner).typing,
                typing,
            );
            self.changed.notify_all();
            if moved || was_typing != typing {
                crate::host::wake();
            }
        }
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
            0 if was_up => Duration::ZERO,
            0 => Duration::ZERO,
            failures => reconnect_delay(failures - 1, jitter()),
        };
        self.line = Line::down(self.config, failures, delay);
        if was_up {
            self.replicas.disconnected();
        }
        shared_offline(self.shared);
        self.changed.notify_all();
        crate::host::wake();
    }

    async fn arrive(&mut self, open: Opened) {
        let (peer, shared) = (self.peer, self.shared);
        self.line = Line::up(peer, open);
        {
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            held.connected = true;
            held.reached = true;
            held.error = None;
        }
        self.changed.notify_all();
        crate::host::wake();
        sync_outbox(peer, self.replicas, shared).await;
        want_open(shared, &self.wanted.blocks());
        relist(shared);
        fetch_listings(peer, shared).await;
        sync_contacts(peer, shared).await;
        self.replicas.reconnect((peer, shared)).await;
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
        self.versions
            .refresh(&mut Context {
                peer,
                shared,
                replicas: &mut *self.replicas,
            })
            .await;
        report(peer, shared, false);
        self.changed.notify_all();
        crate::host::wake();
    }
}

fn shared_offline(shared: &Arc<Mutex<Shared>>) {
    shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .connected = false;
}

async fn sync_outbox(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    shared: &Arc<Mutex<Shared>>,
) {
    if peer.pending().operations == 0 {
        return;
    }
    report(peer, shared, true);
    crate::host::wake();
    match peer.drain(|_| false).await {
        Ok(drained) => {
            for outcome in drained {
                match outcome {
                    be_client::Drained::Diverged {
                        block,
                        local,
                        remote,
                    } => {
                        if let Err(error) = replicas
                            .merge_diverged((peer, shared), block, local, remote)
                            .await
                        {
                            record(shared, error);
                        }
                    }
                    be_client::Drained::Refused { block, code } => record(
                        shared,
                        ClientError::Refused(
                            code,
                            format!("a change made offline to block {block} was refused"),
                        ),
                    ),
                    _ => {}
                }
            }
        }
        Err(error) => record(shared, error),
    }
    report(peer, shared, false);
}

pub(super) fn expired_unsent(
    unsent: &[be_client::UnsentBlock],
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
    let company = peer.kind() == be_protocol::OrganizationKind::Company;
    let choice = crate::offline_choice::choice(Uuid::nil());
    for block in expired_unsent(&peer.unsent(), company, &choice, now) {
        let _ = peer.discard_unsent(block);
        let _ = peer.forget_cached(block);
    }
}

fn report(peer: &Peer<Store>, shared: &Arc<Mutex<Shared>>, syncing: bool) -> bool {
    let now = shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .wall
        .now();
    expire_unsent(peer, now);
    let counts = peer.pending();
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let mut changed = (held.waiting, held.draining) != (counts.operations, syncing);
    (held.waiting, held.draining) = (counts.operations, syncing);
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
    let reader = cfg!(test) || wanted.readers.contains(&block);
    replicas
        .open((peer, shared), block, content_type, reader)
        .await;
    if let Some((_, editors)) = wanted.editing.get(&block) {
        for (editor, editable) in editors {
            replicas.attach(block, *editor, *editable);
        }
    }
}

async fn rekey_changed(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    keyed: &mut HashMap<Uuid, Uuid>,
    shared: &Arc<Mutex<Shared>>,
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
            replicas.seal((peer, shared), block, true).await;
        }
    }
}

enum Woken {
    Command(Command),
    Listed,
    Uploaded,
    Event(Option<ServerMessage>),
    Module,
    Deadline,
    Disconnected,
    Reached(Result<crate::link::Link, ClientError>),
    Nudged,
    Stopped,
}

async fn wait(
    commands: &mut UnboundedReceiver<Command>,
    line: &mut Line,
    nudged: &mut super::nudge::Nudged,
    uploads: &mut Uploads,
    replicas: &mut Replicas,
    listed: &mut UnboundedReceiver<()>,
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
        match listed.recv().await {
            Some(()) => Woken::Listed,
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
}

const NEEDS_CONNECTION: &str =
    "This needs a connection to the server. Try again once Block is back online.";

fn refuse_offline(command: Command, shared: &Arc<Mutex<Shared>>) -> Option<Command> {
    match command {
        Command::SetAccess { .. } | Command::Leave(_) | Command::ApprovePairing { .. } => {
            shared.lock().unwrap_or_else(PoisonError::into_inner).error =
                Some(NEEDS_CONNECTION.to_owned());
        }
        Command::ListAccess { reply, .. } => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::Members(
            MembersCommand::List(reply)
            | MembersCommand::SetRole { reply, .. }
            | MembersCommand::Remove { reply, .. },
        ) => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::Invite { reply, .. } => reply(Err(NEEDS_CONNECTION.to_owned())),
        Command::CreateAgent { reply, .. } => reply(Err(NEEDS_CONNECTION.to_owned())),
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
    crate::host::wake();
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
                contact(self.peer, self.shared, self.config, command).await;
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
                crate::host::wake();
                false
            }
            Command::CreateAgent { name, reply } => {
                let created = create_agent(self.peer, self.config, name).await;
                if created.is_ok() {
                    sync_contacts(self.peer, self.shared).await;
                }
                reply(created.map_err(|error| error.to_string()));
                crate::host::wake();
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
            (held.unsealed, held.owing) = self.replicas.counts();
            held.typing = !self.replicas.modules.settled();
        }
        crate::host::wake();
    }
}

fn asked_to_pair(shared: &Arc<Mutex<Shared>>, from: u64, payload: &[u8]) {
    let Ok(be_keys::PairingMessage::Request { message, device }) =
        be_keys::PairingMessage::decode(payload)
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
    crate::host::wake();
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
    let Some(code) = be_keys::normalize_code(code) else {
        return Err(ClientError::Encoding(
            "that is not a pairing code".to_owned(),
        ));
    };
    let (pairing, message) = be_keys::Pairing::start(&code, peer.account().as_bytes());
    let key = pairing
        .finish(&request.message)
        .map_err(|error| ClientError::Encoding(error.to_string()))?;
    let sealed = key.seal(&peer.keys().identity().to_bytes());
    let payload = be_keys::PairingMessage::Reply { message, sealed }.encode();
    peer.connection()
        .request(|request| ClientMessage::Pair {
            request,
            to: Some(from),
            payload,
        })
        .await?;
    Ok(())
}

async fn sync_contacts(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) {
    let identity = peer.keys().identity();
    match be_client::contacts::finish_links(&peer.connection(), identity, peer.organization()).await
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
                fetch_listings(peer, shared).await;
            }
        }
        Err(error) => record(shared, error),
    }
    tend(peer, shared).await;
    crate::host::wake();
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
        .map(be_client::EscrowWarning::message);
    crate::host::wake();
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
    crate::host::wake();
}

async fn invite(
    peer: &Peer<Store>,
    config: &Config,
    email: String,
    role: be_protocol::OrganizationRole,
) -> Result<String, ClientError> {
    let identity = peer.keys().identity();
    let me = be_client::contacts::card(
        peer.account(),
        identity,
        &config.display_name,
        &config.email,
    );
    let code = be_client::contacts::invite(
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

async fn create_agent(
    peer: &Peer<Store>,
    config: &Config,
    name: String,
) -> Result<crate::accounts::AgentCredentials, ClientError> {
    let identity = peer.keys().identity();
    let me = be_client::contacts::card(
        peer.account(),
        identity,
        &config.display_name,
        &config.email,
    );
    let organization = peer.organization();
    let phrase = be_keys::RecoveryPhrase::generate();
    let secret = phrase.secret();
    let public = secret.public();
    let ServerMessage::AgentCreated { agent, token, .. } = peer
        .connection()
        .request(|request| ClientMessage::CreateAgent {
            request,
            organization,
            name,
            recovery: public.0,
            contact: me.seal_own(&secret),
            escrow: peer.keys().escrow().map_or_else(Vec::new, |escrow| {
                be_client::contacts::seal_pin(&secret, organization, &escrow)
            }),
        })
        .await?
    else {
        return Err(ClientError::Unexpected);
    };
    let card = be_keys::ContactCard {
        account: agent.account.into_bytes(),
        public,
        display_name: agent.name.clone(),
        email: agent.email.clone(),
    };
    peer.connection()
        .request(|request| ClientMessage::PutContact {
            request,
            contact: agent.account,
            sealed: card.seal_own(identity),
        })
        .await?;
    Ok(crate::accounts::AgentCredentials {
        name: agent.name,
        server: config.server_url.clone(),
        organization,
        token,
        phrase: phrase.to_string(),
    })
}

async fn contact(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    config: &Config,
    command: ContactCommand,
) {
    let identity = peer.keys().identity();
    let me = be_client::contacts::card(
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
                be_client::contacts::make_link(&peer.connection(), identity, &me, offer.as_ref())
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
            let opened = be_client::contacts::open_link(&peer.connection(), &code)
                .await
                .map(|(_, card)| card);
            reply(opened.map_err(|error| error.to_string()));
        }
        ContactCommand::Accept { code, reply } => {
            let accepted =
                be_client::contacts::accept_link(&peer.connection(), identity, &me, &code).await;
            if accepted.is_ok() {
                sync_contacts(peer, shared).await;
            }
            reply(accepted.map_err(|error| error.to_string()));
        }
        ContactCommand::Remove(account) => {
            match be_client::contacts::remove_contact(&peer.connection(), account).await {
                Ok(()) => sync_contacts(peer, shared).await,
                Err(error) => record(shared, error),
            }
        }
    }
    crate::host::wake();
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
        crate::host::wake();
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
        crate::host::wake();
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
    command: Command,
) -> bool {
    let ctx = (peer, shared);
    match command {
        Command::Contact(_)
        | Command::Members(_)
        | Command::Invite { .. }
        | Command::CreateAgent { .. }
        | Command::Sync => false,
        Command::Leave(block) => {
            if let Err(error) = peer.leave(block).await {
                record(shared, error);
            }
            refresh(peer, shared, block).await;
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
            crate::host::wake();
            false
        }
        Command::DismissPairing(from) => {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pairing
                .retain(|request| request.from != from);
            crate::host::wake();
            false
        }
        Command::Version { block, command } => {
            set_busy(shared, block, true, None);
            crate::host::wake();
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
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            if let Err(error) = done {
                held.error = Some(error.to_string());
            }
            held.unsent = peer.unsent();
            held.stale_revision += 1;
            drop(held);
            crate::host::wake();
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
        #[cfg(test)]
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
            let created = peer
                .create_block(to, content_type, BlockParent::Detached, &metadata)
                .await;
            if let Err(error) = settled(peer, shared, to, created).await {
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
            let bytes =
                bytes.or_else(|| super::types::kind_of(content_type).map(|kind| kind.blank));
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
        } => {
            let kind = super::types::kind_of(content_type);
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
                .create_block(block, content_type, parent, &metadata)
                .await;
            if let Err(error) = settled(peer, shared, block, created).await {
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
                refresh(peer, shared, block).await;
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
            if let Err(error) = settled(peer, shared, block, defined).await {
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
                _ if account == be_graph::ANYONE_INSIDE => peer.open_inside(block, access).await,
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
            crate::host::wake();
            false
        }
        Command::ReadHead { commit, reply, .. } => {
            let read = async {
                let commit = peer.load_commit(commit).await?;
                peer.read_manifest(&commit.manifest).await
            }
            .await;
            reply(read.map_err(|error| error.to_string()));
            crate::host::wake();
            false
        }
        Command::Export { block, reply } => {
            let exported = export(peer, replicas, shared, block).await;
            reply(exported.map_err(|error| error.to_string()));
            crate::host::wake();
            false
        }
        Command::PlainFile { block, reply } => {
            reply(plain_file(peer, replicas, shared, block).await);
            crate::host::wake();
            false
        }
        Command::Import(_) => false,
        Command::Flush(done) => {
            replicas.seal_all(ctx).await;
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
) -> Result<be_types_host::PlainFile, String> {
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
        None => super::kind_of(content_type)
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
            .publish_encoded(
                block,
                content_type,
                &bytes,
                description.references.clone(),
                description.derived.clone(),
                expected,
                None,
            )
            .await?
        {
            be_client::Saved::Rejected { head } if retry => expected = head,
            _ => return Ok(()),
        }
    }
    Ok(())
}

async fn imported(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    imported: be_client::peer::Imported,
) {
    if let Err(error) = settled(peer, shared, block, imported).await {
        record(shared, error);
    }
    remember_head(peer, shared, block);
}

async fn heard(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, event: Option<ServerMessage>) {
    match event {
        Some(ServerMessage::BlockChanged { block }) => {
            if let Err(error) = peer.learn(std::slice::from_ref(&block)).await {
                record(shared, error);
            }
            let node = node_of(peer, &block);
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
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
        Some(ServerMessage::BlockRemoved { block, revision }) => {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .graph
                .remove(block, revision);
        }
        Some(ServerMessage::HeadChanged { block, head, .. }) => {
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .graph
                .set_head(block, Some(head));
        }
        Some(ServerMessage::Paired { from, payload }) => {
            asked_to_pair(shared, from, &payload);
        }
        Some(ServerMessage::ContactsChanged) => sync_contacts(peer, shared).await,
        Some(ServerMessage::KeyWorkChanged) => tend(peer, shared).await,
        Some(_) => {}
        None => {
            relist(shared);
            fetch_listings(peer, shared).await;
        }
    }
}

async fn export(
    peer: &Arc<Peer<Store>>,
    replicas: &mut Replicas,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
) -> Result<(String, Vec<u8>), ClientError> {
    let picked = peer.summary(block).await?;
    let mut top = picked.clone();
    if let Some(BlockParent::Block(parent)) = picked.parent
        && let Ok(whole) = peer.summary(parent).await
        && crate::editors::plugin::discovery::exported_whole(whole.content_type)
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
                if child.access.can_view() && found.iter().all(|(held, _)| held.id != child.id) {
                    next.push((child.id, found.len()));
                    found.push((child, Some(position)));
                }
            }
        }
        level = next;
    }
    let summaries: Vec<BlockSummary> = found.iter().map(|(summary, _)| summary.clone()).collect();
    peer.learn(&summaries).await?;
    let mut blocks: Vec<be_client::transfer::Saved> = found
        .into_iter()
        .map(|(summary, parent)| be_client::transfer::Saved {
            id: summary.id,
            block_type: summary.content_type,
            parent,
            metadata: node_of(peer, &summary).metadata,
            references: summary.references,
            content: None,
        })
        .collect();
    let heads: HashMap<Uuid, Option<be_commit::CommitId>> = summaries
        .iter()
        .map(|summary| (summary.id, summary.head))
        .collect();
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
            false => read.next().and_then(|(_, content)| content),
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
        be_client::transfer::file_name(&name),
        be_client::transfer::encode(&blocks),
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

fn spawn_listings(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    listed: &tokio::sync::mpsc::UnboundedSender<()>,
) {
    let (unlisted, wanted) = take_listings(shared);
    if unlisted.is_empty() && wanted.is_empty() {
        return;
    }
    let (peer, shared, listed) = (Arc::clone(peer), Arc::clone(shared), listed.clone());
    platform::spawn_local(async move {
        list(&peer, &shared, unlisted, wanted).await;
        let _ = listed.send(());
    });
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
                record(shared, error);
                shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .graph
                    .failed(listing, listing_failure(listing).to_owned());
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
            refresh(peer, shared, block).await;
            Err(error)
        }
    }
}

async fn refresh(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, block: Uuid) {
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
    }
}

async fn connect(
    config: &Config,
    store: Store,
    refs: be_client::RefsDb,
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
