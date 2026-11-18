use std::{
    collections::{BTreeSet, HashMap, HashSet},
    net::IpAddr,
    sync::Arc,
};

use be_block_graph::{Access, BlockParent};
use be_block_protocol::{
    AccountStorage, Authenticated, BrowserLogin, BrowserLoginState, Contents, CreatedAgent,
    DeviceLoginAnswer, DeviceLoginStep, Done, Error, ErrorCode, Event, Header, JoinedSession,
    LinkKind, Listing, LogRecord, MemberKind, Op, Page, PairingMessage, PushEndpointChange,
    PutKeys, Query, RecoveryKey, ServerInfo, SessionEvent, SessionSend, SessionState, Target,
    Topic, UploadUrl, WriteError, embed,
};
use be_wasm_ops::{Backlog, Incoming, OpError, OpId, Outlets, Reply, Started, net::Peer};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    Identity, PublishOutcome, SUMMARY_BYTES, ServerConfig, ServerError, ServerStore, Services,
    SessionRegistry, WatchHub, browser_auth, contacts, contents_room,
    limits::{
        MAX_PAIRING_BYTES, MAX_RELAY_BYTES, MAX_SESSIONS_PER_CONNECTION, MAX_STREAMS_PER_CONNECTION,
    },
    store,
    watch::{Place, Push},
};

/// Items the events stream keeps beyond the credit its consumer granted. A client that lets it
/// fill up is disconnected, as is one whose hub queue (`PUSHES`) fills.
const STREAM_BUFFER: usize = 4096;
/// Bytes the events stream keeps beyond the credit granted; a client that holds back more, by
/// not reading what it is sent, is disconnected too.
pub(crate) const STREAM_BUFFER_BYTES: usize = 4 * be_block_protocol::MAX_FRAME_BYTES;
const PUSHES: usize = 4096;

/// What a query, op or target needs before its handler runs. Each has one, stated in
/// `query_rule`, `op_rule`, `target_rule` and `topic_rule` alone, and the dispatcher
/// (`Client::admit`) applies it against the organization the call names; a handler then checks
/// only what its rule cannot say (a move's destination, a grant's grantee), under the graph
/// lock.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Rule {
    /// Only a one-shot call, never signed in, naming no organization: reading an invitation.
    OneShot,
    /// The account's own: signed in, naming no organization.
    Account,
    /// Signed in, naming no organization or one the account is a member of.
    Anywhere,
    /// A member of the organization named.
    Member,
    /// An administrator of the organization named.
    Administrator,
    /// At least this access to every block named, in the organization named. A block the caller
    /// may not know exists is refused as one that does not.
    Blocks(Vec<Uuid>, Access),
    /// At least this access to a block whose live session the connection is in.
    Session(Uuid, Access),
}

fn block(block: Uuid, level: Access) -> Rule {
    Rule::Blocks(vec![block], level)
}

/// Who may make each query. Its match names every case, so a new one does not compile until it
/// has a rule.
pub(crate) fn query_rule(query: &Query) -> Rule {
    match query {
        Query::Blocks {
            blocks,
            contents: true,
            ..
        } => Rule::Blocks(blocks.clone(), Access::View),
        // Summaries, a listing and a listing's `rest` leave out what the caller may not know of.
        Query::Blocks {
            contents: false, ..
        }
        | Query::List { .. } => Rule::Member,
        Query::Tree { entry: of, .. } => block(*of, Access::View),
        Query::History(of) => block(*of, Access::View),
        Query::BlockLog { block: of, .. } => block(*of, Access::View),
        // Each block the caller may not view is left out of the answer.
        Query::LogRedactions(_) => Rule::Member,
        // Of a block the caller views, or one above a block it views (`access_log`).
        Query::Log { .. } => Rule::Member,
        Query::Access(of) => block(*of, Access::Manage),
        Query::Names(of) => block(*of, Access::View),
        Query::Keys { .. }
        | Query::KeyWork
        | Query::Revocations { .. }
        | Query::SigningKeys(_)
        | Query::DownloadUrls(_)
        | Query::Mutes
        // Every member, to an administrator; the agents it made, to anyone else.
        | Query::Members => Rule::Member,
        Query::Invitations => Rule::Administrator,
        Query::RecoveryKey
        | Query::Organizations
        | Query::Storage
        | Query::Mailbox
        | Query::Link(_)
        | Query::ShareLink(_)
        | Query::ServerInfo => Rule::Account,
        Query::ShareLinks(of) => block(*of, Access::Manage),
        // A bearer read of an invitation, before its holder signs in (a one-shot call).
        Query::Invitation(_) => Rule::OneShot,
        // A Join member joins as one, hearing only the host channel (`Client::join`).
        Query::Session { block: of, .. } => block(*of, Access::Join),
    }
}

/// Who may make each op.
pub(crate) fn op_rule(op: &Op) -> Rule {
    match op {
        Op::CreateBlock { parent, .. } => match parent {
            BlockParent::Block(parent) => block(*parent, Access::Edit),
            BlockParent::Root | BlockParent::Detached => Rule::Member,
        },
        Op::Publish { block: of, .. }
        | Op::Move { block: of, .. }
        | Op::HoldObjects { block: of, .. }
        | Op::Append { block: of, .. } => block(*of, Access::Edit),
        // Its author, or a manager of the block (`ServerStore::redact_log`).
        Op::Redact { block: of, .. } => block(*of, Access::View),
        Op::AppendLog(record) => match record {
            LogRecord::Grant { block: of, .. } => block(*of, Access::KnowExists),
            LogRecord::Role { .. } => Rule::Administrator,
            LogRecord::Ban { block: of, .. } => block(*of, Access::Edit),
            // An administrator removes a person; anyone signs in or out an agent it made.
            LogRecord::Member { .. } => Rule::Member,
        },
        // Anyone deletes the agents it made; an administrator, every agent.
        Op::DeleteAgent { .. } => Rule::Member,
        Op::PutKeys(keys) => match keys {
            PutKeys::Rows(_) => Rule::Member,
            PutKeys::Rotate { roots, .. } => Rule::Blocks(roots.clone(), Access::Edit),
            PutKeys::Escrow { .. } => Rule::Administrator,
            PutKeys::Reseal(_) => Rule::Account,
        },
        Op::ReserveUploads(_)
        | Op::CreateAgent { .. }
        | Op::RotateAgentToken(_)
        | Op::SetPushEndpoint(_) => Rule::Member,
        Op::CreateLink {
            kind: LinkKind::Invitation(_),
            ..
        } => Rule::Administrator,
        Op::CreateLink {
            kind: LinkKind::Contact(_),
            ..
        }
        | Op::CreateOrganization { .. }
        | Op::AcceptLink { .. }
        | Op::DeleteLink { .. }
        | Op::OpenShareLink { .. }
        | Op::DeleteMessage(_)
        | Op::RevokeToken(_) => Rule::Account,
        Op::CreateShareLink { block: of, .. } => block(*of, Access::Manage),
        // Its block is the link's, which the server reads from the link.
        Op::RevokeShareLink { .. } => Rule::Member,
        // With an organization, the revocations of the grants in it go with the removal.
        Op::RemoveContact { .. } => Rule::Anywhere,
        // In the personal organization, the account's block's new epoch goes with it.
        Op::SetRecoveryKey { .. } => Rule::Anywhere,
        Op::StageRecoveryKey(_) => Rule::Account,
        Op::PublishName(_) => Rule::Anywhere,
        Op::SetMute { block: of, .. } => block(*of, Access::View),
    }
}

/// Whether a guest of the organization (guides/blocks/keys.md, "Share links") may make `op`:
/// it works in what links shared with it, and leaves, but makes no blocks, agents or links, and
/// grants nothing, so nothing it made outlives it.
fn guest_may_write(op: &Op, account: Uuid) -> bool {
    match op {
        // A guest makes only its own shell's blocks, which `ServerStore::create_block` checks.
        Op::CreateBlock { .. } => true,
        Op::CreateAgent { .. }
        | Op::RotateAgentToken(_)
        | Op::DeleteAgent { .. }
        | Op::CreateShareLink { .. }
        | Op::RevokeShareLink { .. }
        | Op::Move { .. } => false,
        Op::AppendLog(LogRecord::Grant {
            account: granted,
            access,
            ..
        }) => *granted == account && *access == Access::None,
        Op::AppendLog(_) => false,
        _ => true,
    }
}

/// Who may send to each target.
pub(crate) fn target_rule(target: &Target) -> Rule {
    match target {
        Target::Session { block, kind } => match kind {
            SessionSend::Ordered(_) | SessionSend::Claim => Rule::Session(*block, Access::Edit),
            SessionSend::Ephemeral(_) => Rule::Session(*block, Access::View),
            SessionSend::Host(_) => Rule::Session(*block, Access::Join),
        },
        Target::Devices(_) => Rule::Account,
        Target::Push { block: of, .. } => block(*of, Access::Edit),
    }
}

/// Who may stop following each topic: whoever could follow it.
pub(crate) fn topic_rule(topic: &Topic) -> Rule {
    match topic {
        Topic::Tree(_)
        | Topic::Session(_)
        | Topic::BlockLog(_)
        | Topic::Keys
        | Topic::KeyWork
        | Topic::Roots
        | Topic::Mutes => Rule::Member,
        Topic::Storage | Topic::Mailbox => Rule::Account,
    }
}

/// Whether a block's agent bound to `root` may make `query` at all, before its rule applies.
/// It may read what a terminal host needs on connecting (its organization, its keys and
/// contacts as `Peer::attach` reads them, with its mailbox, where its contacts' key changes
/// arrive, the server's account and clock, object transfers),
/// and the blocks, which `blocks::Visibility` limits to `root` and the blocks under it.
/// `be terminal-host` makes no call these refuse (the `a_terminal_host_*` tests run as one).
/// Everything that reaches past the block is refused: agents, contact links and contacts'
/// keys, members, pairing, access, escrows, a new recovery key, storage.
pub(crate) fn bound_may_read(query: &Query, follow: bool) -> bool {
    match query {
        Query::Blocks { .. }
        | Query::List { .. }
        | Query::Tree { .. }
        | Query::History(_)
        | Query::Log { .. }
        | Query::BlockLog { .. }
        | Query::LogRedactions(_)
        | Query::KeyWork
        | Query::RecoveryKey
        | Query::Organizations
        | Query::Mailbox
        | Query::DownloadUrls(_)
        | Query::ServerInfo
        | Query::Session { .. } => true,
        // The keys topic tells of keys for any block the account may open.
        Query::Keys { .. } => !follow,
        Query::Access(_)
        | Query::Names(_)
        | Query::Revocations { .. }
        | Query::SigningKeys(_)
        | Query::Members
        | Query::Invitations
        | Query::Storage
        | Query::Link(_)
        | Query::Invitation(_)
        | Query::ShareLink(_)
        | Query::ShareLinks(_)
        | Query::Mutes => false,
    }
}

/// Whether a block's agent bound to `root` may make `op`: what changes its blocks, except
/// moving `root` itself, trashing anything, or making a block anywhere else.
pub(crate) fn bound_may_write(op: &Op, root: Uuid) -> bool {
    match op {
        // Opening a block makes it first unless it exists (`Peer::ensure_type`), so making
        // `root` again goes through, to be answered that it exists.
        Op::CreateBlock { block, parent, .. } => {
            *block == root || matches!(parent, BlockParent::Block(_))
        }
        // A block it trashed would leave what it reaches, and no one else could see it to
        // restore it.
        Op::Move { block, parent, .. } => *block != root && matches!(parent, BlockParent::Block(_)),
        Op::Publish { .. }
        | Op::HoldObjects { .. }
        | Op::Append { .. }
        | Op::Redact { .. }
        | Op::ReserveUploads(_)
        | Op::DeleteMessage(_)
        | Op::RevokeToken(_) => true,
        Op::PutKeys(keys) => !matches!(keys, PutKeys::Escrow { .. }),
        Op::AppendLog(_)
        | Op::DeleteAgent { .. }
        | Op::CreateOrganization { .. }
        | Op::CreateAgent { .. }
        | Op::RotateAgentToken(_)
        | Op::CreateLink { .. }
        | Op::AcceptLink { .. }
        | Op::DeleteLink { .. }
        | Op::CreateShareLink { .. }
        | Op::OpenShareLink { .. }
        | Op::RevokeShareLink { .. }
        | Op::RemoveContact { .. }
        | Op::SetRecoveryKey { .. }
        | Op::StageRecoveryKey(_)
        | Op::PublishName(_)
        | Op::SetPushEndpoint(_)
        | Op::SetMute { .. } => false,
    }
}

