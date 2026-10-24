use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, PoisonError},
    time::{Duration, Instant},
};

use be_block::history::History as UndoHistory;
use be_block::{BlockContent, BlockMetadata, Landed, LiveEdit, Merge, Undo};
use be_client::{ClientError, Conflict, Credentials, Journaled, Live, Peer, PeerConfig, Saved};
use be_commit::CommitId;
use be_graph::{Access, BlockParent};
use be_protocol::{AccessEntry, BlockSummary, ClientMessage, ServerMessage};
use block_plugin_api::GesturePhase;
use futures_util::future::{Either, LocalBoxFuture, select};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use super::graph::{Graph, Listing, Node, Owner, Query};
use super::version::{Context, Versions};
use super::{Config, Content, platform};

mod uploads;

use uploads::Uploads;

const SEAL_INTERVAL: Duration = Duration::from_millis(750);
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

type AccessReply = Box<dyn FnOnce(Result<Vec<AccessEntry>, String>) + Send>;

type ExportReply = Box<dyn FnOnce(Result<(String, Vec<u8>), String>) + Send>;

type HeadReply = Box<dyn FnOnce(Result<Vec<u8>, String>) + Send>;

pub(super) enum Command {
    PreviewChild {
        alias: Uuid,
        source: Uuid,
        content_type: Uuid,
    },
    Revision {
        block: Uuid,
        request: block_plugin_api::HostRequest,
        reply: Box<dyn FnOnce(block_plugin_api::HostReply) + Send>,
    },
    Open {
        block: Uuid,
        content_type: Uuid,
        owned: bool,
    },
    Close(Uuid),
    Operate(Uuid, Option<u64>, Vec<u8>, Instant),
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
    Gesture {
        block: Uuid,
        origin: Option<u64>,
        phase: GesturePhase,
    },
    Presence {
        block: Uuid,
        kind: Uuid,
        value: Option<Vec<u8>>,
    },
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
            | Command::Close(block)
            | Command::Operate(block, ..)
            | Command::Seed { block, .. }
            | Command::Replace { block, .. }
            | Command::History { block, .. }
            | Command::Gesture { block, .. }
            | Command::Presence { block, .. }
            | Command::SendToPeers { block, .. }
            | Command::Create { block, .. }
            | Command::SetParent { block, .. }
            | Command::Define { block, .. }
            | Command::SetAccess { block, .. }
            | Command::ListAccess { block, .. }
            | Command::ReadHead { block, .. }
            | Command::Version { block, .. }
            | Command::WatchVersion(block)
            | Command::Leave(block)
            | Command::Contact(ContactCommand::Link {
                offer: Some((block, _)),
                ..
            }) => Some(*block),
            Command::Duplicate { to, .. } => Some(*to),
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

const PEER_MESSAGE_LIMIT: usize = 4096;

#[derive(Default)]
pub(crate) struct Shared {
    pub(crate) revision_previews: HashMap<Uuid, super::revisions::Preview>,
    pub(crate) preview_aliases: HashMap<(Uuid, Uuid), Uuid>,
    pub(crate) released_previews: Vec<Uuid>,
    pub(crate) graph: Graph,
    pub(crate) blocks: HashMap<Uuid, Content>,
    pub(crate) histories: HashMap<Uuid, History>,
    pub(crate) presence: HashMap<Uuid, (u64, Vec<Presence>)>,
    pub(crate) presence_revision: u64,
    pub(crate) peer_messages: HashMap<Uuid, std::collections::VecDeque<(u64, PeerMessage)>>,
    pub(crate) peer_message_count: u64,
    pub(crate) wakes: u64,
    pub(crate) unsealed: usize,
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

pub(super) type Join = for<'a> fn(
    &'a Arc<Peer<Store>>,
    Uuid,
) -> LocalBoxFuture<'a, Result<Box<dyn Session>, ClientError>>;

pub(super) type Copy = for<'a> fn(
    &'a Arc<Peer<Store>>,
    Uuid,
    Uuid,
    Option<Vec<u8>>,
) -> LocalBoxFuture<'a, Result<(), ClientError>>;

pub(super) fn copy<C>(
    peer: &Arc<Peer<Store>>,
    from: Uuid,
    to: Uuid,
    shown: Option<Vec<u8>>,
) -> LocalBoxFuture<'_, Result<(), ClientError>>
where
    C: BlockContent + Default,
{
    Box::pin(async move {
        let content = match shown.and_then(|bytes| C::decode(&bytes).ok()) {
            Some(content) => content,
            None => peer.open::<C>(from).await?.unwrap_or_default(),
        };
        peer.ensure::<C>(to, BlockParent::Detached).await?;
        peer.save(to, &content, None).await?;
        Ok(())
    })
}

pub(super) type Seed =
    for<'a> fn(&'a Arc<Peer<Store>>, Uuid, Vec<u8>) -> LocalBoxFuture<'a, Result<(), ClientError>>;

