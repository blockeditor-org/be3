use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Condvar, Mutex, PoisonError},
    time::{Duration, Instant},
};

use be_block::BlockMetadata;
use be_client::{ClientError, Credentials, Peer, PeerConfig};
use be_commit::CommitId;
use be_graph::{Access, BlockParent};
use be_protocol::{AccessEntry, BlockSummary, ClientMessage, ServerMessage};
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

const RECONNECT_DELAY: Duration = Duration::from_secs(2);

type AccessReply = Box<dyn FnOnce(Result<Vec<AccessEntry>, String>) + Send>;

type ExportReply = Box<dyn FnOnce(Result<(String, Vec<u8>), String>) + Send>;

type HeadReply = Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>;

type PlainReply = Box<dyn FnOnce(Result<be_types_host::PlainFile, String>) + Send>;

pub(super) enum Command {
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
    pub(crate) error: Option<String>,
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
    pub(crate) previewed: std::collections::HashSet<Uuid>,
    pub(crate) staged: HashMap<Uuid, (Uuid, Vec<u8>)>,
    pub(crate) owned: std::collections::HashSet<Uuid>,
    pub(crate) unreadable: HashMap<Uuid, String>,
    pub(crate) stale_revision: u64,
}

impl Shared {
    pub(crate) fn touch(&mut self, block: Uuid) {
        self.touched.insert(block);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) type Store = be_store::FileStore;
#[cfg(target_arch = "wasm32")]
pub(super) type Store = be_store::MemoryStore;

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
    mut commands: UnboundedReceiver<Command>,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
) {
    let mut replicas = Replicas::new(Modules::new(types::sources().await));
    replicas.modules.ready().await;
    crate::host::wake();
    let mut wanted = Wanted::default();
    let mut uploads = Uploads::new(&shared);
    loop {
        if let Outcome::Stopped = connected(
            &config,
            &make_store,
            &mut commands,
            &shared,
            &changed,
            &mut wanted,
            &mut replicas,
            &mut uploads,
        )
        .await
        {
            changed.notify_all();
            return;
        }
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .connected = false;
        changed.notify_all();
        crate::host::wake();
        platform::sleep(RECONNECT_DELAY).await;
    }
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

enum Outcome {
    Stopped,
    Lost,
}

#[allow(clippy::too_many_arguments)]
async fn connected<S: Fn() -> Result<Store, String>>(
    config: &Config,
    make_store: &S,
    commands: &mut UnboundedReceiver<Command>,
    shared: &Arc<Mutex<Shared>>,
    changed: &Arc<Condvar>,
    wanted: &mut Wanted,
    replicas: &mut Replicas,
    uploads: &mut Uploads,
) -> Outcome {
    let store = match make_store() {
        Ok(store) => store,
        Err(error) => {
            fail(shared, error);
            return Outcome::Lost;
        }
    };
    let (peer, _open) = match connect(config, store).await {
        Ok((peer, open)) => {
            let resolving = Arc::clone(shared);
            peer.set_resolver(Arc::new(move |block, references| {
                let held = resolving.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(scope) = held.graph.scope_of(block) else {
                    return references;
                };
                references
                    .into_iter()
                    .map(|reference| {
                        held.graph.to_real(
                            scope,
                            reference,
                            block_plugin_api::BlockIdRole::Existing,
                        )
                    })
                    .collect()
            }));
            (Arc::new(peer), open)
        }
        Err(error) => {
            fail(shared, error.to_string());
            return Outcome::Lost;
        }
    };
    {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.connected = true;
        held.error = None;
    }
    changed.notify_all();
    crate::host::wake();
    let mut events = peer.connection().subscribe();
    let mut gone = peer.connection().closed();
    {
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.graph.want(Owner::Host, vec![Query::Roots]);
    }
    want_open(shared, &wanted.blocks());
    relist(shared);
    fetch_listings(&peer, shared).await;
    sync_contacts(&peer, shared).await;
    replicas.reconnect((&peer, shared)).await;
    for (block, content_type) in wanted.blocks() {
        open_wanted(&peer, shared, wanted, replicas, block, content_type).await;
    }
    replicas.pump((&peer, shared)).await;
    let mut versions = Versions::default();
    versions
        .refresh(&mut Context {
            peer: &peer,
            shared,
            replicas: &mut *replicas,
        })
        .await;
    changed.notify_all();
    crate::host::wake();
    let mut flushes = Vec::new();
    let mut keyed: HashMap<Uuid, Uuid> = HashMap::new();
    let mut stopping = false;
    uploads.connect(&peer);
    loop {
        if stopping && uploads.is_empty() {
            replicas.seal_all((&peer, shared)).await;
            return Outcome::Stopped;
        }
        let deadline = replicas.modules.deadline();
        let woken = {
            let waited = std::pin::pin!(wait(
                commands,
                &mut events,
                &mut gone,
                uploads,
                &mut *replicas,
                stopping
            ));
            match deadline {
                Some(remaining) => {
                    let timer = std::pin::pin!(platform::sleep(remaining));
                    match select(waited, timer).await {
                        Either::Left((woken, _)) => woken,
                        Either::Right(_) => Woken::Deadline,
                    }
                }
                None => waited.await,
            }
        };
        let handled = replicas.handled;
        let pumped = matches!(woken, Woken::Module) && replicas.pump((&peer, shared)).await;
        if !matches!(woken, Woken::Module) || replicas.handled != handled {
            shared.lock().unwrap_or_else(PoisonError::into_inner).wakes += 1;
        }
        let mut ran = Ran {
            peer: &peer,
            replicas: &mut *replicas,
            wanted: &mut *wanted,
            shared,
            config,
            versions: &mut versions,
            flushes: &mut flushes,
        };
        let mut moved = match woken {
            Woken::Stopped => {
                stopping = true;
                false
            }
            Woken::Disconnected if stopping => return Outcome::Stopped,
            Woken::Disconnected => return Outcome::Lost,
            Woken::Command(command) => {
                let mut next = Some(command);
                let mut applied = false;
                while let Some(command) = next.take() {
                    let quick = command.is_quick();
                    match command {
                        Command::Import(blocks) => {
                            uploads.add(blocks);
                            ran.done();
                        }
                        command => {
                            if let Some(command) = uploads.defer(command) {
                                applied |= ran.run(command).await;
                            }
                        }
                    }
                    if quick {
                        next = commands.try_recv().ok();
                    }
                }
                applied
            }
            Woken::Uploaded => {
                let mut applied = false;
                for command in uploads.runnable() {
                    applied |= ran.run(command).await;
                }
                crate::host::wake();
                applied
            }
            Woken::Event(event) => {
                let mut heard_all = vec![event];
                loop {
                    match events.try_recv() {
                        Ok(event) => heard_all.push(Some(event)),
                        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                            heard_all.push(None);
                        }
                        Err(_) => break,
                    }
                }
                for event in heard_all {
                    match &event {
                        Some(message) => replicas.heard((&peer, shared), message).await,
                        None => replicas.resynchronize((&peer, shared)).await,
                    }
                    heard(&peer, shared, event).await;
                }
                false
            }
            Woken::Module | Woken::Deadline => {
                replicas.modules.tick_due();
                false
            }
        };
        moved |= pumped;
        moved |= replicas.pump((&peer, shared)).await;
        rekey_changed(&peer, replicas, &mut keyed, shared).await;
        let before = version_revisions(shared);
        versions
            .refresh(&mut Context {
                peer: &peer,
                shared,
                replicas: &mut *replicas,
            })
            .await;
        moved |= version_revisions(shared) != before;
        want_open(shared, &wanted.blocks());
        moved |= fetch_listings(&peer, shared).await;
        if events.is_empty() && uploads.is_empty() && !flushes.is_empty() {
            replicas.seal_all((&peer, shared)).await;
            for done in flushes.drain(..) {
                let _ = done.send(());
            }
        }
        let typing = !replicas.modules.settled();
        let was_typing = std::mem::replace(
            &mut shared.lock().unwrap_or_else(PoisonError::into_inner).typing,
            typing,
        );
        changed.notify_all();
        if moved || was_typing != typing {
            crate::host::wake();
        }
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
    Uploaded,
    Event(Option<ServerMessage>),
    Module,
    Deadline,
    Disconnected,
    Stopped,
}

async fn wait(
    commands: &mut UnboundedReceiver<Command>,
    events: &mut tokio::sync::broadcast::Receiver<be_protocol::ServerMessage>,
    gone: &mut tokio::sync::watch::Receiver<bool>,
    uploads: &mut Uploads,
    replicas: &mut Replicas,
    stopping: bool,
) -> Woken {
    let command = std::pin::pin!(async {
        match stopping {
            true => std::future::pending().await,
            false => commands.recv().await,
        }
    });
    let event = std::pin::pin!(events.recv());
    let closed = std::pin::pin!(gone.changed());
    let uploaded = std::pin::pin!(uploads.next());
    let module = std::pin::pin!(replicas.woken());
    match select(
        select(select(select(command, event), closed), uploaded),
        module,
    )
    .await
    {
        Either::Right(_) => Woken::Module,
        Either::Left((Either::Right(_), _)) => Woken::Uploaded,
        Either::Left((Either::Left((woken, _)), _)) => match woken {
            Either::Left((Either::Left((Some(command), _)), _)) => Woken::Command(command),
            Either::Left((Either::Left((None, _)), _)) => Woken::Stopped,
            Either::Left((Either::Right((Ok(event), _)), _)) => Woken::Event(Some(event)),
            Either::Left((Either::Right((Err(error), _)), _)) => match error {
                tokio::sync::broadcast::error::RecvError::Lagged(_) => Woken::Event(None),
                tokio::sync::broadcast::error::RecvError::Closed => Woken::Disconnected,
            },
            Either::Right(_) => Woken::Disconnected,
        },
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

impl Ran<'_> {
    async fn run(&mut self, command: Command) -> bool {
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
    match be_client::contacts::finish_links(peer.connection(), identity, peer.organization()).await
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
        peer.connection(),
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
                be_client::contacts::make_link(peer.connection(), identity, &me, offer.as_ref())
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
            let opened = be_client::contacts::open_link(peer.connection(), &code)
                .await
                .map(|(_, card)| card);
            reply(opened.map_err(|error| error.to_string()));
        }
        ContactCommand::Accept { code, reply } => {
            let accepted =
                be_client::contacts::accept_link(peer.connection(), identity, &me, &code).await;
            if accepted.is_ok() {
                sync_contacts(peer, shared).await;
            }
            reply(accepted.map_err(|error| error.to_string()));
        }
        ContactCommand::Remove(account) => {
            match be_client::contacts::remove_contact(peer.connection(), account).await {
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
                && let Some(head) = summary.head
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
    let Some(head) = peer.remote_head(block).await? else {
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
    if peer.remote_head(block).await?.is_some() {
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
    let mut expected = peer.remote_head(block).await?;
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
        let (unlisted, wanted) = {
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            (held.graph.take_unlisted(), held.graph.take_wanted())
        };
        let unlists = futures_util::future::join_all(
            unlisted.into_iter().map(|listing| peer.unlist(listing)),
        );
        if wanted.is_empty() {
            unlists.await;
            return fetched;
        }
        fetched = true;
        let lists = futures_util::future::join_all(wanted.iter().map(|listing| async move {
            match *listing {
                Listing::Children(parent) => peer.children(parent).await,
                Listing::Deleted => peer.deleted().await,
                Listing::Shared => peer.shared().await,
                Listing::Backrefs(block) => peer.backrefs(block).await,
                Listing::References(block) => peer.referenced(block).await,
                Listing::Block(block) => peer.summary(block).await.map(|summary| vec![summary]),
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
    }
}

async fn connect(
    config: &Config,
    store: Store,
) -> Result<(Peer<Store>, tokio::sync::OwnedMutexGuard<()>), ClientError> {
    let link = crate::link::signed_in(&config.server_url, &config.token).await?;
    let open = link.open_organization().await;
    let peer = Peer::signed_in(
        link.connection,
        link.account,
        config.token.clone(),
        PeerConfig::new(
            config.socket_url(),
            be_keys::RecoverySecret::from_bytes(config.identity),
            Credentials::Token(config.token.clone()),
        )
        .organization(Some(config.organization)),
        store,
    )
    .await?;
    Ok((peer, open))
}

pub(super) fn record(shared: &Arc<Mutex<Shared>>, error: ClientError) {
    shared.lock().unwrap_or_else(PoisonError::into_inner).error = Some(error.to_string());
}

fn fail(shared: &Arc<Mutex<Shared>>, error: String) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    held.connected = false;
    held.error = Some(error);
}