/// Whether a block's agent may send to `target`: in its sessions, and pushes; an agent posts in
/// its block like anyone, though it takes no pushes, not being a person's device.
pub(crate) fn bound_may_send(target: &Target) -> bool {
    match target {
        Target::Session { .. } | Target::Push { .. } => true,
        Target::Devices(_) => false,
    }
}

fn bound_refused() -> ServerError {
    ServerError::Refused(
        ErrorCode::PermissionDenied,
        "an agent bound to one block may not do this".into(),
    )
}

/// What a signed-in connection is told of a call only a one-shot call makes: a browser
/// sign-in's step, or the read of an invitation (`http::one_shot`, limited per address).
fn one_shot_only() -> ServerError {
    ServerError::Refused(
        ErrorCode::PermissionDenied,
        "only a one-shot call, made before signing in, may do this".into(),
    )
}

fn login_disabled() -> ServerError {
    ServerError::Refused(
        ErrorCode::LoginDisabled,
        "This server is not accepting sign-ins.".into(),
    )
}

fn seconds(duration: std::time::Duration) -> u32 {
    u32::try_from(duration.as_secs()).unwrap_or(u32::MAX)
}

fn administrators_only() -> ServerError {
    ServerError::Refused(
        ErrorCode::PermissionDenied,
        "only an administrator of the organization may do this".into(),
    )
}

fn not_joined() -> ServerError {
    ServerError::Refused(
        ErrorCode::PermissionDenied,
        "join the session before speaking in it".into(),
    )
}

fn invalid(message: impl Into<String>) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

pub(crate) enum Late {
    /// The usage of the account this connection follows may have changed.
    Storage,
}

/// `error` as a call's refusal: its code, so a storage failure still tells the client to try
/// again, and only what a client is told of it (`ServerError::told`).
pub(crate) fn wire(error: ServerError) -> Error {
    Error {
        code: error.code(),
        message: error.told(),
    }
}

/// A topic a connection follows besides trees and sessions, which the hub and the session
/// registry keep.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Followed {
    Keys(Uuid),
    KeyWork(Uuid),
    Roots(Uuid),
    Mutes(Uuid),
    Storage,
    Mailbox,
}

impl Followed {
    fn of(organization: Option<Uuid>, topic: &Topic) -> Option<Self> {
        Some(match (topic, organization) {
            (Topic::Keys, Some(organization)) => Self::Keys(organization),
            (Topic::KeyWork, Some(organization)) => Self::KeyWork(organization),
            (Topic::Roots, Some(organization)) => Self::Roots(organization),
            (Topic::Mutes, Some(organization)) => Self::Mutes(organization),
            (Topic::Storage, None) => Self::Storage,
            (Topic::Mailbox, None) => Self::Mailbox,
            _ => return None,
        })
    }

    fn told(self) -> Event {
        let (organization, topic) = match self {
            Self::Keys(organization) => (Some(organization), Topic::Keys),
            Self::KeyWork(organization) => (Some(organization), Topic::KeyWork),
            Self::Roots(organization) => (Some(organization), Topic::Roots),
            Self::Mutes(organization) => (Some(organization), Topic::Mutes),
            Self::Storage => (None, Topic::Storage),
            Self::Mailbox => (None, Topic::Mailbox),
        };
        Event::TopicChanged {
            organization,
            topic,
        }
    }
}

/// The topic a read of `query` follows with `follow`; none for one that follows nothing.
fn topic_of(query: &Query) -> Option<Topic> {
    Some(match query {
        Query::Tree { entry, .. } => Topic::Tree(*entry),
        Query::Session { block, .. } => Topic::Session(*block),
        Query::BlockLog { block, .. } => Topic::BlockLog(*block),
        Query::Keys { .. } => Topic::Keys,
        Query::KeyWork => Topic::KeyWork,
        Query::List {
            listing: Listing::Children(BlockParent::Root) | Listing::Deleted | Listing::Shared,
            ..
        } => Topic::Roots,
        Query::Storage => Topic::Storage,
        Query::Mailbox => Topic::Mailbox,
        Query::Mutes => Topic::Mutes,
        _ => return None,
    })
}

/// What a connection follows and is in, and the one stream it is told of them on.
#[derive(Default)]
struct Following {
    /// The `events` stream, once the connection asked for it.
    events: Option<OpId>,
    topics: HashSet<Followed>,
    /// The live sessions the connection is in, with the build it runs in each.
    sessions: HashMap<Place, be_block_store::Hash>,
}

/// Who a call was let through as: its account, and who it is in the organization it names.
#[derive(Clone, Copy)]
struct Admitted {
    account: Option<Uuid>,
    identity: Option<Identity>,
}

impl Admitted {
    fn account(&self) -> Result<Uuid, ServerError> {
        self.account.ok_or_else(|| {
            ServerError::Refused(
                ErrorCode::NotAuthenticated,
                "sign in with the connection's greeting".into(),
            )
        })
    }

    fn identity(&self) -> Result<Identity, ServerError> {
        self.identity
            .ok_or_else(|| invalid("name the organization this is in"))
    }
}

pub(crate) struct Connection {
    pub(crate) browser_auth: Arc<browser_auth::Sessions>,
    pub(crate) store: Arc<ServerStore>,
    pub(crate) hub: Arc<WatchHub>,
    pub(crate) registry: Arc<SessionRegistry>,
    pub(crate) client: u64,
    pub(crate) account: Option<Uuid>,
    /// For a block's agent, its organization and block (`ServerStore::bound_to`).
    pub(crate) bound: Option<(Uuid, Uuid)>,
    pub(crate) token: Option<String>,
    /// The `ServerStore::session_changes` version `token` last resolved at.
    checked: Option<u64>,
    pub(crate) header_token: Option<String>,
    pub(crate) config: ServerConfig,
    /// What presigns the URLs object bytes move with (`Services::transfers`).
    pub(crate) transfers: crate::objects::Bucket,
    /// Sends pushes (`Services::pusher`).
    pub(crate) pusher: Arc<crate::push::Pusher>,
    /// Notifications each account sent this minute (`Services::notifications`).
    pub(crate) notifications: Arc<crate::address::Rate<Uuid>>,
    /// Notifications each account sent about each block this minute.
    pub(crate) block_notifications: Arc<crate::address::Rate<(Uuid, Uuid)>>,
    /// Log entries each account appended this minute (`Services::log_entries`).
    pub(crate) log_entries: Arc<crate::address::Rate<Uuid>>,
    /// The address the connection comes from; see [`crate::Services::client_address`].
    pub(crate) address: IpAddr,
    /// Set once the connection should be closed after what it has answered: it signed out.
    pub(crate) closing: bool,
    /// While a batch runs, what its ops do once it commits (`After`).
    deferred: Option<Vec<After>>,
    /// What the op being applied asks the write to leave whole, checked at its end.
    whole: Vec<(Identity, crate::keys::Whole)>,
}

pub(crate) struct Client {
    pub(crate) connection: Connection,
    pub(crate) peer: Peer,
    pub(crate) pushes: mpsc::Receiver<Push>,
    late: mpsc::UnboundedSender<Late>,
    pub(crate) lates: mpsc::UnboundedReceiver<Late>,
    outlets: Outlets<()>,
    following: Following,
    /// The storage usage last read or told, and whether a task follows the payer's.
    storage: Option<AccountStorage>,
    following_storage: bool,
}

/// What an op tells and changes in memory once what it wrote is kept: pushes, session entries,
/// connections dropped, the hub's cache of who is who. An op of one op's write does it at once,
/// as its store work is a transaction of its own; a batch's ops do theirs once the batch commits,
/// and none of them if it rolls back (`Connection::after`). The store's own signals
/// (`usage_changes`, `session_changes`) stay immediate: each only says to read again, which
/// after a rollback reads what was kept.
enum After {
    Advance {
        organization: Uuid,
        blocks: Vec<Uuid>,
        subtree: bool,
        above: Vec<Uuid>,
    },
    Roots {
        organization: Uuid,
        blocks: Vec<Uuid>,
        before: BTreeSet<Uuid>,
    },
    Sealed {
        place: Place,
        head: be_block_commit::CommitId,
        told: Option<(u64, u64)>,
        trimmed: crate::journal::Trimmed,
    },
    Moved {
        organization: Uuid,
        block: Uuid,
        parent: BlockParent,
        moved: crate::blocks::Moved,
    },
    Revoked {
        revoked: Vec<contacts::Granted>,
        accounts: Vec<Uuid>,
    },
    Recheck {
        organization: Uuid,
        account: Option<Uuid>,
        within: Option<Uuid>,
    },
    RoleChanged {
        organization: Uuid,
        account: Uuid,
        role: be_block_protocol::OrganizationRole,
    },
    DropMember {
        organization: Uuid,
        account: Uuid,
    },
    KeyWorkChanged {
        organization: Uuid,
        accounts: BTreeSet<Uuid>,
    },
    /// Every member's key work may have changed: a demotion or a removal outdates every epoch
    /// under the escrow, so every member's devices hear of it.
    KeyWorkChangedForAll {
        organization: Uuid,
    },
    KeysChanged {
        organization: Uuid,
        accounts: BTreeSet<Uuid>,
    },
    /// The accounts' mailboxes changed: a contact made or removed leaves its records there.
    MailboxChanged(Vec<Uuid>),
    Regranted {
        organization: Uuid,
        block: Uuid,
        before: BTreeSet<Uuid>,
    },
    /// Entries appended to a block's log, told to its followers who may view it.
    Logged {
        place: Place,
        told: Arc<crate::log::Told>,
    },
    /// An entry of a block's log redacted, told to its followers who may view it.
    Redacted { place: Place, seq: u64 },
    /// A push to the account's other connections.
    Tell {
        account: Uuid,
        push: Push,
    },
}

/// A write refused at the op `index`.
struct Refused {
    index: u32,
    error: ServerError,
}

impl From<ServerError> for Refused {
    fn from(error: ServerError) -> Self {
        Self { index: 0, error }
    }
}

impl Client {
    /// A client from `address`, not signed in until its greeting signs it in (`greeted`). A
    /// one-shot client is never signed in: it answers one call that needs no sign-in.
    pub(crate) async fn open(
        services: &Services,
        address: IpAddr,
        sink: impl Fn(Vec<u8>) + Send + Sync + 'static,
    ) -> Self {
        let (sender, pushes) = mpsc::channel(PUSHES);
        let client = services.hub.register(sender).await;
        let (late, lates) = mpsc::unbounded_channel();
        Self {
            connection: Connection {
                browser_auth: Arc::clone(&services.browser_auth),
                store: Arc::clone(&services.store),
                hub: Arc::clone(&services.hub),
                registry: Arc::clone(&services.registry),
                client,
                account: None,
                bound: None,
                token: None,
                checked: None,
                header_token: None,
                config: services.config.clone(),
                transfers: services.transfers.clone(),
                pusher: Arc::clone(&services.pusher),
                notifications: Arc::clone(&services.notifications),
                block_notifications: Arc::clone(&services.block_notifications),
                log_entries: Arc::clone(&services.log_entries),
                address,
                closing: false,
                deferred: None,
                whole: Vec::new(),
            },
            peer: Peer::new(be_wasm_ops::Side::Host, sink),
            pushes,
            late,
            lates,
            outlets: Outlets::within(STREAM_BUFFER_BYTES),
            following: Following::default(),
            storage: None,
            following_storage: false,
        }
    }

    pub(crate) async fn receive(&mut self, frame: &[u8]) -> Result<(), ServerError> {
        let events = self
            .peer
            .receive(frame)
            .map_err(|error| ServerError::Refused(ErrorCode::InvalidRequest, error.to_string()))?;
        self.handle(events).await;
        Ok(())
    }