pub(super) fn seed<C>(
    peer: &Arc<Peer<Store>>,
    block: Uuid,
    bytes: Vec<u8>,
) -> LocalBoxFuture<'_, Result<(), ClientError>>
where
    C: BlockContent + Default,
{
    Box::pin(async move {
        let Ok(content) = C::decode(&bytes) else {
            return Ok(());
        };
        peer.ensure::<C>(block, BlockParent::Detached).await?;
        if peer.remote_head(block).await?.is_some() {
            return Ok(());
        }
        peer.save(block, &content, None).await?;
        Ok(())
    })
}

pub(super) fn replace<C>(
    peer: &Arc<Peer<Store>>,
    block: Uuid,
    bytes: Vec<u8>,
) -> LocalBoxFuture<'_, Result<(), ClientError>>
where
    C: BlockContent + Default,
{
    Box::pin(async move {
        let Ok(content) = C::decode(&bytes) else {
            return Ok(());
        };
        peer.ensure::<C>(block, BlockParent::Detached).await?;
        let mut expected = peer.remote_head(block).await?;
        for _ in 0..4 {
            match peer.save(block, &content, expected).await? {
                be_client::Saved::Rejected { head } => expected = head,
                _ => return Ok(()),
            }
        }
        Ok(())
    })
}

pub(super) trait Session {
    fn content_type(&self) -> Uuid;

    fn bytes(&self) -> Vec<u8>;

    fn session_state(&self) -> Vec<u8>;

    fn derived(&self) -> Option<be_block::Entries>;

    fn references(&self) -> Vec<Uuid>;

    fn head(&self) -> Option<CommitId>;

    fn is_clean(&self) -> bool;

    fn owes_seal(&self) -> bool;

    fn edit<'a>(
        &'a mut self,
        operation: &'a [u8],
        origin: Option<u64>,
        at: Instant,
    ) -> LocalBoxFuture<'a, Result<(), ClientError>>;

    fn take_log(&mut self) -> Vec<Logged>;

    fn poll(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn seal(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn rekey(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn replace(&mut self, bytes: Vec<u8>) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn show(
        &mut self,
        kind: Uuid,
        value: Option<Vec<u8>>,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn take_presence(&mut self) -> Option<Vec<Presence>>;

    fn send_to_peers(
        &mut self,
        to: Option<u64>,
        channel: Uuid,
        payload: Vec<u8>,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn take_peer_messages(&mut self) -> Vec<PeerMessage>;

    fn take_conflicts(&mut self) -> Vec<Conflict>;

    fn published_elsewhere(
        &mut self,
        head: CommitId,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn history(&self) -> History {
        History::default()
    }

    fn step_history(&mut self, redo: bool) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        let _ = redo;
        Box::pin(async { Ok(()) })
    }

    fn gesture(
        &mut self,
        origin: Option<u64>,
        phase: GesturePhase,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        let _ = (origin, phase);
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PeerMessage {
    pub(crate) from: u64,
    pub(crate) channel: Uuid,
    pub(crate) payload: Vec<u8>,
}

fn take_peer_messages<C>(live: &mut Live<Store, C>) -> Vec<PeerMessage>
where
    C: LiveEdit + Clone + Default,
{
    live.take_channel()
        .into_iter()
        .map(|message| PeerMessage {
            from: message.from,
            channel: message.channel,
            payload: message.payload,
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Presence {
    pub(crate) client: u64,
    pub(crate) kind: Uuid,
    pub(crate) value: Vec<u8>,
}

fn take_presence<C>(live: &mut Live<Store, C>) -> Option<Vec<Presence>>
where
    C: LiveEdit + Clone + Default,
{
    live.take_presence_changed().then(|| {
        live.presence()
            .iter()
            .map(|((client, kind), value)| Presence {
                client: *client,
                kind: *kind,
                value: value.clone(),
            })
            .collect()
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct History {
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
}

pub(super) fn join_with_history<C>(
    peer: &Arc<Peer<Store>>,
    block: Uuid,
) -> LocalBoxFuture<'_, Result<Box<dyn Session>, ClientError>>
where
    C: Undo + Merge + Clone + Default,
{
    Box::pin(async move {
        peer.ensure::<C>(block, BlockParent::Detached).await?;
        let mut live = Live::<Store, C>::join(Arc::clone(peer), block).await?;
        live.reconcile().await?;
        Ok(Box::new(WithHistory {
            live,
            log: Vec::new(),
            history: UndoHistory::default(),
        }) as Box<dyn Session>)
    })
}

struct WithHistory<C: Undo> {
    live: Live<Store, C>,
    log: Vec<Logged>,
    history: UndoHistory<C>,
}

impl<C> Session for WithHistory<C>
where
    C: Undo + Merge + Clone + Default,
{
    fn content_type(&self) -> Uuid {
        C::CONTENT_TYPE
    }

    fn bytes(&self) -> Vec<u8> {
        self.live.content().encode()
    }

    fn session_state(&self) -> Vec<u8> {
        self.live.content().session_state()
    }

    fn derived(&self) -> Option<be_block::Entries> {
        Some(be_block::derived_metadata(self.live.content()))
    }

    fn references(&self) -> Vec<Uuid> {
        self.live.references()
    }

    fn head(&self) -> Option<CommitId> {
        self.live.head()
    }

    fn is_clean(&self) -> bool {
        self.live.is_clean()
    }

    fn owes_seal(&self) -> bool {
        self.live.is_owner() && !self.live.is_clean()
    }

    fn edit<'a>(
        &'a mut self,
        operation: &'a [u8],
        origin: Option<u64>,
        at: Instant,
    ) -> LocalBoxFuture<'a, Result<(), ClientError>> {
        Box::pin(async move {
            let Ok(operation) = C::decode_operation(operation) else {
                return Ok(());
            };
            self.history
                .record(self.live.content(), &operation, origin, Some(at));
            drain(&mut self.live, &mut self.log, None);
            let edited = self.live.edit(operation).await;
            drain(&mut self.live, &mut self.log, origin);
            edited
        })
    }

    fn take_log(&mut self) -> Vec<Logged> {
        drain(&mut self.live, &mut self.log, None);
        std::mem::take(&mut self.log)
    }

    fn poll(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move {
            self.live.poll().await?;
            self.live.catch_up().await
        })
    }

    fn seal(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { seal(&mut self.live).await })
    }

    fn rekey(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { rekey(&mut self.live).await })
    }

    fn replace(&mut self, bytes: Vec<u8>) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move {
            let Ok(content) = C::decode(&bytes) else {
                return Ok(());
            };
            self.history.clear();
            drain(&mut self.live, &mut self.log, None);
            let replaced = self.live.replace(content).await;
            drain(&mut self.live, &mut self.log, None);
            match replaced? {
                true => Ok(()),
                false => Err(ClientError::Unexpected),
            }
        })
    }

    fn show(
        &mut self,
        kind: Uuid,
        value: Option<Vec<u8>>,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.set_presence(kind, value).await })
    }

    fn take_presence(&mut self) -> Option<Vec<Presence>> {
        take_presence(&mut self.live)
    }

    fn send_to_peers(
        &mut self,
        to: Option<u64>,
        channel: Uuid,
        payload: Vec<u8>,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.send_channel(to, channel, payload).await })
    }

    fn take_peer_messages(&mut self) -> Vec<PeerMessage> {
        take_peer_messages(&mut self.live)
    }

    fn take_conflicts(&mut self) -> Vec<Conflict> {
        self.live.take_conflicts()
    }

    fn published_elsewhere(
        &mut self,
        head: CommitId,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.published_elsewhere(head).await })
    }

    fn history(&self) -> History {
        History {
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
        }
    }

    fn step_history(&mut self, redo: bool) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move {
            let operations = match redo {
                false => self.history.undo(self.live.content()),
                true => self.history.redo(self.live.content()),
            };
            for operation in operations {
                self.live.edit(operation).await?;
            }
            Ok(())
        })
    }

    fn gesture(
        &mut self,
        origin: Option<u64>,
        phase: GesturePhase,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move {
            let operations = match phase {
                GesturePhase::Begin => {
                    self.history.begin(origin);
                    Vec::new()
                }
                GesturePhase::Commit => {
                    self.history.commit(origin);
                    Vec::new()
                }
                GesturePhase::Cancel => self.history.cancel(self.live.content(), origin),
            };
            for operation in operations {
                drain(&mut self.live, &mut self.log, None);
                self.live.edit(operation).await?;
            }
            drain(&mut self.live, &mut self.log, None);
            Ok(())
        })
    }
}