    pub(crate) async fn handle(&mut self, events: Vec<Incoming>) {
        for event in events {
            match event {
                Incoming::Started(started) => self.serve(started).await,
                Incoming::Canceled { op } => {
                    if self.following.events == Some(op) {
                        self.following.events = None;
                    }
                    self.outlets
                        .end(&mut self.peer.lock(), op, Some(OpError::Canceled));
                }
                Incoming::Credited { op } => {
                    self.outlets.credited(&mut self.peer.lock(), op);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn pending_pushes(&mut self) -> Vec<Push> {
        let mut pushes = Vec::new();
        while let Ok(push) = self.pushes.try_recv() {
            pushes.push(push);
        }
        pushes
    }

    pub(crate) fn pending_lates(&mut self) -> Vec<Late> {
        let mut lates = Vec::new();
        while let Ok(late) = self.lates.try_recv() {
            lates.push(late);
        }
        lates
    }

    pub(crate) async fn late(&mut self, late: Late) {
        match late {
            Late::Storage => self.storage_changed().await,
        }
    }

    /// Tells the connection its storage changed when it follows it and the usage differs from
    /// what it last read or was told of.
    async fn storage_changed(&mut self) {
        if !self.following.topics.contains(&Followed::Storage) {
            return;
        }
        let Some(account) = self.connection.account else {
            return;
        };
        let usage = match self.connection.store.storage_usage(account).await {
            Ok(usage) => usage,
            Err(error) => {
                tracing::warn!(%error, "the storage usage could not be read");
                return;
            }
        };
        if self.storage.as_ref() == Some(&usage) {
            return;
        }
        self.storage = Some(usage);
        let _ = self.tell(Followed::Storage.told());
    }

    /// Follows the payer's usage changes for this connection, until the connection closes.
    fn follow_storage(&mut self, payer: Uuid) {
        if std::mem::replace(&mut self.following_storage, true) {
            return;
        }
        let mut changes = self.connection.store.usage_changes.subscribe();
        let late = self.late.clone();
        tokio::spawn(async move {
            loop {
                let changed = tokio::select! {
                    changed = changes.recv() => changed,
                    () = late.closed() => return,
                };
                let ours = match changed {
                    Ok(changed) => changed == payer,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => true,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                };
                if ours && late.send(Late::Storage).is_err() {
                    return;
                }
            }
        });
    }

    pub(crate) async fn close(self) {
        let Connection {
            hub,
            registry,
            client,
            account,
            bound,
            ..
        } = self.connection;
        if let (Some(account), Some(_)) = (account, bound) {
            hub.leave_live(account, client);
        }
        self.peer.close();
        registry.leave_all(client).await;
        hub.remove(client).await;
    }

    fn reply<T, E, W: be_wasm_ops::wire::Encode<Result<T, E>>>(
        &self,
        reply: Reply<Result<T, E>, W>,
        result: Result<T, E>,
    ) {
        reply.send(&mut self.peer.lock(), &result);
    }

    /// Sends `event` on the connection's events stream, when it has one.
    fn tell(&mut self, event: Event) -> Result<(), ServerError> {
        let Some(events) = self.following.events else {
            return Ok(());
        };
        self.outlets
            .send(&mut self.peer.lock(), &event, |_| true)
            .map_err(|_| {
                tracing::debug!(op = ?events, "a client is not reading its events");
                crate::refused("the client is not reading what it is sent")
            })
    }

    pub(crate) async fn push(&mut self, push: Push) -> Result<(), ServerError> {
        let session = |block: Place, event: SessionEvent| Event::Session {
            organization: block.organization,
            block: block.block,
            event,
        };
        let event = match push {
            Push::TreeAdvanced {
                organization,
                entry,
                sequence,
            } => Event::TreeAdvanced {
                organization,
                entry,
                sequence,
            },
            Push::Roots { organization } => match self.follows(Followed::Roots(organization)) {
                true => Followed::Roots(organization).told(),
                false => return Ok(()),
            },
            Push::Membership { block, entry } => {
                if !self.following.sessions.contains_key(&block) {
                    return Ok(());
                }
                // The registry tells a participant it left, and then nothing more of the session.
                if entry.client == self.connection.client
                    && matches!(entry.change, be_block_protocol::MembershipChange::Left)
                {
                    self.following.sessions.remove(&block);
                }
                session(block, SessionEvent::Membership((*entry).clone()))
            }
            Push::Sealed {
                block,
                commit,
                through,
                run,
            } => match self.following.sessions.contains_key(&block) {
                true => session(
                    block,
                    SessionEvent::Sealed {
                        commit,
                        through,
                        run,
                    },
                ),
                false => return Ok(()),
            },
            Push::Relayed {
                block,
                from,
                payload,
                received,
            } => match self.following.sessions.contains_key(&block) {
                true => session(
                    block,
                    SessionEvent::Relayed {
                        from,
                        payload: payload.to_vec(),
                        received,
                    },
                ),
                false => return Ok(()),
            },
            Push::Hosted {
                block,
                from,
                payload,
                received,
            } => match self.following.sessions.contains_key(&block) {
                true => session(
                    block,
                    SessionEvent::Hosted {
                        from,
                        payload: payload.to_vec(),
                        received,
                    },
                ),
                false => return Ok(()),
            },
            Push::Host { block, host } => match self.following.sessions.contains_key(&block) {
                true => session(block, SessionEvent::Host(host)),
                false => return Ok(()),
            },
            Push::Joiner { block, change } => match self.following.sessions.contains_key(&block) {
                true => session(block, SessionEvent::Joiner(change)),
                false => return Ok(()),
            },
            Push::Sequenced { block, entry } => {
                match self.following.sessions.contains_key(&block) {
                    true => session(block, SessionEvent::Sequenced((*entry).clone())),
                    false => return Ok(()),
                }
            }
            Push::Paired(message) => Event::Paired(message),
            Push::Redacted { block, seq } => Event::Redacted(be_block_protocol::Redacted {
                organization: block.organization,
                block: block.block,
                seq,
            }),
            Push::Logged { block, told } => Event::Appended(be_block_protocol::Appended {
                organization: block.organization,
                block: block.block,
                previous: told.previous,
                head: told.head,
                entries: told.entries.clone(),
                proof: told.proof.clone(),
                consistency: told.consistency.clone(),
            }),
            Push::MailboxChanged => match self.follows(Followed::Mailbox) {
                true => Followed::Mailbox.told(),
                false => return Ok(()),
            },
            Push::KeyWorkChanged { organization } => {
                match self.follows(Followed::KeyWork(organization)) {
                    true => Followed::KeyWork(organization).told(),
                    false => return Ok(()),
                }
            }
            Push::KeysChanged { organization } => {
                match self.follows(Followed::Keys(organization)) {
                    true => Followed::Keys(organization).told(),
                    false => return Ok(()),
                }
            }
            Push::MutesChanged { organization } => {
                match self.follows(Followed::Mutes(organization)) {
                    true => Followed::Mutes(organization).told(),
                    false => return Ok(()),
                }
            }
        };
        self.tell(event)
    }

    /// Sends on what the hub queued for this connection so far.
    async fn drain_pushes(&mut self) {
        for push in self.pending_pushes() {
            if let Err(error) = self.push(push).await {
                tracing::debug!(%error, "a push could not be told");
            }
        }
    }

    fn follows(&self, followed: Followed) -> bool {
        self.following.topics.contains(&followed)
    }

    async fn prelude(&mut self) -> Result<(), ServerError> {
        self.connection.signed_in().await
    }

    async fn serve(&mut self, started: Started) {
        use embed::server::Call;
        let Some(embed::Call::Server(call)) = embed::Call::accept(&mut self.peer.lock(), started)
        else {
            return;
        };
        let call = match call {
            Call::Ping { reply } => return reply.send(&mut self.peer.lock(), &()),
            call => call,
        };
        let prelude = self.prelude().await;
        match call {
            Call::Ping { .. } => {}
            Call::DeviceLogin { reply, step } => {
                let answered = match prelude {
                    Ok(()) if self.connection.account.is_some() => Err(one_shot_only()),
                    Ok(()) => self.device_login(step).await,
                    Err(error) => Err(error),
                };
                self.reply(reply, answered.map_err(wire));
            }
            Call::Read {
                reply,
                organization,
                query,
                follow,
            } => {
                let answered = match prelude {
                    Ok(()) => self.read(organization, query, follow).await,
                    Err(error) => Err(error),
                };
                self.reply(reply, answered.map_err(wire));
            }
            Call::Unfollow {
                reply,
                organization,
                topic,
            } => {
                let unfollowed = match prelude {
                    Ok(()) => self.unfollow(organization, topic).await,
                    Err(error) => Err(error),
                };
                self.reply(reply, unfollowed.map_err(wire));
            }
            Call::Write {
                reply,
                organization,
                ops,
            } => {
                let written = match prelude {
                    Ok(()) => self.write(organization, ops).await,
                    Err(error) => Err(Refused { index: 0, error }),
                };
                self.reply(
                    reply,
                    written.map_err(|refused| WriteError {
                        index: refused.index,
                        error: wire(refused.error),
                    }),
                );
            }
            Call::Send {
                reply,
                organization,
                target,
                payload,
            } => {
                let sent = match prelude {
                    Ok(()) => self.send(organization, target, payload).await,
                    Err(error) => Err(error),
                };
                self.reply(reply, sent.map_err(wire));
            }
            Call::Events { stream } => {
                if prelude.is_err() || self.connection.account.is_none() {
                    return stream.fail(&mut self.peer.lock(), OpError::Denied);
                }
                let mut lock = self.peer.lock();
                if let Some(before) = self.following.events.take() {
                    self.outlets.end(&mut lock, before, Some(OpError::Canceled));
                }
                match self.outlets.len() < MAX_STREAMS_PER_CONNECTION {
                    true => {
                        self.following.events = Some(stream.op());
                        self.outlets
                            .insert(stream, (), Backlog::Queue(STREAM_BUFFER));
                    }
                    false => stream.fail(&mut lock, OpError::Denied),
                }
            }
        }
    }

    /// Applies `rule` to a call naming `organization`, after refusing what a block's agent may
    /// not do at all (`bound`, whether it may) and everything while its block is in the trash.
    async fn admit(
        &self,
        organization: Option<Uuid>,
        rule: Rule,
        bound: bool,
    ) -> Result<Admitted, ServerError> {
        let connection = &self.connection;
        if let Some((of, root)) = connection.bound {
            if !bound {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "an agent bound to one block may not make this call with these arguments"
                        .into(),
                ));
            }
            connection.store.require_bound_live(of, root).await?;
        }
        let account = connection.account;
        let none = |admitted: Admitted| match organization {
            Some(_) => Err(invalid("this is the account's own: name no organization")),
            None => Ok(admitted),
        };
        let member = || async {
            let Some(organization) = organization else {
                return Err(invalid("name the organization this is in"));
            };
            connection.identity_in(organization).await
        };
        let admitted = |identity: Identity| Admitted {
            account,
            identity: Some(identity),
        };
        match rule {
            Rule::OneShot => {
                if account.is_some() {
                    return Err(one_shot_only());
                }
                none(Admitted {
                    account,
                    identity: None,
                })
            }
            Rule::Account => {
                connection.account()?;
                none(Admitted {
                    account,
                    identity: None,
                })
            }
            Rule::Anywhere => {
                connection.account()?;
                Ok(Admitted {
                    account,
                    identity: match organization {
                        Some(organization) => Some(connection.identity_in(organization).await?),
                        None => None,
                    },
                })
            }
            Rule::Member => Ok(admitted(member().await?)),
            Rule::Administrator => {
                let identity = member().await?;
                match identity.is_administrator() {
                    true => Ok(admitted(identity)),
                    false => Err(administrators_only()),
                }
            }
            Rule::Blocks(blocks, level) => {
                let identity = member().await?;
                connection
                    .store
                    .require_access(identity, blocks, level)
                    .await?;
                Ok(admitted(identity))
            }
            Rule::Session(block, level) => {
                let identity = member().await?;
                let place = Place {
                    organization: identity.organization,
                    block,
                };
                if !self.following.sessions.contains_key(&place) {
                    return Err(not_joined());
                }
                connection
                    .store
                    .require_access(identity, vec![block], level)
                    .await?;
                Ok(admitted(identity))
            }
        }
    }

    async fn device_login(
        &mut self,
        step: DeviceLoginStep,
    ) -> Result<DeviceLoginAnswer, ServerError> {
        let connection = &self.connection;
        if !connection.config.allow_login {
            return Err(login_disabled());
        }
        match step {
            DeviceLoginStep::Begin => {
                let (code, secret, interval, expires) =
                    connection.browser_auth.begin(connection.address);
                Ok(DeviceLoginAnswer::Begun(BrowserLogin {
                    code,
                    secret,
                    interval: seconds(interval),
                    expires: seconds(expires),
                }))
            }
            DeviceLoginStep::Poll { code, secret } => Ok(DeviceLoginAnswer::Polled(
                match connection.browser_auth.poll(&code, &secret)? {
                    browser_auth::Polled::Pending => BrowserLoginState::Pending,
                    browser_auth::Polled::Denied => BrowserLoginState::Denied,
                    browser_auth::Polled::Expired => BrowserLoginState::Expired,
                    browser_auth::Polled::Approved { account, issuer } => {
                        let token = connection
                            .store
                            .claim_browser_session(account, &issuer)
                            .await?;
                        let profile = connection.store.resolve_token(&token).await?;
                        BrowserLoginState::Approved(Authenticated {
                            account: profile.account,
                            email: profile.email,
                            display_name: profile.display_name,
                            token,
                            client: 0,
                        })
                    }
                },
            )),
        }
    }

    /// Signs the connection in with its greeting's token, or with the request's Authorization
    /// header when the greeting carries none: what the server's greeting answers. A connection
    /// it does not sign in is closed before any call.
    pub(crate) async fn greeted(&mut self, token: Option<String>) -> be_block_protocol::Welcome {
        self.sign_in(token).await.map_err(wire)
    }

    async fn sign_in(&mut self, token: Option<String>) -> Result<Authenticated, ServerError> {
        let header = token.is_none();
        let token = token
            .or_else(|| self.connection.header_token.clone())
            .ok_or_else(|| {
                ServerError::Refused(
                    ErrorCode::NotAuthenticated,
                    "no Bearer account token was supplied in Authorization".into(),
                )
            })?;
        let profile = self.connection.store.resolve_token(&token).await?;
        let mut authenticated = self.connection.authenticated(profile, token).await?;
        if header {
            authenticated.token.clear();
        }
        Ok(authenticated)
    }
}

impl Client {
    async fn read(
        &mut self,
        organization: Option<Uuid>,
        query: Query,
        follow: bool,
    ) -> Result<Page, ServerError> {
        let bound = bound_may_read(&query, follow);
        let admitted = self.admit(organization, query_rule(&query), bound).await?;
        let account = admitted.account().ok();
        if !follow {
            let page = self.answer(admitted, query).await?;
            return Ok(self.lied(account, page));
        }
        let topic = topic_of(&query).ok_or_else(|| invalid("this query has nothing to follow"))?;
        match topic {
            // Followed by its own handlers, which follow before they read.
            Topic::Tree(_) | Topic::Session(_) | Topic::BlockLog(_) => {}
            topic => {
                let followed = Followed::of(organization, &topic)
                    .ok_or_else(|| invalid("this topic is followed in another organization"))?;
                self.following.topics.insert(followed);
            }
        }
        let page = self.answer_following(admitted, query).await?;
        Ok(self.lied(account, page))
    }

    /// `page` as a test's lying server sends it to `account` (`ServerConfig::lies`).
    fn lied(&self, account: Option<Uuid>, mut page: Page) -> Page {
        if let (Some(lies), Some(account)) = (&self.connection.config.lies, account) {
            lies.apply(account, &mut page);
        }
        page
    }

    /// Answers a read that follows its topic: a tree is followed in the hub before it is read,
    /// and a session joined.
    async fn answer_following(
        &mut self,
        admitted: Admitted,
        query: Query,
    ) -> Result<Page, ServerError> {
        match query {
            Query::Tree {
                entry,
                after,
                limit,
            } => {
                let identity = admitted.identity()?;
                let connection = &self.connection;
                let place = Place {
                    organization: identity.organization,
                    block: entry,
                };
                // Followed before it is read, so nothing changed during the read is missed.
                let new = connection.hub.follow(connection.client, place).await?;
                let page = connection
                    .store
                    .changed_since(identity, entry, after, limit)
                    .await;
                match &page {
                    Ok(page) if page.next.is_none() => {
                        connection
                            .hub
                            .read(connection.client, place, page.sequence)
                            .await;
                    }
                    Ok(_) => {}
                    // A follow this read began goes with it; one from before is told the next
                    // change again.
                    Err(_) if new => connection.hub.unfollow(connection.client, place).await,
                    Err(_) => connection.hub.unread(connection.client, place).await,
                }
                page.map(Page::Tree)
            }
            Query::Session { block, tag } => self.join(admitted.identity()?, block, tag).await,
            Query::BlockLog {
                block,
                from,
                limit,
                known,
                redactions_after,
            } => {
                let identity = admitted.identity()?;
                let connection = &self.connection;
                let place = Place {
                    organization: identity.organization,
                    block,
                };
                // Followed before it is read, so no append during the read is missed.
                connection.hub.follow_log(connection.client, place).await?;
                let page = connection
                    .store
                    .read_log(identity, block, from, limit, known, redactions_after)
                    .await;
                if page.is_err() {
                    connection.hub.unfollow_log(connection.client, place).await;
                }
                page.map(Page::BlockLog)
            }
            Query::Storage => {
                let account = admitted.account()?;
                let payer = self.connection.store.payer(account).await?;
                self.follow_storage(payer);
                let usage = self.connection.store.storage_usage(account).await?;
                self.storage = Some(usage.clone());
                Ok(Page::Storage(usage))
            }
            query => self.answer(admitted, query).await,
        }
    }

    /// Joins a block's live session: answers the membership as of the number it joins at,
    /// itself in it, and the block's journal; what the order numbers after reaches the
    /// connection's events. Joining one it is in leaves it first.
    async fn join(
        &mut self,
        identity: Identity,
        block: Uuid,
        tag: be_block_store::Hash,
    ) -> Result<Page, ServerError> {
        let place = Place {
            organization: identity.organization,
            block,
        };
        let connection = &self.connection;
        if self.following.sessions.remove(&place).is_some() {
            connection.registry.leave(place, connection.client).await;
        } else if self.following.sessions.len() >= MAX_SESSIONS_PER_CONNECTION {
            return Err(crate::limits::too_many(
                "sessions one connection is in; leave some first",
                MAX_SESSIONS_PER_CONNECTION,
            ));
        }
        // What the hub queued for this connection before it joins (its own leave of an earlier
        // join, entries of that membership) is told or dropped now, as of before the join, so
        // none of it reaches the connection as if of the session it joins.
        self.drain_pushes().await;
        let connection = &self.connection;
        // A ban binds anyone below Edit (an editor may change the block directly anyway).
        let barred = || async {
            let editor = connection
                .store
                .require_access(identity, vec![block], Access::Edit)
                .await
                .is_ok();
            // A ban that cannot be read bars: it fails closed.
            Ok::<_, ServerError>(
                !editor
                    && connection
                        .store
                        .banned(identity, block)
                        .await
                        .unwrap_or(true),
            )
        };
        if barred().await? {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                format!("this account is barred from joining block {block}'s host"),
            ));
        }
        // Below View, a Join member (the rule admitted Join): it hears who hosts and the host
        // channel, nothing of the order, and only the lease holder hears of it.
        if connection
            .store
            .require_access(identity, vec![block], Access::View)
            .await
            .is_err()
        {
            self.following.sessions.insert(place, tag);
            let connection = &self.connection;
            let host = connection
                .registry
                .join_member(place, connection.client, identity.account)
                .await;
            // A Join lost, or a ban made, since the checks ran leaves again.
            let mut still = connection
                .store
                .require_access(identity, vec![block], Access::Join)
                .await;
            if still.is_ok()
                && connection
                    .store
                    .banned(identity, block)
                    .await
                    .unwrap_or(true)
            {
                still = Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    format!("this account is barred from joining block {block}'s host"),
                ));
            }
            if let Err(error) = still {
                connection.registry.leave(place, connection.client).await;
                self.following.sessions.remove(&place);
                return Err(error);
            }
            return Ok(Page::Session(JoinedSession {
                client: connection.client,
                state: SessionState::default(),
                journal: None,
                may_edit: false,
                joined_as: MemberKind::Join,
                host,
            }));
        }
        let may_edit = connection
            .store
            .require_access(identity, vec![block], Access::Edit)
            .await
            .is_ok();
        self.following.sessions.insert(place, tag);
        let connection = &self.connection;
        let (state, host) = connection
            .registry
            .join(place, connection.client, identity.account, may_edit, tag)
            .await;
        // A view lost, or a ban made against one below Edit, since the checks ran leaves the
        // session again.
        let journal = async {
            connection
                .store
                .require_access(identity, vec![block], Access::View)
                .await?;
            if barred().await? {
                return Err(ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    format!("this account is barred from joining block {block}'s host"),
                ));
            }
            connection
                .store
                .read_journal(identity.organization, block)
                .await
        }
        .await;
        match journal {
            Ok(journal) => Ok(Page::Session(JoinedSession {
                client: connection.client,
                state,
                journal,
                may_edit,
                joined_as: MemberKind::Participant,
                host,
            })),
            Err(error) => {
                connection.registry.leave(place, connection.client).await;
                self.following.sessions.remove(&place);
                Err(error)
            }
        }
    }

    /// Answers a read that follows nothing.
    async fn answer(&mut self, admitted: Admitted, query: Query) -> Result<Page, ServerError> {
        let connection = &self.connection;
        let store = &connection.store;
        Ok(match query {
            Query::Blocks {
                blocks,
                contents: false,
                ..
            } => {
                if blocks.len() > be_block_protocol::MAX_SUMMARIES_ASKED {
                    return Err(invalid(format!(
                        "ask for at most {} summaries at a time",
                        be_block_protocol::MAX_SUMMARIES_ASKED
                    )));
                }
                Page::Summaries(
                    store
                        .read_summaries(admitted.identity()?, blocks, SUMMARY_BYTES)
                        .await?,
                )
            }
            Query::Blocks {
                blocks,
                contents: true,
                held,
            } => {
                let identity = admitted.identity()?;
                let (blocks, hashes) = store
                    .read_contents(identity, blocks, held, SUMMARY_BYTES)
                    .await?;
                let (objects_room, journals_room) =
                    contents_room(be_block_protocol::encoded_len(&blocks));
                let objects = store.get_objects(hashes, objects_room).await?;
                let journals = store
                    .read_journals(
                        identity.organization,
                        blocks.iter().map(|summary| summary.id).collect(),
                        journals_room,
                    )
                    .await?;
                Page::Contents(Contents {
                    blocks,
                    objects,
                    journals,
                })
            }
            Query::List { listing, after } => Page::Listed(
                store
                    .list_page(admitted.identity()?, listing, after)
                    .await?,
            ),
            Query::Tree {
                entry,
                after,
                limit,
            } => {
                let identity = admitted.identity()?;
                let page = store.changed_since(identity, entry, after, limit).await?;
                // A read to the end of a tree the connection follows lets the next change be
                // told, whether or not this read follows it (`event.tree-advanced`).
                if page.next.is_none() {
                    let place = Place {
                        organization: identity.organization,
                        block: entry,
                    };
                    let connection = &self.connection;
                    connection
                        .hub
                        .read(connection.client, place, page.sequence)
                        .await;
                }
                Page::Tree(page)
            }
            Query::History(block) => {
                Page::History(store.list_history(admitted.identity()?, block).await?)
            }
            Query::BlockLog {
                block,
                from,
                limit,
                known,
                redactions_after,
            } => Page::BlockLog(
                store
                    .read_log(
                        admitted.identity()?,
                        block,
                        from,
                        limit,
                        known,
                        redactions_after,
                    )
                    .await?,
            ),
            Query::Log {
                log: Some(block),
                held,
            } => {
                let identity = admitted.identity()?;
                // A reader below Edit checks a lease holder's right to host before it speaks to
                // it (plans/hosted_blocks.md), in the block's log or one above it: the path proves
                // the records of the holder of each session this connection is in, whose host it
                // was told, where that session's block is this one or lies below it
                // (`ServerStore::access_log`).
                let mut hosts = Vec::new();
                for place in self.following.sessions.keys() {
                    if place.organization == identity.organization
                        && let Some(host) = connection.registry.host(*place).await
                    {
                        hosts.push((place.block, host.account));
                    }
                }
                Page::Log(store.access_log(identity, block, hosts, held).await?)
            }
            Query::Log { log: None, held } => {
                Page::Log(store.organization_log(admitted.identity()?, held).await?)
            }
            Query::LogRedactions(asked) => Page::LogRedactions(
                store
                    .log_redactions(admitted.identity()?, asked)
                    .await?,
            ),
            Query::Access(block) => {
                let identity = admitted.identity()?;
                let mut list = store.list_access(identity, block).await?;
                // Who hosts the block now: the account whose device holds its session's lease.
                list.host = connection
                    .registry
                    .host(crate::watch::Place {
                        organization: identity.organization,
                        block,
                    })
                    .await
                    .map(|host| host.account);
                Page::Access(list)
            }
            Query::Names(block) => {
                Page::Names(store.list_names(admitted.identity()?, block).await?)
            }
            Query::Keys { blocks, after } => {
                Page::Keys(store.get_keys(admitted.identity()?, blocks, after).await?)
            }
            Query::KeyWork => Page::KeyWork(store.key_work(admitted.identity()?).await?),
            Query::Revocations { account, contact } => {
                let identity = admitted.identity()?;
                let pairs = match contact {
                    true => {
                        store
                            .contact_revocations(identity.account, account, identity.organization)
                            .await?
                    }
                    false => {
                        store
                            .member_revocations(identity.organization, account)
                            .await?
                    }
                };
                Page::Revocations(
                    pairs
                        .into_iter()
                        .map(|(block, account)| be_block_protocol::Revocation { block, account })
                        .collect(),
                )
            }
            Query::RecoveryKey => {
                let account = admitted.account()?;
                let (public, sealed) = store.recovery_key(account).await?;
                let logs = store.signed_logs(account).await?;
                Page::RecoveryKey(RecoveryKey {
                    public,
                    sealed,
                    logs,
                })
            }
            Query::SigningKeys(accounts) => {
                Page::SigningKeys(store.signing_keys(admitted.identity()?, accounts).await?)
            }
            Query::Members => Page::Members(store.list_members(admitted.identity()?).await?),
            Query::Organizations => {
                Page::Organizations(store.list_organizations(admitted.account()?).await?)
            }
            Query::Invitations => {
                let identity = admitted.identity()?;
                Page::Invitations(store.list_invitations(identity.organization).await?)
            }
            Query::Storage => {
                let usage = store.storage_usage(admitted.account()?).await?;
                Page::Storage(usage)
            }
            Query::Mailbox => Page::Mailbox(store.mailbox(admitted.account()?).await?),
            Query::Link(link) => Page::Link(store.link(admitted.account()?, link).await?),
            Query::ShareLink(link) => {
                Page::ShareLink(store.share_link(admitted.account()?, link).await?)
            }
            Query::ShareLinks(block) => {
                Page::ShareLinks(store.share_links(admitted.identity()?, block).await?)
            }
            Query::Invitation(link) => Page::Invitation(store.read_invitation(link).await?),
            Query::DownloadUrls(hashes) => {
                let identity = admitted.identity()?;
                at_most_urls(hashes.len())?;
                let lifetime = store.transfer_lifetime();
                let readable = store.may_read_objects(identity, hashes.clone()).await?;
                Page::DownloadUrls(
                    hashes
                        .into_iter()
                        .zip(readable)
                        .map(|(hash, readable)| {
                            readable.then(|| connection.transfers.presign_get(hash, lifetime))
                        })
                        .collect(),
                )
            }
            Query::ServerInfo => Page::ServerInfo(ServerInfo {
                server: store.server_account(),
                push_key: connection.pusher.public_key(),
                offers_fcm: connection.pusher.offers_fcm(),
                time: be_block_commit::now_milliseconds(),
            }),
            Query::Mutes => Page::Mutes(store.mutes(admitted.identity()?).await?),
            Query::Session { .. } => {
                return Err(invalid("a session is joined with a read that follows it"));
            }
        })
    }

    async fn unfollow(
        &mut self,
        organization: Option<Uuid>,
        topic: Topic,
    ) -> Result<(), ServerError> {
        let admitted = self.admit(organization, topic_rule(&topic), true).await?;
        let connection = &self.connection;
        match topic {
            Topic::Tree(entry) => {
                let identity = admitted.identity()?;
                let place = Place {
                    organization: identity.organization,
                    block: entry,
                };
                connection.hub.unfollow(connection.client, place).await;
            }
            Topic::BlockLog(block) => {
                let identity = admitted.identity()?;
                let place = Place {
                    organization: identity.organization,
                    block,
                };
                connection.hub.unfollow_log(connection.client, place).await;
            }
            Topic::Session(block) => {
                let identity = admitted.identity()?;
                let place = Place {
                    organization: identity.organization,
                    block,
                };
                if self.following.sessions.remove(&place).is_some() {
                    connection.registry.leave(place, connection.client).await;
                    // Its own leave is dropped now, before a later join of the same block.
                    self.drain_pushes().await;
                }
            }
            topic => {
                if let Some(followed) = Followed::of(organization, &topic) {
                    self.following.topics.remove(&followed);
                }
            }
        }
        Ok(())
    }

    /// Applies a write's ops in order. More than one, or one that makes or moves a block, is
    /// applied in one store transaction (`ServerStore::batch`), so one refused keeps none of
    /// them; each op's rule is applied as it comes, so it sees what the ops before it did, and
    /// what must hold over them all (`keys::Whole`) is checked once they are all applied. The caller is admitted by its role as of
    /// the write's start (the hub takes a role change only once the write commits): safe, as an
    /// op never raises its caller's own role, and the store's checks inside the transaction see
    /// what the write's own ops wrote.
    async fn write(
        &mut self,
        organization: Option<Uuid>,
        ops: Vec<Op>,
    ) -> Result<Vec<Done>, Refused> {
        // A write let go of before it ended (an HTTP post's route dropped) left its batch's
        // effects behind: it rolled back, so they are dropped, and this write starts afresh.
        self.connection.deferred = None;
        if ops.len() > 1 && ops.iter().any(|op| matches!(op, Op::RevokeToken(_))) {
            return Err(Refused {
                index: 0,
                error: invalid("signing out is a write of its own"),
            });
        }
        if ops.len() > crate::limits::MAX_OPS_PER_WRITE {
            return Err(Refused {
                index: 0,
                error: crate::limits::too_many(
                    "ops in one write",
                    crate::limits::MAX_OPS_PER_WRITE,
                ),
            });
        }
        // A write that makes or moves a block is checked whole at its end, so it is one
        // transaction even of one op.
        let checked = ops
            .iter()
            .any(|op| matches!(op, Op::CreateBlock { .. } | Op::Move { .. }));
        if ops.len() < 2 && !checked {
            let mut done = Vec::with_capacity(1);
            for op in ops {
                done.push(
                    self.apply(organization, op)
                        .await
                        .map_err(|error| Refused { index: 0, error })?,
                );
            }
            return Ok(done);
        }
        let store = Arc::clone(&self.connection.store);
        self.connection.deferred = Some(Vec::new());
        let written = store
            .batch(async {
                let mut done = Vec::with_capacity(ops.len());
                let mut whole = Vec::new();
                for (index, op) in ops.into_iter().enumerate() {
                    let index = u32::try_from(index).unwrap_or(u32::MAX);
                    self.connection.whole.clear();
                    done.push(
                        self.apply(organization, op)
                            .await
                            .map_err(|error| Refused { index, error })?,
                    );
                    whole.extend(
                        self.connection
                            .whole
                            .drain(..)
                            .map(|(identity, check)| (identity, index, check)),
                    );
                }
                for (identity, index, check) in whole {
                    store
                        .check_whole(identity, check)
                        .await
                        .map_err(|error| Refused { index, error })?;
                }
                Ok(done)
            })
            .await;
        // Only what was kept is told or changed in memory.
        let deferred = self.connection.deferred.take().unwrap_or_default();
        if written.is_ok() {
            for after in deferred {
                self.connection.run(after).await;
            }
        }
        written
    }

    async fn apply(&mut self, organization: Option<Uuid>, op: Op) -> Result<Done, ServerError> {
        let bound = self
            .connection
            .bound
            .is_none_or(|(_, root)| bound_may_write(&op, root));
        let admitted = self.admit(organization, op_rule(&op), bound).await?;
        if let Some(identity) = admitted.identity
            && identity.role == be_block_protocol::OrganizationRole::Guest
            && !guest_may_write(&op, identity.account)
        {
            return Err(ServerError::Refused(
                ErrorCode::PermissionDenied,
                "a guest of this organization holds only what links shared with it: it makes no blocks and grants nothing".into(),
            ));
        }
        let connection = &mut self.connection;
        match op {
            Op::CreateBlock {
                block,
                content_type,
                parent,
                metadata,
                references,
                key,
                owner_only,
                root,
                placement,
            } => {
                let identity = admitted.identity()?;
                let made = block;
                // The blocks it references are known to exist by whoever views it: they changed.
                let referenced = references.clone();
                let block = connection
                    .store
                    .create_block(
                        identity,
                        block,
                        content_type,
                        parent,
                        metadata,
                        references,
                        key,
                        owner_only,
                        root,
                        placement,
                    )
                    .await?;
                connection
                    .whole
                    .push((identity, crate::keys::Whole::Made { block: made, key }));
                let mut changed = vec![block.id];
                changed.extend(referenced);
                connection
                    .after(After::Advance {
                        organization: identity.organization,
                        blocks: changed,
                        subtree: false,
                        above: Vec::new(),
                    })
                    .await;
                if matches!(parent, BlockParent::Root | BlockParent::Detached) {
                    connection
                        .after(After::Roots {
                            organization: identity.organization,
                            blocks: vec![block.id],
                            before: BTreeSet::new(),
                        })
                        .await;
                }
                Ok(Done::Block(block))
            }
            Op::Publish {
                block,
                expected,
                commit: None,
                references,
                metadata,
                journal,
            } => {
                let identity = admitted.identity()?;
                if journal.is_some() {
                    return Err(invalid("a journal's seal is published with a commit"));
                }
                let described = connection
                    .store
                    .describe(identity, block, expected, metadata, references)
                    .await?;
                let (block, relinked) = match described {
                    Ok(described) => described,
                    Err(head) => {
                        return Ok(Done::Published(
                            be_block_protocol::PublishOutcome::Rejected(head),
                        ));
                    }
                };
                let mut changed = vec![block.id];
                changed.extend(relinked);
                connection
                    .after(After::Advance {
                        organization: identity.organization,
                        blocks: changed,
                        subtree: false,
                        above: Vec::new(),
                    })
                    .await;
                Ok(Done::Block(block))
            }
            Op::Publish {
                block,
                expected,
                commit:
                    Some(be_block_protocol::NewCommit {
                        id: commit,
                        parents,
                        held,
                        time,
                        pinned,
                        objects,
                        log_through,
                    }),
                references,
                metadata,
                journal,
            } => {
                let identity = admitted.identity()?;
                let place = Place {
                    organization: identity.organization,
                    block,
                };
                // A seal trims as it wins the head, from whichever connection sends it (an
                // outbox drains before its replica joins again); a trim with no new commit
                // only while the caller holds the lease, which the store checks with the
                // registry held. Told only when the head moved: a trim with no new commit is no
                // news of which head holds which number.
                let told = journal
                    .as_ref()
                    .filter(|_| expected != Some(commit))
                    .map(|seal| (seal.base.sequence, seal.run));
                let trim = journal.map(|seal| {
                    let (registry, client) = (Arc::clone(&connection.registry), connection.client);
                    crate::journal::Asked {
                        tag: seal.tag,
                        run: seal.run,
                        base: seal.base,
                        holding: Box::new(move || registry.holding(place, client)),
                    }
                });
                connection.store.put_objects(identity, objects).await?;
                let outcome = connection
                    .store
                    .publish(
                        identity,
                        block,
                        commit,
                        expected,
                        parents,
                        held,
                        time,
                        pinned,
                        references,
                        metadata,
                        trim,
                        log_through,
                    )
                    .await?;
                let (outcome, trimmed) = match outcome {
                    PublishOutcome::Published(head, relinked, trimmed) => {
                        (PublishOutcome::Published(head, relinked, None), trimmed)
                    }
                    rejected => (rejected, None),
                };
                if let (PublishOutcome::Published(head, ..), Some(trimmed)) = (&outcome, trimmed) {
                    connection
                        .after(After::Sealed {
                            place,
                            head: *head,
                            told,
                            trimmed,
                        })
                        .await;
                }
                // Its tree advanced, and so did those of the blocks it started or stopped
                // referencing; a publish of the head on itself (a trim with no new commit)
                // moved nothing.
                if let PublishOutcome::Published(_, relinked, _) = &outcome
                    && expected != Some(commit)
                {
                    let mut changed = vec![block];
                    changed.extend(relinked.iter().copied());
                    connection
                        .after(After::Advance {
                            organization: identity.organization,
                            blocks: changed,
                            subtree: false,
                            above: Vec::new(),
                        })
                        .await;
                }
                Ok(Done::Published(match outcome {
                    PublishOutcome::Published(head, ..) => {
                        be_block_protocol::PublishOutcome::Published(head)
                    }
                    PublishOutcome::Rejected(head) => {
                        be_block_protocol::PublishOutcome::Rejected(head)
                    }
                }))
            }
            Op::Move {
                block,
                parent,
                placement,
            } => {
                let identity = admitted.identity()?;
                let moved = connection
                    .store
                    .set_parent(identity, block, parent, placement)
                    .await?;
                connection.whole.push((
                    identity,
                    crate::keys::Whole::Moved {
                        block,
                        scope_before: moved.scope_before,
                    },
                ));
                connection
                    .after(After::Moved {
                        organization: identity.organization,
                        block,
                        parent,
                        moved,
                    })
                    .await;
                Ok(Done::Nothing)
            }
            Op::AppendLog(record) => {
                let identity = admitted.identity()?;
                match record {
                    LogRecord::Grant {
                        block,
                        account,
                        access,
                        signed,
                    } => {
                        let changed = connection
                            .store
                            .set_access(identity, block, account, access, signed)
                            .await?;
                        if let Some(before) = changed {
                            connection
                                .after(After::Revoked {
                                    revoked: vec![contacts::Granted {
                                        organization: identity.organization,
                                        block,
                                        before,
                                    }],
                                    accounts: vec![account],
                                })
                                .await;
                        }
                        // A grant can give edit access as well as take it.
                        connection
                            .after(After::Recheck {
                                organization: identity.organization,
                                account: None,
                                within: Some(block),
                            })
                            .await;
                    }
                    LogRecord::Ban {
                        block,
                        account,
                        signed,
                    } => {
                        connection
                            .store
                            .ban(identity, block, account, signed)
                            .await?;
                        // A banned Join member in the session leaves.
                        connection
                            .after(After::Recheck {
                                organization: identity.organization,
                                account: Some(account),
                                within: Some(block),
                            })
                            .await;
                    }
                    LogRecord::Role {
                        account,
                        role,
                        signed,
                    } => {
                        connection
                            .store
                            .set_role(identity, account, role, signed)
                            .await?;
                        connection
                            .after(After::RoleChanged {
                                organization: identity.organization,
                                account,
                                role,
                            })
                            .await;
                    }
                    LogRecord::Member {
                        signed,
                        revocations,
                    } => {
                        let account = be_block_keys::Entry::decode(&signed).ok().and_then(
                            |entry| match entry.record {
                                be_block_keys::Record::Member(named) => {
                                    Some(Uuid::from_bytes(named.account))
                                }
                                _ => None,
                            },
                        );
                        match connection
                            .store
                            .append_member(identity, signed, revocations)
                            .await?
                        {
                            Some(revoked) => {
                                let account = account.ok_or(ServerError::Corrupt)?;
                                connection
                                    .removed(identity.organization, account, revoked)
                                    .await;
                            }
                            None => {
                                connection
                                    .after(After::KeyWorkChanged {
                                        organization: identity.organization,
                                        accounts: BTreeSet::new(),
                                    })
                                    .await;
                            }
                        }
                    }
                }
                Ok(Done::Nothing)
            }
            Op::DeleteAgent {
                account,
                signed,
                revocations,
            } => {
                let identity = admitted.identity()?;
                let revoked = connection
                    .store
                    .delete_agent(identity, account, signed, revocations)
                    .await?;
                connection
                    .removed(identity.organization, account, revoked)
                    .await;
                Ok(Done::Nothing)
            }
            Op::PutKeys(keys) => {
                match keys {
                    PutKeys::Rows(keys) => {
                        let identity = admitted.identity()?;
                        if let Some(told) = connection.store.put_keys(identity, keys).await? {
                            connection
                                .after(After::KeysChanged {
                                    organization: identity.organization,
                                    accounts: told,
                                })
                                .await;
                        }
                    }
                    PutKeys::Rotate { roots, keys } => {
                        let identity = admitted.identity()?;
                        connection
                            .store
                            .rotate(identity, roots.clone(), keys)
                            .await?;
                        connection
                            .after(After::Advance {
                                organization: identity.organization,
                                blocks: roots,
                                subtree: true,
                                above: Vec::new(),
                            })
                            .await;
                    }
                    PutKeys::Escrow { escrow, keys } => {
                        connection
                            .store
                            .create_escrow(admitted.identity()?, escrow, keys)
                            .await?;
                    }
                    PutKeys::Reseal(keys) => {
                        connection
                            .store
                            .reseal_keys(admitted.account()?, keys)
                            .await?;
                    }
                }
                Ok(Done::Nothing)
            }
            Op::HoldObjects {
                block,
                objects,
                hold,
            } => {
                connection
                    .store
                    .hold_objects(admitted.identity()?, block, objects, hold)
                    .await?;
                Ok(Done::Nothing)
            }
            Op::ReserveUploads(objects) => {
                let identity = admitted.identity()?;
                at_most_urls(objects.len())?;
                let held = connection
                    .store
                    .held_objects(identity, objects.iter().map(|object| object.hash).collect())
                    .await?;
                // An object the caller cannot upload (no length) must be one it holds: any
                // other is refused as not found, the same whether or not someone stored it, so
                // its size and existence stay private and nothing is reserved for it.
                let mut missing = Vec::new();
                for (object, held) in objects.iter().zip(&held) {
                    match (held, object.length) {
                        (true, _) => {}
                        (false, Some(length)) => missing.push((object.hash, length)),
                        (false, None) => {
                            return Err(ServerError::Refused(
                                ErrorCode::ObjectNotFound,
                                format!("object {} is not held", object.hash),
                            ));
                        }
                    }
                }
                connection.store.reserve_uploads(identity, missing).await?;
                let lifetime = connection.store.transfer_lifetime();
                Ok(Done::Uploads(
                    objects
                        .into_iter()
                        .zip(held)
                        .map(|(object, held)| {
                            if held {
                                return None;
                            }
                            let length = object
                                .length
                                .and_then(|length| usize::try_from(length).ok())
                                .unwrap_or(usize::MAX);
                            let signed =
                                connection
                                    .transfers
                                    .presign_put(object.hash, length, lifetime);
                            Some(UploadUrl {
                                url: signed.url,
                                headers: signed
                                    .headers
                                    .into_iter()
                                    .map(|(name, value)| Header {
                                        name: name.to_owned(),
                                        value,
                                    })
                                    .collect(),
                            })
                        })
                        .collect(),
                ))
            }
            Op::CreateOrganization {
                id,
                name,
                root,
                pinned,
            } => Ok(Done::Organization(
                connection
                    .store
                    .create_organization(admitted.account()?, id, &name, root, pinned)
                    .await?,
            )),
            Op::CreateAgent {
                account: agent,
                name,
                recovery,
                signing,
                contact,
                record,
                escrow,
                bound_to,
                member,
            } => {
                let identity = admitted.identity()?;
                let block = bound_to.as_ref().map(|bound| bound.block);
                let (agent, token, before) = connection
                    .store
                    .create_agent(
                        identity.account,
                        identity.organization,
                        agent,
                        &name,
                        recovery,
                        signing,
                        contact,
                        record,
                        escrow,
                        bound_to,
                        member,
                    )
                    .await?;
                if let Some(block) = block {
                    connection
                        .after(After::Regranted {
                            organization: identity.organization,
                            block: block,
                            before: before,
                        })
                        .await;
                }
                connection
                    .after(After::MailboxChanged(vec![identity.account, agent.account]))
                    .await;
                Ok(Done::Agent(CreatedAgent { agent, token }))
            }
            Op::RotateAgentToken(account) => {
                let identity = admitted.identity()?;
                Ok(Done::Token(
                    connection
                        .store
                        .rotate_agent_token(identity.account, identity.organization, account)
                        .await?,
                ))
            }
            Op::CreateLink {
                link,
                card,
                kind,
                signed,
            } => {
                let account = admitted.account()?;
                if let LinkKind::Invitation(invitation) = &kind
                    && Some(invitation.organization)
                        != admitted.identity.map(|identity| identity.organization)
                {
                    return Err(invalid(
                        "an invitation is made in the organization it invites into",
                    ));
                }
                connection
                    .store
                    .create_link(account, link, card, kind, signed)
                    .await?;
                Ok(Done::Nothing)
            }
            Op::AcceptLink {
                link,
                card,
                contact,
                key,
                member,
            } => {
                let account = admitted.account()?;
                let accepted = connection
                    .store
                    .accept_link(account, link, card, contact, key, member)
                    .await?;
                if let Some(granted) = accepted.granted {
                    connection
                        .after(After::Regranted {
                            organization: granted.organization,
                            block: granted.block,
                            before: granted.before,
                        })
                        .await;
                }
                connection
                    .after(After::MailboxChanged(vec![account, accepted.owner]))
                    .await;
                Ok(Done::Nothing)
            }
            Op::CreateShareLink {
                link,
                block,
                sealed,
                signed,
            } => {
                connection
                    .store
                    .create_share_link(admitted.identity()?, link, block, sealed, signed)
                    .await?;
                Ok(Done::Nothing)
            }
            Op::OpenShareLink {
                link,
                signed,
                key,
                sealed,
            } => {
                let account = admitted.account()?;
                let opened = connection
                    .store
                    .open_share_link(account, link, signed, key, sealed)
                    .await?;
                // As any grant: the block's audience hears it, and its editors' key work
                // lists any scope key newer than the link's as unsealed to the opener.
                connection
                    .after(After::Revoked {
                        revoked: vec![contacts::Granted {
                            organization: opened.organization,
                            block: opened.block,
                            before: opened.before,
                        }],
                        accounts: vec![account],
                    })
                    .await;
                connection
                    .after(After::Recheck {
                        organization: opened.organization,
                        account: None,
                        within: Some(opened.block),
                    })
                    .await;
                Ok(Done::Nothing)
            }
            Op::RevokeShareLink { link, signed } => {
                let identity = admitted.identity()?;
                let (block, revoked) = connection
                    .store
                    .revoke_share_link(identity, link, signed)
                    .await?;
                if !revoked.fallen.is_empty() {
                    connection
                        .after(After::Revoked {
                            revoked: vec![contacts::Granted {
                                organization: identity.organization,
                                block,
                                before: revoked.before,
                            }],
                            accounts: revoked.fallen,
                        })
                        .await;
                }
                for account in revoked.left {
                    connection
                        .after(After::DropMember {
                            organization: identity.organization,
                            account,
                        })
                        .await;
                }
                Ok(Done::Nothing)
            }
            Op::DeleteLink { link, signed } => {
                connection
                    .store
                    .delete_link(admitted.account()?, link, signed)
                    .await?;
                Ok(Done::Nothing)
            }
            Op::RemoveContact {
                account,
                revocations,
            } => {
                let caller = admitted.account()?;
                let current = admitted.identity.map(|identity| identity.organization);
                let revoked = connection
                    .store
                    .remove_contact(caller, account, current, revocations)
                    .await?;
                connection
                    .after(After::Revoked {
                        revoked: revoked,
                        accounts: vec![caller, account],
                    })
                    .await;
                connection
                    .after(After::MailboxChanged(vec![caller, account]))
                    .await;
                Ok(Done::Nothing)
            }
            Op::SetRecoveryKey {
                public,
                signing,
                sealed,
                changes,
                retirements,
            } => {
                let account = admitted.account()?;
                let told: Vec<Uuid> = changes.iter().map(|change| change.contact).collect();
                connection
                    .store
                    .set_recovery_key(account, public, signing, sealed, changes, retirements)
                    .await?;
                for contact in told {
                    connection
                        .after(After::Tell {
                            account: contact,
                            push: Push::MailboxChanged,
                        })
                        .await;
                }
                Ok(Done::Nothing)
            }
            Op::StageRecoveryKey(part) => {
                connection
                    .store
                    .stage_recovery_key(admitted.account()?, part)
                    .await?;
                Ok(Done::Nothing)
            }
            Op::PublishName(signed) => {
                connection
                    .store
                    .publish_name(admitted.account()?, signed)
                    .await?;
                Ok(Done::Nothing)
            }
            Op::SetPushEndpoint(change) => {
                let identity = admitted.identity()?;
                match change {
                    PushEndpointChange::Register(endpoint) => {
                        connection.pusher.check(&endpoint)?;
                        connection
                            .store
                            .register_endpoint(identity, endpoint)
                            .await?;
                    }
                    PushEndpointChange::Forget(address) => {
                        connection
                            .store
                            .unregister_endpoint(identity, address)
                            .await?;
                    }
                }
                Ok(Done::Nothing)
            }
            Op::Append { block, entries } => {
                let identity = admitted.identity()?;
                for _ in &entries {
                    if !connection.log_entries.allow_key(identity.account) {
                        return Err(crate::limits::too_many(
                            "log entries a minute",
                            crate::log::MAX_ENTRIES_PER_MINUTE,
                        ));
                    }
                }
                let (logged, told) = connection
                    .store
                    .append_log(identity, block, entries)
                    .await?;
                if let Some(told) = told {
                    connection
                        .after(After::Logged {
                            place: Place {
                                organization: identity.organization,
                                block,
                            },
                            told: Arc::new(told),
                        })
                        .await;
                }
                Ok(Done::Logged(logged))
            }
            Op::Redact { block, seq } => {
                let identity = admitted.identity()?;
                if connection.store.redact_log(identity, block, seq).await? {
                    connection
                        .after(After::Redacted {
                            place: Place {
                                organization: identity.organization,
                                block,
                            },
                            seq,
                        })
                        .await;
                }
                Ok(Done::Nothing)
            }
            Op::SetMute { block, mode } => {
                let identity = admitted.identity()?;
                connection.store.set_mute(identity, block, mode).await?;
                connection
                    .after(After::Tell {
                        account: identity.account,
                        push: Push::MutesChanged {
                            organization: identity.organization,
                        },
                    })
                    .await;
                Ok(Done::Nothing)
            }
            Op::DeleteMessage(id) => {
                let account = admitted.account()?;
                if connection.store.delete_message(account, id).await? {
                    connection
                        .after(After::Tell {
                            account: account,
                            push: Push::MailboxChanged,
                        })
                        .await;
                }
                Ok(Done::Nothing)
            }
            Op::RevokeToken(push_address) => {
                // The session ends first: an endpoint that cannot be forgotten now is dropped
                // once its push service refuses a push, but a token the client has let go of
                // must not stay valid.
                if let Some(token) = connection.token.take() {
                    connection.store.logout(&token).await?;
                }
                if let (Some(account), Some(address)) = (connection.account, push_address)
                    && let Err(error) = connection.store.forget_endpoint_of(account, address).await
                {
                    tracing::warn!(%account, %error, "a logout could not forget its push endpoint");
                }
                connection.account = None;
                connection.closing = true;
                connection.hub.leave_account(connection.client).await;
                Ok(Done::Nothing)
            }
        }
    }

    async fn send(
        &mut self,
        organization: Option<Uuid>,
        target: Target,
        payload: Vec<u8>,
    ) -> Result<(), ServerError> {
        let bound = bound_may_send(&target);
        let admitted = self
            .admit(organization, target_rule(&target), bound)
            .await?;
        let connection = &self.connection;
        match target {
            Target::Session { block, kind } => {
                let identity = admitted.identity()?;
                let place = Place {
                    organization: identity.organization,
                    block,
                };
                let tag = *self.following.sessions.get(&place).ok_or_else(not_joined)?;
                if let SessionSend::Host(to) = kind {
                    crate::limits::at_most(
                        "a message to the host",
                        payload.len(),
                        crate::limits::MAX_HOST_MESSAGE_BYTES,
                    )?;
                    let route = connection
                        .registry
                        .host_route(place, connection.client, identity.account, to)
                        .await;
                    let target = match route {
                        crate::sessions::HostRoute::To(target) => target,
                        crate::sessions::HostRoute::NoHost => {
                            return Err(invalid("no one hosts the block now"));
                        }
                        crate::sessions::HostRoute::SlowDown => {
                            return Err(ServerError::Refused(
                                ErrorCode::SlowDown,
                                format!(
                                    "at most {} messages a second to the host",
                                    crate::limits::HOST_MESSAGES_PER_SECOND
                                ),
                            ));
                        }
                        crate::sessions::HostRoute::Refused => {
                            return Err(ServerError::Refused(
                                ErrorCode::PermissionDenied,
                                "only the host sends to a member, and a member only to the host"
                                    .into(),
                            ));
                        }
                    };
                    let connection = &self.connection;
                    connection
                        .hub
                        .send_to(
                            target,
                            Push::Hosted {
                                block: place,
                                from: connection.client,
                                payload: payload.into(),
                                received: be_block_commit::now_milliseconds(),
                            },
                        )
                        .await;
                    return Ok(());
                }
                let participants = connection.registry.participants(place).await;
                if !participants.contains(&connection.client) {
                    return Err(not_joined());
                }
                match kind {
                    SessionSend::Claim => {
                        // The rule found it may edit: the order says so before its claim.
                        connection
                            .registry
                            .access(place, connection.client, true)
                            .await;
                        match connection.registry.claim(place, connection.client).await {
                            true => Ok(()),
                            false => Err(not_joined()),
                        }
                    }
                    SessionSend::Ephemeral(to) => {
                        crate::limits::at_most(
                            "a relayed message",
                            payload.len(),
                            MAX_RELAY_BYTES,
                        )?;
                        let message = Push::Relayed {
                            block: place,
                            from: connection.client,
                            payload: payload.into(),
                            received: be_block_commit::now_milliseconds(),
                        };
                        match to {
                            Some(target) if participants.contains(&target) => {
                                connection.hub.send_to(target, message).await;
                            }
                            // Named for one client, which has left (or never joined): the relay
                            // is best-effort, so there is nobody to deliver it to, and the
                            // sender, which cannot know it left, is not told of an error.
                            Some(_) => {}
                            None => {
                                connection
                                    .hub
                                    .send_all(&participants, connection.client, &message)
                                    .await;
                            }
                        }
                        Ok(())
                    }
                    SessionSend::Host(_) => unreachable!("sent above"),
                    SessionSend::Ordered(submission) => {
                        if !connection.registry.speaks(place, connection.client).await {
                            return Err(invalid(
                                "the session speaks another build of the block's type",
                            ));
                        }
                        crate::limits::at_most("a submission", payload.len(), MAX_RELAY_BYTES)?;
                        let submission = be_block_session::Submission {
                            id: be_block_session::OpId {
                                origin: submission.origin,
                                counter: submission.counter,
                            },
                            base: submission.base,
                            barrier: submission.barrier,
                            payload,
                        };
                        // Numbered and sent with the block's order held, so the participants hear
                        // two submissions numbered at once in the order they were numbered.
                        let _ordering = connection.registry.ordering(place).await;
                        let numbered = connection
                            .store
                            .submit_journal(place.organization, place.block, tag, submission)
                            .await?;
                        if let crate::journal::Submitted::Numbered(entry) = numbered {
                            // Sent after the entry is kept, so a connection joining meanwhile
                            // either reads it in the journal or hears it here.
                            connection.registry.sequenced(place, entry).await;
                        }
                        Ok(())
                    }
                }
            }
            Target::Devices(to) => {
                let account = admitted.account()?;
                crate::limits::at_most("a pairing message", payload.len(), MAX_PAIRING_BYTES)?;
                let message = Push::Paired(PairingMessage {
                    from: connection.client,
                    payload,
                });
                let sent = connection
                    .hub
                    .send_account(account, connection.client, to, &message)
                    .await;
                if to.is_some() && sent == 0 {
                    return Err(invalid("that device is no longer connected"));
                }
                Ok(())
            }
            Target::Push {
                block,
                mentions,
                keys,
            } => {
                connection
                    .notify(admitted.identity()?, block, payload, mentions, keys)
                    .await
            }
        }
    }
}

impl Connection {
    /// Tells what removing `account` from `organization` changed: its connections close, the
    /// blocks it was revoked from are told, and key work for everyone, as for a demotion, since
    /// the removed may have administered.
    async fn removed(
        &mut self,
        organization: Uuid,
        account: Uuid,
        revoked: Vec<crate::contacts::Granted>,
    ) {
        self.after(After::DropMember {
            organization,
            account,
        })
        .await;
        self.after(After::Revoked {
            revoked,
            accounts: vec![account],
        })
        .await;
        self.after(After::KeyWorkChangedForAll { organization })
            .await;
    }

    /// Tells the followers of `place`'s log who may still view it of entries appended to it.
    async fn logged(&self, place: Place, told: Arc<crate::log::Told>) {
        self.tell_log_followers(place, Push::Logged { block: place, told })
            .await;
    }

    /// Tells the followers of `place`'s log who may still view it `push`.
    async fn tell_log_followers(&self, place: Place, push: Push) {
        let followers = self.hub.log_followers(place).await;
        if followers.is_empty() {
            return;
        }
        let asked = followers
            .iter()
            .map(|(_, identity)| (*identity, place.block))
            .collect();
        let viewing = match self.store.viewing(place.organization, asked).await {
            Ok(viewing) => viewing,
            Err(error) => {
                tracing::warn!(%error, "who may view a log could not be read");
                return;
            }
        };
        let clients: Vec<u64> = followers
            .into_iter()
            .zip(viewing)
            .filter_map(|((client, _), viewing)| viewing.then_some(client))
            .collect();
        self.hub.send_every(&clients, &push).await;
    }