pub(super) fn join<C>(
    peer: &Arc<Peer<Store>>,
    block: Uuid,
) -> LocalBoxFuture<'_, Result<Box<dyn Session>, ClientError>>
where
    C: LiveEdit + Merge + Clone + Default,
{
    Box::pin(async move {
        peer.ensure::<C>(block, BlockParent::Detached).await?;
        let mut live = Live::<Store, C>::join(Arc::clone(peer), block).await?;
        live.reconcile().await?;
        Ok(Box::new(Plain {
            live,
            log: Vec::new(),
        }) as Box<dyn Session>)
    })
}

struct Plain<C: LiveEdit> {
    live: Live<Store, C>,
    log: Vec<Logged>,
}

impl<C> Session for Plain<C>
where
    C: LiveEdit + Merge + Clone + Default,
{
    fn content_type(&self) -> Uuid {
        C::CONTENT_TYPE
    }

    fn bytes(&self) -> Vec<u8> {
        self.live.content().encode()
    }

    fn session_state(&self) -> Vec<u8> {
        self.live.content().session_state()
    }

    fn derived(&self) -> Option<be_block::Entries> {
        Some(be_block::derived_metadata(self.live.content()))
    }

    fn references(&self) -> Vec<Uuid> {
        self.live.references()
    }

    fn head(&self) -> Option<CommitId> {
        self.live.head()
    }

    fn is_clean(&self) -> bool {
        self.live.is_clean()
    }

    fn owes_seal(&self) -> bool {
        self.live.is_owner() && !self.live.is_clean()
    }

    fn edit<'a>(
        &'a mut self,
        operation: &'a [u8],
        origin: Option<u64>,
        _at: Instant,
    ) -> LocalBoxFuture<'a, Result<(), ClientError>> {
        Box::pin(async move {
            let Ok(operation) = C::decode_operation(operation) else {
                return Ok(());
            };
            drain(&mut self.live, &mut self.log, None);
            let edited = self.live.edit(operation).await;
            drain(&mut self.live, &mut self.log, origin);
            edited
        })
    }

    fn take_log(&mut self) -> Vec<Logged> {
        drain(&mut self.live, &mut self.log, None);
        std::mem::take(&mut self.log)
    }

    fn poll(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move {
            self.live.poll().await?;
            self.live.catch_up().await
        })
    }

    fn seal(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { seal(&mut self.live).await })
    }

    fn rekey(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { rekey(&mut self.live).await })
    }

    fn replace(&mut self, bytes: Vec<u8>) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move {
            let Ok(content) = C::decode(&bytes) else {
                return Ok(());
            };
            drain(&mut self.live, &mut self.log, None);
            let replaced = self.live.replace(content).await;
            drain(&mut self.live, &mut self.log, None);
            match replaced? {
                true => Ok(()),
                false => Err(ClientError::Unexpected),
            }
        })
    }

    fn show(
        &mut self,
        kind: Uuid,
        value: Option<Vec<u8>>,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.set_presence(kind, value).await })
    }

    fn take_presence(&mut self) -> Option<Vec<Presence>> {
        take_presence(&mut self.live)
    }

    fn send_to_peers(
        &mut self,
        to: Option<u64>,
        channel: Uuid,
        payload: Vec<u8>,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.send_channel(to, channel, payload).await })
    }

    fn take_peer_messages(&mut self) -> Vec<PeerMessage> {
        take_peer_messages(&mut self.live)
    }

    fn take_conflicts(&mut self) -> Vec<Conflict> {
        self.live.take_conflicts()
    }

    fn published_elsewhere(
        &mut self,
        head: CommitId,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.published_elsewhere(head).await })
    }
}

async fn seal<C>(live: &mut Live<Store, C>) -> Result<(), ClientError>
where
    C: LiveEdit + Merge + Clone + Default,
{
    if !live.is_owner() || live.is_clean() {
        return Ok(());
    }
    if matches!(live.seal().await?, Saved::Rejected { .. }) {
        live.reconcile().await?;
        live.seal().await?;
    }
    Ok(())
}

async fn rekey<C>(live: &mut Live<Store, C>) -> Result<(), ClientError>
where
    C: LiveEdit + Merge + Clone + Default,
{
    if !live.is_owner() || live.is_diverged() {
        return Ok(());
    }
    if matches!(live.seal().await?, Saved::Rejected { .. }) {
        live.reconcile().await?;
        live.seal().await?;
    }
    Ok(())
}

pub(crate) enum Logged {
    Operation {
        landed: Landed<Vec<u8>>,
        origin: Option<u64>,
    },
    Replaced {
        acknowledged: HashMap<u64, u64>,
    },
}

fn drain<C>(live: &mut Live<Store, C>, log: &mut Vec<Logged>, origin: Option<u64>)
where
    C: LiveEdit + Clone + Default,
{
    for entry in live.take_journal() {
        let from = match entry {
            Journaled::Edited { .. } => origin,
            _ => None,
        };
        if let Journaled::Replaced { edits } = entry {
            let mut acknowledged = HashMap::new();
            for logged in log.drain(..) {
                match logged {
                    Logged::Operation {
                        origin: Some(origin),
                        ..
                    } => *acknowledged.entry(origin).or_default() += 1,
                    Logged::Operation { origin: None, .. } => {}
                    Logged::Replaced {
                        acknowledged: earlier,
                    } => {
                        for (origin, count) in earlier {
                            *acknowledged.entry(origin).or_default() += count;
                        }
                    }
                }
            }
            if let Some(origin) = origin
                && edits > 0
            {
                *acknowledged.entry(origin).or_default() += edits;
            }
            log.push(Logged::Replaced { acknowledged });
            continue;
        }
        if let Some(landed) = entry.landed() {
            log.push(Logged::Operation {
                landed: landed.map(C::encode_operation),
                origin: from,
            });
        }
    }
}