    /// Does `after` now, or once the batch running commits.
    async fn after(&mut self, after: After) {
        match &mut self.deferred {
            Some(deferred) => deferred.push(after),
            None => self.run(after).await,
        }
    }

    async fn run(&self, after: After) {
        match after {
            After::Advance {
                organization,
                blocks,
                subtree,
                above,
            } => self.advance(organization, blocks, subtree, above).await,
            After::Roots {
                organization,
                blocks,
                before,
            } => self.roots(organization, blocks, before).await,
            After::Sealed {
                place,
                head,
                told,
                trimmed,
            } => self.registry.sealed(place, head, told, &trimmed).await,
            After::Moved {
                organization,
                block,
                parent,
                moved:
                    crate::blocks::Moved {
                        before,
                        lost,
                        above,
                        scope_before: _,
                    },
            } => {
                // Its roots change for whoever could know of it only when it leaves or reaches
                // the top or the trash, or who may know of it changed.
                let top =
                    above.is_empty() || matches!(parent, BlockParent::Root | BlockParent::Detached);
                self.advance(organization, vec![block], true, above).await;
                if let Ok(after) = self.store.audience(organization, vec![block]).await
                    && (top || after != before)
                {
                    let mut accounts = after;
                    accounts.extend(before.iter().copied());
                    self.hub.roots(organization, &accounts).await;
                }
                if !lost.is_empty() {
                    self.key_work_changed(organization, before.clone()).await;
                }
                // Whoever can no longer view the block leaves its sessions, and a change of
                // edit access is written into each (trash is reversible, so no keys rotate for
                // it).
                self.recheck_sessions(organization, None, Some(block)).await;
            }
            After::Revoked { revoked, accounts } => self.revoked(&revoked, &accounts).await,
            After::Recheck {
                organization,
                account,
                within,
            } => self.recheck_sessions(organization, account, within).await,
            After::RoleChanged {
                organization,
                account,
                role,
            } => {
                // A role changes edit access across the organization: the hub takes it for
                // every connection of the account there, and each session it is in hears it.
                self.hub.set_role(organization, account, role).await;
                self.recheck_sessions(organization, Some(account), None)
                    .await;
                // A demotion outdates every epoch under the escrow: every
                // member's devices hear of it, so their next write rotates its scope.
                match role {
                    be_block_protocol::OrganizationRole::Administrator => {
                        self.key_work_changed(organization, BTreeSet::new()).await;
                    }
                    be_block_protocol::OrganizationRole::Editor
                    | be_block_protocol::OrganizationRole::Guest => {
                        self.hub
                            .announce(organization, Push::KeyWorkChanged { organization })
                            .await;
                    }
                }
            }
            After::DropMember {
                organization,
                account,
            } => self.hub.drop_member(organization, account).await,
            After::Logged { place, told } => self.logged(place, told).await,
            After::Redacted { place, seq } => {
                self.tell_log_followers(place, Push::Redacted { block: place, seq })
                    .await;
            }
            After::KeyWorkChanged {
                organization,
                accounts,
            } => self.key_work_changed(organization, accounts).await,
            After::KeyWorkChangedForAll { organization } => {
                self.hub
                    .announce(organization, Push::KeyWorkChanged { organization })
                    .await;
            }
            After::KeysChanged {
                organization,
                accounts,
            } => self.keys_changed(organization, accounts).await,
            After::MailboxChanged(accounts) => self.mailbox_changed(&accounts).await,
            After::Regranted {
                organization,
                block,
                before,
            } => self.regranted(organization, block, before).await,
            After::Tell { account, push } => {
                self.hub
                    .send_account(account, self.client, None, &push)
                    .await;
            }
        }
    }

    async fn authenticated(
        &mut self,
        profile: store::Profile,
        token: String,
    ) -> Result<Authenticated, ServerError> {
        let bound = self.store.bound_to(profile.account).await?;
        if let Some(account) = self.account.filter(|_| self.bound.is_some()) {
            self.hub.leave_live(account, self.client);
        }
        self.bound = bound;
        let replaced = bound.and_then(|_| self.hub.go_live(profile.account, self.client));
        if let Some(replaced) = replaced {
            // The replaced connection closes at its next sign-in check; its sessions are left
            // now, so this one takes them over at once.
            self.store.sessions_changed();
            self.registry.leave_all(replaced).await;
        }
        self.hub.join_account(self.client, profile.account).await;
        self.account = Some(profile.account);
        self.token = Some(token.clone());
        self.checked = None;
        Ok(Authenticated {
            account: profile.account,
            email: profile.email,
            display_name: profile.display_name,
            token,
            client: self.client,
        })
    }

    /// Who the connection's account is in `organization`: a member, or refused. The first call
    /// naming it makes the connection one that works there (`WatchHub::join_organization`),
    /// which hears what its account's connections there hear.
    async fn identity_in(&self, organization: Uuid) -> Result<Identity, ServerError> {
        let account = self.account()?;
        if let Some(identity) = self.hub.identity(self.client, organization).await {
            return Ok(identity);
        }
        let bound = match self.bound {
            Some((of, block)) if of == organization => Some(block),
            Some(_) => return Err(bound_refused()),
            None => None,
        };
        let role = self.store.membership(account, organization).await?;
        let identity = Identity {
            account,
            organization,
            role,
            bound,
        };
        self.hub.join_organization(self.client, identity).await;
        Ok(identity)
    }