pub(super) async fn serve<S: Fn() -> Result<Store, String>>(
    config: Config,
    make_store: S,
    mut commands: UnboundedReceiver<Command>,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
) {
    let mut open: HashMap<Uuid, Uuid> = HashMap::new();
    let mut uploads = Uploads::new(&shared);
    loop {
        if let Outcome::Stopped = connected(
            &config,
            &make_store,
            &mut commands,
            &shared,
            &changed,
            &mut open,
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

enum Outcome {
    Stopped,
    Lost,
}

async fn connected<S: Fn() -> Result<Store, String>>(
    config: &Config,
    make_store: &S,
    commands: &mut UnboundedReceiver<Command>,
    shared: &Arc<Mutex<Shared>>,
    changed: &Arc<Condvar>,
    open: &mut HashMap<Uuid, Uuid>,
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
    want_open(shared, open);
    relist(shared);
    fetch_listings(&peer, shared).await;
    sync_contacts(&peer, shared).await;
    let mut sessions: HashMap<Uuid, Box<dyn Session>> = HashMap::new();
    let rejoining: Vec<(Uuid, Uuid)> = open.clone().into_iter().collect();
    let rejoined = futures_util::future::join_all(
        rejoining
            .iter()
            .map(|(block, content_type)| join_session(&peer, shared, *block, *content_type)),
    )
    .await;
    for ((block, _), session) in rejoining.into_iter().zip(rejoined) {
        if let Some(session) = session {
            sessions.insert(block, session);
        }
    }
    let mut versions = Versions::default();
    publish(&mut sessions, shared);
    versions
        .refresh(&mut Context {
            peer: &peer,
            shared,
            sessions: &mut sessions,
        })
        .await;
    changed.notify_all();
    crate::host::wake();
    let mut unsealed_since: Option<Instant> = None;
    let mut flushes = Vec::new();
    let mut keyed: HashMap<Uuid, Uuid> = HashMap::new();
    let mut stopping = false;
    uploads.connect(&peer);
    loop {
        if stopping && uploads.is_empty() {
            seal_all(&mut sessions, shared).await;
            return Outcome::Stopped;
        }
        let deadline = unsealed_since.map(|since| since + SEAL_INTERVAL);
        let woken = match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let waited =
                    std::pin::pin!(wait(commands, &mut events, &mut gone, uploads, stopping));
                let timer = std::pin::pin!(platform::sleep(remaining));
                match select(waited, timer).await {
                    Either::Left((woken, _)) => woken,
                    Either::Right(_) => Woken::Deadline,
                }
            }
            None => wait(commands, &mut events, &mut gone, uploads, stopping).await,
        };
        shared.lock().unwrap_or_else(PoisonError::into_inner).wakes += 1;
        let mut ran = Ran {
            peer: &peer,
            sessions: &mut sessions,
            shared,
            config,
            open: &mut *open,
            versions: &mut versions,
            flushes: &mut flushes,
        };
        let sealed = match woken {
            Woken::Stopped => {
                stopping = true;
                false
            }
            Woken::Disconnected if stopping => return Outcome::Stopped,
            Woken::Disconnected => return Outcome::Lost,
            Woken::Command(Command::Import(blocks)) => {
                uploads.add(blocks);
                ran.done();
                false
            }
            Woken::Command(command) => {
                let mut next = Some(command);
                let mut applied = false;
                while let Some(command) = next.take() {
                    let edits = matches!(command, Command::Operate(..) | Command::Gesture { .. });
                    match command {
                        Command::Import(blocks) => {
                            uploads.add(blocks);
                            ran.done();
                        }
                        command @ Command::Open { .. } => {
                            let mut opens = vec![command];
                            while let Ok(queued) = commands.try_recv() {
                                match queued {
                                    queued @ Command::Open { .. } => opens.push(queued),
                                    queued => {
                                        next = Some(queued);
                                        break;
                                    }
                                }
                            }
                            let opens = opens
                                .into_iter()
                                .filter_map(|open| uploads.defer(open))
                                .collect();
                            ran.open_all(opens).await;
                        }
                        command => {
                            if let Some(command) = uploads.defer(command) {
                                applied |= ran.run(command).await;
                            }
                        }
                    }
                    if edits {
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
                heard(&peer, shared, event).await;
                loop {
                    match events.try_recv() {
                        Ok(event) => heard(&peer, shared, Some(event)).await,
                        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                            heard(&peer, shared, None).await;
                        }
                        Err(_) => break,
                    }
                }
                for session in sessions.values_mut() {
                    if let Err(error) = session.poll().await {
                        record(shared, error);
                    }
                }
                false
            }
            Woken::Deadline => {
                let due = unsealed_since.is_some_and(|since| since.elapsed() >= SEAL_INTERVAL);
                if due {
                    seal_all(&mut sessions, shared).await;
                }
                due
            }
        };
        rekey_changed(&peer, &mut sessions, &mut keyed, shared).await;
        if sealed {
            unsealed_since = None;
        }
        unsealed_since = match sessions.values().any(|session| session.owes_seal()) {
            true => unsealed_since.or_else(|| Some(Instant::now())),
            false => None,
        };
        let mut moved = publish(&mut sessions, shared);
        let before = version_revisions(shared);
        versions
            .refresh(&mut Context {
                peer: &peer,
                shared,
                sessions: &mut sessions,
            })
            .await;
        moved |= version_revisions(shared) != before;
        want_open(shared, open);
        moved |= fetch_listings(&peer, shared).await;
        if events.is_empty() && uploads.is_empty() && !flushes.is_empty() {
            seal_all(&mut sessions, shared).await;
            publish(&mut sessions, shared);
            for done in flushes.drain(..) {
                let _ = done.send(());
            }
        }
        changed.notify_all();
        if moved {
            crate::host::wake();
        }
    }
}

async fn rekey_changed(
    peer: &Peer<Store>,
    sessions: &mut HashMap<Uuid, Box<dyn Session>>,
    keyed: &mut HashMap<Uuid, Uuid>,
    shared: &Arc<Mutex<Shared>>,
) {
    keyed.retain(|block, _| sessions.contains_key(block));
    for (block, session) in sessions.iter_mut() {
        let Some(key) = peer.current_key(*block) else {
            continue;
        };
        if keyed
            .insert(*block, key)
            .is_some_and(|previous| previous != key)
            && let Err(error) = session.rekey().await
        {
            record(shared, error);
        }
    }
}

enum Woken {
    Command(Command),
    Uploaded,
    Event(Option<ServerMessage>),
    Deadline,
    Disconnected,
    Stopped,
}

async fn wait(
    commands: &mut UnboundedReceiver<Command>,
    events: &mut tokio::sync::broadcast::Receiver<be_protocol::ServerMessage>,
    gone: &mut tokio::sync::watch::Receiver<bool>,
    uploads: &mut Uploads,
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
    match select(select(select(command, event), closed), uploaded).await {
        Either::Right(_) => Woken::Uploaded,
        Either::Left((woken, _)) => match woken {
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
    sessions: &'a mut HashMap<Uuid, Box<dyn Session>>,
    shared: &'a Arc<Mutex<Shared>>,
    config: &'a Config,
    open: &'a mut HashMap<Uuid, Uuid>,
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
                    self.sessions,
                    self.shared,
                    self.open,
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

    async fn open_all(&mut self, opens: Vec<Command>) {
        let mut joining: Vec<(Uuid, Uuid)> = Vec::new();
        let count = opens.len();
        for command in opens {
            let Command::Open {
                block,
                content_type,
                owned,
            } = command
            else {
                continue;
            };
            if owned {
                self.shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .owned
                    .insert(block);
            }
            self.open.insert(block, content_type);
            if !self.sessions.contains_key(&block)
                && !joining.iter().any(|(joined, _)| *joined == block)
            {
                joining.push((block, content_type));
            }
        }
        want_open(self.shared, self.open);
        let joined = futures_util::future::join_all(joining.iter().map(|(block, content_type)| {
            join_session(self.peer, self.shared, *block, *content_type)
        }))
        .await;
        for ((block, _), session) in joining.into_iter().zip(joined) {
            if let Some(session) = session {
                self.sessions.insert(block, session);
            }
        }
        for _ in 0..count {
            self.done();
        }
    }

    fn done(&self) {
        {
            let mut held = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
            held.pending = held.pending.saturating_sub(1);
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

async fn rejoin(
    peer: &Arc<Peer<Store>>,
    sessions: &mut HashMap<Uuid, Box<dyn Session>>,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    content_type: Uuid,
) {
    if let Some(session) = join_session(peer, shared, block, content_type).await {
        sessions.insert(block, session);
    }
}

async fn join_session(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    content_type: Uuid,
) -> Option<Box<dyn Session>> {
    let kind = super::kind_of(content_type)?;
    let owned = !kind.frozen
        && shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .owned
            .contains(&block);
    let joined = (kind.join)(peer, block).await;
    set_locked(shared, block, matches!(joined, Err(ClientError::Locked(_))));
    match joined {
        Err(ClientError::Locked(_)) => None,
        Ok(mut session) => {
            set_unreadable(shared, block, None);
            if owned && reshaped(shared, block, kind) {
                match session.replace((kind.blank)()).await {
                    Ok(()) => reset_notice(),
                    Err(error) => record(shared, error),
                }
            }
            Some(session)
        }
        Err(ClientError::Content(_)) if owned => {
            if let Err(error) = (kind.replace)(peer, block, (kind.blank)()).await {
                set_unreadable(shared, block, Some(error.to_string()));
                return None;
            }
            reset_notice();
            match (kind.join)(peer, block).await {
                Ok(session) => {
                    set_unreadable(shared, block, None);
                    Some(session)
                }
                Err(error) => {
                    set_unreadable(shared, block, Some(error.to_string()));
                    None
                }
            }
        }
        Err(ClientError::Content(error)) => {
            set_unreadable(shared, block, Some(error.to_string()));
            None
        }
        Err(error) => {
            record(shared, error);
            None
        }
    }
}

fn reshaped(shared: &Arc<Mutex<Shared>>, block: Uuid, kind: &super::Kind) -> bool {
    shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .graph
        .get(block)
        .and_then(|node| node.metadata.get(&be_block::SCHEMA))
        .is_some_and(|stamped| stamped != (kind.schema)())
}

fn reset_notice() {
    crate::notices::report(
        "Some of this device's settings were saved by another build of Block, so they were reset."
            .to_owned(),
    );
}

fn set_locked(shared: &Arc<Mutex<Shared>>, block: Uuid, locked: bool) {
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

fn set_unreadable(shared: &Arc<Mutex<Shared>>, block: Uuid, error: Option<String>) {
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
    sessions: &mut HashMap<Uuid, Box<dyn Session>>,
    shared: &Arc<Mutex<Shared>>,
    open: &mut HashMap<Uuid, Uuid>,
    versions: &mut Versions,
    flushes: &mut Vec<std::sync::mpsc::Sender<()>>,
    command: Command,
) -> bool {
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
            seal_all(sessions, shared).await;
            publish(sessions, shared);
            let outcome = versions
                .run(
                    &mut Context {
                        peer,
                        shared,
                        sessions,
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
            let result = super::revisions::run(peer, sessions, shared, block, request).await;
            publish(sessions, shared);
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
            }
            open.insert(block, content_type);
            want_open(shared, open);
            if sessions.contains_key(&block) {
                return false;
            }
            rejoin(peer, sessions, shared, block, content_type).await;
            false
        }
        Command::Close(block) => {
            open.remove(&block);
            want_open(shared, open);
            let Some(mut session) = sessions.remove(&block) else {
                return false;
            };
            if let Err(error) = session.seal().await {
                record(shared, error);
            }
            let _ = peer.leave_session(block).await;
            let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            held.blocks.remove(&block);
            held.graph.forget_derived(block);
            held.graph.forget_references(block);
            held.previewed.remove(&block);
            held.histories.remove(&block);
            held.presence.remove(&block);
            held.peer_messages.remove(&block);
            held.touch(block);
            true
        }
        Command::Operate(block, origin, operation, at) => {
            let Some(session) = sessions.get_mut(&block) else {
                return false;
            };
            if let Err(error) = session.edit(&operation, origin, at).await {
                record(shared, error);
            }
            false
        }
        Command::Duplicate {
            from,
            to,
            content_type,
            metadata,
        } => {
            let Some(copy) = super::copy_for(content_type) else {
                shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .graph
                    .settle(to);
                return false;
            };
            let created = peer
                .create_block(to, content_type, BlockParent::Detached, &metadata)
                .await;
            if let Err(error) = settled(peer, shared, to, created).await {
                record(shared, error);
                return false;
            }
            let shown = sessions.get(&from).map(|session| session.bytes());
            if let Err(error) = copy(peer, from, to, shown).await {
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
            let outcome = match sessions.get_mut(&block) {
                Some(session) => session.replace(bytes).await,
                None => match super::replace_for(content_type) {
                    Some(replace) => replace(peer, block, bytes).await,
                    None => Ok(()),
                },
            };
            match outcome {
                Ok(()) if !sessions.contains_key(&block) && open.contains_key(&block) => {
                    rejoin(peer, sessions, shared, block, content_type).await;
                }
                Ok(()) => {}
                Err(error) => record(shared, error),
            }
            remember_head(peer, shared, block);
            false
        }
        Command::Seed {
            block,
            content_type,
            bytes,
        } => {
            if sessions.contains_key(&block) {
                return false;
            }
            let Some(seed) = super::seed_for(content_type) else {
                return false;
            };
            if let Err(error) = seed(peer, block, bytes).await {
                record(shared, error);
            }
            remember_head(peer, shared, block);
            false
        }
        Command::Presence { block, kind, value } => {
            let Some(session) = sessions.get_mut(&block) else {
                return false;
            };
            if let Err(error) = session.show(kind, value).await {
                record(shared, error);
            }
            false
        }
        Command::SendToPeers {
            block,
            to,
            channel,
            payload,
        } => {
            let Some(session) = sessions.get_mut(&block) else {
                return false;
            };
            if let Err(error) = session.send_to_peers(to, channel, payload).await {
                record(shared, error);
            }
            false
        }
        Command::History { block, redo } => {
            let Some(session) = sessions.get_mut(&block) else {
                return false;
            };
            if let Err(error) = session.step_history(redo).await {
                record(shared, error);
            }
            false
        }
        Command::Gesture {
            block,
            origin,
            phase,
        } => {
            let Some(session) = sessions.get_mut(&block) else {
                return false;
            };
            if let Err(error) = session.gesture(origin, phase).await {
                record(shared, error);
            }
            false
        }
        Command::Create {
            block,
            content_type,
            parent,
            metadata,
            bytes,
        } => {
            let created = peer
                .create_block(block, content_type, parent, &metadata)
                .await;
            if let Err(error) = settled(peer, shared, block, created).await {
                record(shared, error);
                return false;
            }
            if let (Some(bytes), Some(seed)) = (bytes, super::seed_for(content_type))
                && let Err(error) = seed(peer, block, bytes).await
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
                && let Some(session) = sessions.get_mut(&block)
                && let Err(error) = session.published_elsewhere(head).await
            {
                record(shared, error);
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
            let exported = export(peer, sessions, shared, block).await;
            reply(exported.map_err(|error| error.to_string()));
            crate::host::wake();
            false
        }
        Command::Import(_) => false,
        Command::Flush(done) => {
            seal_all(sessions, shared).await;
            publish(sessions, shared);
            flushes.push(done);
            true
        }
    }
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
    sessions: &HashMap<Uuid, Box<dyn Session>>,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
) -> Result<(String, Vec<u8>), ClientError> {
    let picked = peer.summary(block).await?;
    let mut top = picked.clone();
    if picked.content_type == be_block::DatabaseContent::CONTENT_TYPE
        && let Some(BlockParent::Block(parent)) = picked.parent
        && let Ok(view) = peer.summary(parent).await
        && view.content_type == be_block::DatabaseViewContent::CONTENT_TYPE
    {
        top = view;
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
        .filter(|block| !sessions.contains_key(&block.id))
        .map(|block| (block.id, heads[&block.id]))
        .collect();
    let mut read = peer.read_contents(&wanted).await?.into_iter();
    for block in &mut blocks {
        block.content = match sessions.get(&block.id) {
            Some(session) => Some(session.bytes()),
            None => read.next().and_then(|(_, content)| content),
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

async fn seal_all(sessions: &mut HashMap<Uuid, Box<dyn Session>>, shared: &Arc<Mutex<Shared>>) {
    let sealed =
        futures_util::future::join_all(sessions.values_mut().map(|session| session.seal())).await;
    for error in sealed.into_iter().filter_map(Result::err) {
        record(shared, error);
    }
}

fn publish(sessions: &mut HashMap<Uuid, Box<dyn Session>>, shared: &Arc<Mutex<Shared>>) -> bool {
    let logs: HashMap<Uuid, Vec<Logged>> = sessions
        .iter_mut()
        .map(|(block, session)| (*block, session.take_log()))
        .collect();
    let known: HashMap<Uuid, (usize, bool)> = {
        let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        sessions
            .keys()
            .filter(|block| !held.previewed.contains(*block))
            .filter_map(|block| {
                let content = held.blocks.get(block)?;
                Some((*block, (content.unfolded(), content.lands(&logs[block]))))
            })
            .collect()
    };
    let unsealed = sessions
        .values()
        .filter(|session| !session.is_clean())
        .count();
    let owing = sessions
        .values()
        .filter(|session| session.owes_seal())
        .count();
    let mut logs = logs;
    let published: Vec<Published> = sessions
        .iter_mut()
        .map(|(block, session)| {
            let log = logs.remove(block).unwrap_or_default();
            let (snapshot, compact) = match known.get(block) {
                None => (true, false),
                Some((unfolded, lands)) => (
                    !lands
                        || log
                            .iter()
                            .any(|entry| matches!(entry, Logged::Replaced { .. })),
                    unfolded + log.len() > super::LOG_LIMIT / 2,
                ),
            };
            let content = (snapshot || compact || !log.is_empty()).then(|| {
                (
                    session.content_type(),
                    snapshot.then(|| (session.bytes(), session.session_state())),
                    session.derived(),
                    session.references(),
                )
            });
            Published {
                block: *block,
                history: session.history(),
                presence: session.take_presence(),
                peer_messages: session.take_peer_messages(),
                conflicts: session.take_conflicts(),
                head: session.head(),
                log,
                content,
            }
        })
        .collect();
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let mut changed = held.owing != owing;
    held.unsealed = unsealed;
    held.owing = owing;
    for published in published {
        let block = published.block;
        if held.histories.get(&block) != Some(&published.history) {
            held.histories.insert(block, published.history);
            held.touch(block);
            changed = true;
        }
        if let Some(presence) = published.presence {
            held.presence_revision += 1;
            let revision = held.presence_revision;
            held.presence.insert(block, (revision, presence));
            held.touch(block);
            changed = true;
        }
        if !published.peer_messages.is_empty() {
            for message in published.peer_messages {
                held.peer_message_count += 1;
                let sequence = held.peer_message_count;
                let queue = held.peer_messages.entry(block).or_default();
                queue.push_back((sequence, message));
                if queue.len() > PEER_MESSAGE_LIMIT {
                    queue.pop_front();
                }
            }
            held.touch(block);
            changed = true;
        }
        if !published.conflicts.is_empty() {
            let count: usize = published
                .conflicts
                .iter()
                .map(|conflict| conflict.count)
                .sum();
            held.error = Some(format!(
                "block {block} merged with {count} conflicting change(s); both versions are kept in its history"
            ));
            held.conflicts
                .entry(block)
                .or_default()
                .extend(published.conflicts);
            changed = true;
        }
        if held.graph.get(block).and_then(|node| node.head) != published.head {
            held.touch(block);
            changed = true;
        }
        held.graph.set_head(block, published.head);
        let Some((content_type, snapshot, derived, references)) = published.content else {
            continue;
        };
        held.graph.refer(block, references);
        held.previewed.remove(&block);
        match held.blocks.get_mut(&block) {
            Some(content) => {
                for entry in published.log {
                    content.record(entry);
                }
                match snapshot {
                    Some((bytes, session)) => content.settle(bytes, session),
                    None if content.unfolded() > super::LOG_LIMIT / 2 => content.compact(),
                    None => {}
                }
                content.derived = derived;
            }
            None => {
                let (bytes, session) = snapshot.unwrap_or_default();
                held.blocks
                    .insert(block, Content::new(content_type, bytes, session, derived));
            }
        }
        held.touch(block);
        changed = true;
    }
    changed
}

type PublishedContent = (
    Uuid,
    Option<(Vec<u8>, Vec<u8>)>,
    Option<be_block::Entries>,
    Vec<Uuid>,
);

struct Published {
    block: Uuid,
    history: History,
    presence: Option<Vec<Presence>>,
    peer_messages: Vec<PeerMessage>,
    conflicts: Vec<Conflict>,
    head: Option<CommitId>,
    log: Vec<Logged>,
    content: Option<PublishedContent>,
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

fn record(shared: &Arc<Mutex<Shared>>, error: ClientError) {
    shared.lock().unwrap_or_else(PoisonError::into_inner).error = Some(error.to_string());
}

fn fail(shared: &Arc<Mutex<Shared>>, error: String) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    held.connected = false;
    held.error = Some(error);
}