    /// Fails once the connection's token no longer signs it in. It resolves the token again
    /// only after a session changed (`ServerStore::sessions_changed`), not on every call.
    pub(crate) async fn signed_in(&mut self) -> Result<(), ServerError> {
        let Some(token) = &self.token else {
            return Ok(());
        };
        let version = *self.store.session_changes.borrow();
        if self.checked == Some(version) {
            return Ok(());
        }
        self.store.resolve_token(token).await?;
        if let (Some(account), Some((organization, block))) = (self.account, self.bound) {
            if !self.hub.is_live(account, self.client) {
                return Err(ServerError::Refused(
                    ErrorCode::NotAuthenticated,
                    "this agent signed in on another connection".into(),
                ));
            }
            // A connection working in its organization stops once its block is in the trash.
            if self.hub.identity(self.client, organization).await.is_some() {
                self.store.require_bound_live(organization, block).await?;
            }
        }
        self.checked = Some(version);
        Ok(())
    }

    /// Queues a push of `sealed` to each device of every member `notification_recipients`
    /// names who has no connection working in the organization, and drops each endpoint its
    /// service says is gone. One whose `keys` were all told before pushes nothing
    /// (`ServerStore::tell`).
    async fn notify(
        &self,
        identity: Identity,
        block: Uuid,
        sealed: Vec<u8>,
        mentions: Vec<Uuid>,
        keys: Vec<be_block_store::Hash>,
    ) -> Result<(), ServerError> {
        crate::limits::at_most(
            "a notification",
            sealed.len(),
            be_block_protocol::MAX_NOTIFICATION_BYTES,
        )?;
        if mentions.len() > crate::limits::MAX_MENTIONS {
            return Err(crate::limits::too_many(
                "accounts one notification mentions",
                crate::limits::MAX_MENTIONS,
            ));
        }
        if keys.len() > be_block_protocol::MAX_TOLD_KEYS {
            return Err(crate::limits::too_many(
                "keys one notification names",
                be_block_protocol::MAX_TOLD_KEYS,
            ));
        }
        if !keys.is_empty() && !self.store.tell(identity, block, keys).await? {
            return Ok(());
        }
        if !self.notifications.allow_key(identity.account) {
            return Err(crate::limits::too_many(
                "notifications a minute",
                crate::limits::MAX_NOTIFICATIONS_PER_MINUTE,
            ));
        }
        if !self
            .block_notifications
            .allow_key((identity.account, block))
        {
            return Err(crate::limits::too_many(
                "notifications about one block a minute",
                crate::limits::MAX_NOTIFICATIONS_PER_BLOCK,
            ));
        }
        let recipients = self
            .store
            .notification_recipients(identity, block, mentions)
            .await?;
        let accounts: Vec<Uuid> = recipients
            .iter()
            .map(|recipient| recipient.account)
            .collect();
        let connected = self.hub.connected(identity.organization, &accounts).await;
        let deliveries: Vec<crate::push::Delivery> = recipients
            .into_iter()
            .filter(|recipient| !connected.contains(&recipient.account))
            .map(|recipient| crate::push::Delivery {
                account: recipient.account,
                endpoints: recipient.endpoints,
                urgent: recipient.mentioned,
            })
            .collect();
        let message: Arc<[u8]> = be_block_protocol::PushMessage {
            organization: identity.organization,
            block,
            sender: identity.account,
            sealed,
        }
        .encode()
        .into();
        self.pusher.queue(
            &self.store,
            identity.organization,
            block,
            deliveries,
            &message,
        );
        Ok(())
    }

    fn account(&self) -> Result<Uuid, ServerError> {
        self.account.ok_or_else(|| {
            ServerError::Refused(
                ErrorCode::NotAuthenticated,
                "sign in with the connection's greeting".into(),
            )
        })
    }

    async fn revoked(&self, revoked: &[contacts::Granted], accounts: &[Uuid]) {
        let mut organizations = BTreeSet::new();
        for granted in revoked {
            self.regranted(granted.organization, granted.block, granted.before.clone())
                .await;
            self.key_work_changed(granted.organization, granted.before.clone())
                .await;
            organizations.insert(granted.organization);
        }
        for organization in organizations {
            for account in accounts {
                self.recheck_sessions(organization, Some(*account), None)
                    .await;
            }
        }
    }

    async fn key_work_changed(&self, organization: Uuid, accounts: BTreeSet<Uuid>) {
        for (client, _) in self.hub.audience(organization, &accounts).await {
            if client != self.client {
                self.hub
                    .send_to(client, Push::KeyWorkChanged { organization })
                    .await;
            }
        }
    }

    /// Checks again what each participant of the sessions in `organization` may do there,
    /// those of `account`'s connections only with one, and with `within` only on that block and
    /// below it: one that can no longer view the block leaves its session, and a change of
    /// edit access is written into the session's order (`Membership`).
    async fn recheck_sessions(
        &self,
        organization: Uuid,
        account: Option<Uuid>,
        within: Option<Uuid>,
    ) {
        let mut held = self.registry.sessions_in(organization).await;
        if let Some(root) = within {
            let ids = held.iter().map(|(place, _)| place.block).collect();
            // A store error keeps everything held: each block is checked just below.
            if let Ok(below) = self.store.within(organization, root, ids).await {
                held.retain(|(place, _)| below.contains(&place.block));
            }
        }
        for (block, members) in held {
            for (client, kind) in members {
                let Some(identity) = self.hub.identity(client, organization).await else {
                    continue;
                };
                if account.is_some_and(|account| identity.account != account) {
                    continue;
                }
                let access = |level| {
                    self.store
                        .require_access(identity, vec![block.block], level)
                };
                // A participant that falls to Join leaves too, and joins again as a Join member.
                let needs = match kind {
                    MemberKind::Participant => Access::View,
                    MemberKind::Join => Access::Join,
                };
                // A ban binds anyone below Edit, and one that cannot be read bars.
                let barred = access(Access::Edit).await.is_err()
                    && self
                        .store
                        .banned(identity, block.block)
                        .await
                        .unwrap_or(true);
                if barred || access(needs).await.is_err() {
                    self.registry.leave(block, client).await;
                    continue;
                }
                if kind == MemberKind::Join {
                    continue;
                }
                let may_edit = access(Access::Edit).await.is_ok();
                self.registry.access(block, client, may_edit).await;
            }
        }
    }

    /// Tells every other connection in the organization of `accounts` that keys they may open
    /// were stored (`topic.keys`), so one that found a key missing asks again.
    async fn keys_changed(&self, organization: Uuid, accounts: BTreeSet<Uuid>) {
        for (client, _) in self.hub.audience(organization, &accounts).await {
            if client != self.client {
                self.hub
                    .send_to(client, Push::KeysChanged { organization })
                    .await;
            }
        }
    }

    async fn mailbox_changed(&self, accounts: &[Uuid]) {
        for account in accounts {
            self.hub
                .send_account(*account, self.client, None, &Push::MailboxChanged)
                .await;
        }
    }

    /// Tells the followers of the trees `blocks` lie in (with `subtree`, the trees of every
    /// block below them too) and of those of `above`, the blocks above them before a move,
    /// that they advanced (`WatchHub::advance`), each one that may still view what it follows.
    async fn advance(
        &self,
        organization: Uuid,
        blocks: Vec<Uuid>,
        subtree: bool,
        above: Vec<Uuid>,
    ) {
        let advanced = self
            .store
            .advanced(organization, blocks, subtree, above)
            .await;
        let Ok((sequence, path)) = advanced else {
            return;
        };
        let followers = self.hub.followers(organization, &path).await;
        if followers.is_empty() {
            return;
        }
        let asked = followers
            .iter()
            .map(|(_, identity, entry)| (*identity, *entry))
            .collect();
        let Ok(viewing) = self.store.viewing(organization, asked).await else {
            return;
        };
        for ((client, _, entry), viewing) in followers.into_iter().zip(viewing) {
            let place = Place {
                organization,
                block: entry,
            };
            match viewing {
                true => self.hub.advance(client, place, sequence).await,
                // It lost view of what it follows (a grant revoked, a move): told once, it reads
                // the tree, is refused and lets go of what it showed of it.
                false => self.hub.left(client, place, sequence).await,
            }
        }
    }

    /// Tells whoever may know of `blocks` now, and `before`, that their roots may have changed
    /// (`topic.roots`): a move or a grant.
    async fn roots(&self, organization: Uuid, blocks: Vec<Uuid>, before: BTreeSet<Uuid>) {
        let Ok(mut accounts) = self.store.audience(organization, blocks).await else {
            return;
        };
        accounts.extend(before);
        self.hub.roots(organization, &accounts).await;
    }

    /// Who may view `block` and what is below it changed: `before` could know of it.
    async fn regranted(&self, organization: Uuid, block: Uuid, before: BTreeSet<Uuid>) {
        self.advance(organization, vec![block], true, Vec::new())
            .await;
        self.roots(organization, vec![block], before).await;
    }
}

/// Refuses a read or op that names more objects than one answer holds.
fn at_most_urls(objects: usize) -> Result<(), ServerError> {
    match objects > be_block_protocol::MAX_OBJECT_URLS {
        true => Err(invalid(format!(
            "ask for at most {} objects at a time",
            be_block_protocol::MAX_OBJECT_URLS
        ))),
        false => Ok(()),
    }
}
