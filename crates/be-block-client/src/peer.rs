use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use be_block::OrganizationPolicies;
use be_block_commit::{Commit, CommitId, CommitStore, MergeBase, MergeBaseWalk, now_milliseconds};
use be_block_content::{
    BlockContent, BlockMetadata, Entries, Root, Streamed, derived_metadata, payload_start,
};
use be_block_graph::{Access, BlockParent};
use be_block_keys::{RecoveryPublic, RecoverySecret};
use be_block_protocol::{
    AccessList, BlockSummary, ChangeCursor, ChangedPage, ClientId, ErrorCode, EscrowKey,
    HistoryEntry, KeyKind, KeySet, Listing, ObjectChange, OrganizationKind, OrganizationRole,
    PublishOutcome, ReferenceChange, RelayKind, ServerAccount, WrappedBy, client,
};
use be_block_store::{
    ChunkerConfig, Hash, Keyring, Manifest, ObjectStore, Part, StoreError, Stored, Vault,
};
use uuid::Uuid;

use crate::{
    ClientError,
    connection::{Answer, Connection},
    keys::KeyBook,
    refs::{Creation, ListedBlock, Op, Pending, Refs, RefsDb, Scope, Session},
};

/// A block as `Peer::read_contents` read it.
#[derive(Clone, Debug)]
pub struct BlockRead {
    pub summary: BlockSummary,
    /// The content of the summary's head.
    pub stored: Option<Stored>,
    /// What the block's session sequenced past its base, opened; none when it has nothing, or
    /// it could not be opened.
    pub journal: Option<be_block_protocol::Journal>,
}

const CARRIED_BYTES: usize = be_block_protocol::MAX_FRAME_BYTES / 2;
/// How many blocks one `ReadContents` asks for; the server may answer fewer, as many as fit
/// its answer, and the rest are asked for again. Tests read across several such calls with a
/// few blocks each, rather than making hundreds of blocks to cross one.
pub(crate) const CONTENTS_BLOCKS: usize = if cfg!(test) { 8 } else { 256 };

mod authorship;
mod import;
mod journal;
pub use journal::Submitted;
mod notifications;
mod offline;
mod prefixes;
mod rotation;
mod transfers;

pub use notifications::{
    MAX_NOTIFICATION_AGE_MILLISECONDS, MAX_NOTIFICATION_TEXT, Notification, OpenedNotification,
};

pub use authorship::Signer;
pub use import::Imported;
pub use prefixes::Registry;

#[derive(Clone)]
pub enum Credentials {
    Token(String),
    Header,
}

impl Credentials {
    pub async fn authenticate(
        self,
        connection: &Arc<Connection>,
    ) -> Result<be_block_protocol::Authenticated, ClientError> {
        let token = match self {
            Self::Header => None,
            Self::Token(token) => Some(token),
        };
        Ok(client::account::authenticate(connection, token).await??)
    }
}

pub struct PeerConfig {
    pub url: String,
    pub identity: RecoverySecret,
    pub credentials: Credentials,
    pub organization: Option<Uuid>,
    pub organization_name: String,
    pub chunker: ChunkerConfig,
    pub refs: RefsDb,
}

impl PeerConfig {
    pub fn new(url: impl Into<String>, identity: RecoverySecret, credentials: Credentials) -> Self {
        Self {
            url: url.into(),
            identity,
            credentials,
            organization: None,
            organization_name: "Organization".into(),
            chunker: ChunkerConfig::DEFAULT,
            refs: RefsDb::memory(),
        }
    }

    pub fn refs(mut self, refs: RefsDb) -> Self {
        self.refs = refs;
        self
    }

    pub fn organization(mut self, organization: Option<Uuid>) -> Self {
        self.organization = organization;
        self
    }

    pub fn chunker(mut self, chunker: ChunkerConfig) -> Self {
        self.chunker = chunker;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Saved {
    /// The write changed nothing: the block stays at this head.
    Unchanged(CommitId),
    /// The server took the commit: its head.
    Published(CommitId),
    /// Sealed and kept in the outbox, which sends it later (`Peer::drain`): the server does not
    /// have it yet.
    Kept(CommitId),
    Rejected {
        head: Option<CommitId>,
    },
}

impl Saved {
    /// The head the write left the block at on this device, unless it was rejected.
    pub fn published(self) -> Option<CommitId> {
        match self {
            Self::Unchanged(head) | Self::Published(head) | Self::Kept(head) => Some(head),
            Self::Rejected { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sealed {
    Unchanged(CommitId),
    Sealed(CommitId),
}

enum Content<'a> {
    Encoded(Uuid, Part<'a>),
    Written(Manifest),
}

pub struct Write<'a> {
    content: Content<'a>,
    references: Vec<Uuid>,
    derived: Entries,
    defined: Option<Entries>,
    expected: Option<CommitId>,
    merging: Option<(Option<CommitId>, CommitId)>,
    bookmark: Option<String>,
    sealed_through: Option<be_block_commit::SealedThrough>,
}

impl<'a> Write<'a> {
    /// Content's stored form, as bytes or as `BlockContent::parts`.
    pub fn encoded(
        content_type: Uuid,
        parts: impl Into<Part<'a>>,
        expected: Option<CommitId>,
    ) -> Self {
        Self::of(Content::Encoded(content_type, parts.into()), expected)
    }

    pub fn written(manifest: Manifest, expected: Option<CommitId>) -> Self {
        Self::of(Content::Written(manifest), expected)
    }

    fn of(content: Content<'a>, expected: Option<CommitId>) -> Self {
        Self {
            content,
            references: Vec::new(),
            derived: Entries::default(),
            defined: None,
            expected,
            merging: None,
            bookmark: None,
            sealed_through: None,
        }
    }

    pub fn described(mut self, references: Vec<Uuid>, derived: Entries) -> Self {
        self.references = references;
        self.derived = derived;
        self
    }

    pub fn defined(mut self, defined: Entries) -> Self {
        self.defined = Some(defined);
        self
    }

    pub fn merging(mut self, base: Option<CommitId>, ours: CommitId) -> Self {
        self.merging = Some((base, ours));
        self
    }

    pub fn bookmark(mut self, label: &str) -> Self {
        self.bookmark = Some(label.to_owned());
        self
    }

    /// The commit holds its session through a sequence of a generation
    /// (`Commit::sealed_through`).
    pub fn sealed_through(mut self, sealed: Option<be_block_commit::SealedThrough>) -> Self {
        self.sealed_through = sealed;
        self
    }
}

/// `Diverged` stays in the outbox for the replica host to merge. `Refused` drops a move or
/// define, and keeps a create or publish with the rest of that block waiting for the next
/// drain; `Failed` is kept likewise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Drained {
    Created {
        block: Uuid,
    },
    Published {
        block: Uuid,
        head: CommitId,
    },
    Diverged {
        block: Uuid,
        local: CommitId,
        remote: Option<CommitId>,
    },
    Unsent {
        block: Uuid,
    },
    Moved {
        block: Uuid,
        /// Where it was before.
        from: Option<BlockParent>,
    },
    Defined {
        block: Uuid,
    },
    /// Objects held for the block offline were uploaded and held.
    Held {
        block: Uuid,
    },
    /// The server refused a change. With `restored`, it was a move or a definition, and the
    /// block's summary is the server's again (`unchanged`); otherwise a publish.
    Refused {
        block: Uuid,
        code: ErrorCode,
        restored: bool,
    },
    /// The server refused to make the block, or one it was made under, or what was sealed of
    /// it offline is under a key its log shows outdated: what this device made of it is set
    /// aside among the unsent (`UnsentBlock::refused`).
    SetAside {
        block: Uuid,
        why: crate::refs::Aside,
    },
    Failed {
        block: Uuid,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reconciled {
    Done,
    Merge {
        base: Option<CommitId>,
        ours: CommitId,
        theirs: CommitId,
    },
}

/// Keys arrived: what was found missing is asked for again, and a search under way as they
/// did asks once more (`Peer::arrivals`).
fn keys_arrived(tried: &Mutex<HashSet<Uuid>>, arrived: &std::sync::atomic::AtomicU64) {
    tried.lock().unwrap().clear();
    arrived.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

pub struct Peer<S: ObjectStore> {
    connection: Arc<Mutex<Arc<Connection>>>,
    commits: CommitStore<S>,
    token: Mutex<String>,
    account: Uuid,
    organization: Uuid,
    session: Mutex<(OrganizationRole, OrganizationKind)>,
    refs: Arc<Refs>,
    resolver: Mutex<Option<Resolver>>,
    keys: KeyBook,
    current: Mutex<HashMap<Uuid, Uuid>>,
    /// Keys looked for and found missing, not asked for again until keys are stored
    /// (`keys_changed`) or a key opens.
    tried: Arc<Mutex<HashSet<Uuid>>>,
    /// The members key work last listed as signed into the organization's log by no record
    /// (`KeyWork::unsigned_members`).
    unsigned: Mutex<Vec<Uuid>>,
    /// How many times keys arrived (`keys_changed`): a search for keys under way as it changes
    /// asks once more, since what arrived may have come after the server answered.
    keys_arrived: Arc<std::sync::atomic::AtomicU64>,
    /// How many outbox drains are under way: announcements wait for their end
    /// (`send_announcements`).
    draining: std::sync::atomic::AtomicUsize,
    /// Commits whose objects the store holds every one of (`holds`). The store never drops an
    /// object, so a commit found whole stays whole.
    whole: Mutex<HashSet<CommitId>>,
    /// Records `verify_authored` found forged after asking the server, by account, block and
    /// a hash of the bytes and signature, for the connection that answered: asked again, they
    /// cost no round trip until the device reconnects (`authorship::Forged`).
    forged: Mutex<authorship::Forged>,
    /// The server's clock as measured when the session opened, which commits are stamped by
    /// (`commit_time`, `server_now`).
    server_clock: Mutex<ServerClock>,
    /// A local account's peer (`Peer::local`), which has no server: what it makes is final as
    /// it is made.
    local: bool,
    /// The blocks a local account's peer settled since they were last taken
    /// (`take_settled_here`): with no server to list what changed, this is how its host hears
    /// of them (a search index catching up, above all).
    settled_here: Mutex<HashSet<Uuid>>,
    /// The scope logs read in this run to judge each key (`Peer::outdated`), each asked for
    /// once.
    log_reads: Mutex<HashSet<(Uuid, Uuid)>>,
}

/// How long a read of an access log before a write may take before what was kept of it is
/// used instead (`Peer::bounded_read`).
const LOG_READ_LIMIT: std::time::Duration = std::time::Duration::from_secs(10);

/// What this device knows of the server's clock (`Peer::server_now`).
struct ServerClock {
    anchor: Option<Anchor>,
    offset: Option<i64>,
}

/// The server's clock when this run's session opened, and the connection's clock then.
struct Anchor {
    server: i64,
    at: std::time::Duration,
    clock: crate::Clock,
}

pub type Resolver = Arc<dyn Fn(Uuid, Vec<Uuid>) -> Vec<Uuid> + Send + Sync>;

impl<S: ObjectStore> Peer<S> {
    pub async fn connect(config: PeerConfig, store: S) -> Result<Self, ClientError> {
        let connection = Connection::connect(&config.url, &crate::Clock::real()).await?;
        let be_block_protocol::Authenticated { account, token, .. } =
            config.credentials.clone().authenticate(&connection).await?;
        Self::signed_in(connection, account, token, config, store).await
    }

    pub async fn signed_in(
        connection: Arc<Connection>,
        account: Uuid,
        token: String,
        config: PeerConfig,
        store: S,
    ) -> Result<Self, ClientError> {
        let organization = match config.organization {
            Some(organization) => organization,
            None => {
                crate::contacts::create_organization(
                    &connection,
                    &config.identity,
                    account,
                    config.organization_name.clone(),
                )
                .await?
                .id
            }
        };
        let peer = Self::assemble(Connection::offline(), account, organization, config, store)?;
        peer.attach(connection, token).await?;
        Ok(peer)
    }

    pub fn cached(config: PeerConfig, account: Uuid, store: S) -> Result<Self, ClientError> {
        let Some(organization) = config.organization else {
            return Err(ClientError::Disconnected(
                "a workspace is only made with the server".into(),
            ));
        };
        let peer = Self::assemble(Connection::offline(), account, organization, config, store)?;
        if peer.refs.session().is_none() {
            return Err(ClientError::Disconnected(format!(
                "this device has nothing saved for workspace {organization}"
            )));
        }
        Ok(peer)
    }

    /// A local account's peer, which has no server (guides/blocks/client.md, Saving and the outbox): its organization
    /// is made on this device the first time, with this account its one member, and what it
    /// makes is final as it is made. A commit is the block's head once it is sealed, as the
    /// server would take it, compared with the head it went on (`send_pending`), and creations,
    /// moves and definitions are settled as they are kept (`settle_locally`), so nothing waits
    /// to be sent. Listings answer from what the device holds (`Refs::local_listing`), and
    /// everything that needs a server, sharing above all, is refused as offline.
    pub fn local(
        config: PeerConfig,
        account: Uuid,
        organization: Uuid,
        store: S,
    ) -> Result<Self, ClientError> {
        let config = PeerConfig {
            url: String::new(),
            ..config
        };
        let mut peer = Self::assemble(Connection::offline(), account, organization, config, store)?;
        peer.local = true;
        if peer.refs.session().is_none() {
            peer.refs.set_session(Session {
                role: OrganizationRole::Editor,
                kind: OrganizationKind::Personal,
                escrows: Vec::new(),
                pinned: crate::contacts::Pinned::default(),
                server: None,
                server_offset: None,
            })?;
        }
        *peer.session.lock().unwrap() = (OrganizationRole::Editor, OrganizationKind::Personal);
        peer.settle_locally()?;
        Ok(peer)
    }

    /// Whether this is a local account's peer, with no server (`Peer::local`).
    pub fn is_local(&self) -> bool {
        self.local
    }

    /// The blocks a local account's peer made, changed, moved or named since this was last
    /// asked: what a server's `changed-since` would list for it.
    pub fn take_settled_here(&self) -> HashSet<Uuid> {
        std::mem::take(&mut *self.settled_here.lock().unwrap())
    }

    fn assemble(
        connection: Arc<Connection>,
        account: Uuid,
        organization: Uuid,
        config: PeerConfig,
        store: S,
    ) -> Result<Self, ClientError> {
        let refs = config.refs.scope(Scope {
            server: config.url.clone(),
            account,
            organization,
        })?;
        let session = refs.session();
        let ring = Keyring::new();
        let _ = refs.add_signer(account, config.identity.signing().0, true, None);
        let keys = KeyBook::new(account, organization, config.identity, ring.clone());
        keys.set_contacts(
            refs.contacts()
                .into_iter()
                .map(|(account, record)| (account, record.card.public)),
        );
        if let Some(session) = &session {
            keys.set_escrows(session.escrows.clone());
            keys.set_pinned(Some(session.pinned.clone()));
            if let Some(server) = session.server {
                keys.set_server(server.account, RecoveryPublic(server.public));
            }
        }
        keys.set_kept_logs(refs.kept_logs());
        keys.absorb(refs.keys());
        let peer = Self {
            connection: Arc::new(Mutex::new(connection)),
            commits: CommitStore::new(Vault::new(store, ring).with_chunker(config.chunker)),
            token: Mutex::new(String::new()),
            account,
            organization,
            session: Mutex::new(session.as_ref().map_or(
                (OrganizationRole::Editor, OrganizationKind::default()),
                |session| (session.role, session.kind),
            )),
            refs: Arc::new(refs),
            resolver: Mutex::new(None),
            keys,
            current: Mutex::new(HashMap::new()),
            tried: Arc::default(),
            unsigned: Mutex::new(Vec::new()),
            keys_arrived: Arc::default(),
            draining: std::sync::atomic::AtomicUsize::new(0),
            whole: Mutex::new(HashSet::new()),
            forged: Mutex::new(authorship::Forged::default()),
            server_clock: Mutex::new(ServerClock {
                anchor: None,
                offset: session.as_ref().and_then(|session| session.server_offset),
            }),
            local: false,
            settled_here: Mutex::new(HashSet::new()),
            log_reads: Mutex::new(HashSet::new()),
        };
        for summary in peer.refs.summaries() {
            if let Some(key) = summary.key {
                peer.make_current(summary.id, key);
            }
        }
        // Every key that opens from here on arrived, by whatever path opened it: it is told as
        // keys the server says were stored are (`Event::KeysChanged`), and counted alike.
        let (tried, arrived, connection) = (
            Arc::clone(&peer.tried),
            Arc::clone(&peer.keys_arrived),
            Arc::clone(&peer.connection),
        );
        peer.keys.on_opened(move || {
            keys_arrived(&tried, &arrived);
            let connection = connection.lock().unwrap().clone();
            connection.keys_opened();
        });
        Ok(peer)
    }

    pub async fn attach(
        &self,
        connection: Arc<Connection>,
        token: String,
    ) -> Result<(), ClientError> {
        *self.connection.lock().unwrap() = connection;
        *self.token.lock().unwrap() = token;
        self.open_session().await
    }

    /// The time a commit made now is stamped with: this device's clock, held to the server's
    /// plus a little once a session measured it, and not before `parent`'s
    /// (`be_block_commit::commit_time`).
    pub fn commit_time(&self, parent: Option<&Commit>) -> i64 {
        be_block_commit::commit_time(
            now_milliseconds(),
            self.server_now(),
            parent.map(|parent| parent.time),
        )
    }

    /// The latest commit time this device believes (`be_block_commit::latest_believed`).
    pub fn latest_believed(&self) -> i64 {
        be_block_commit::latest_believed(now_milliseconds(), self.server_now())
    }

    /// The server's clock now, as estimated: from when this run's session opened by the
    /// connection's monotonic clock, so a jump of the wall clock after that changes nothing;
    /// before that (offline since a restart) from the offset last measured; none if never
    /// measured. A pinned clock (`be_block_commit::pin_now`) is the test's, which an embedded
    /// server reads too, so it is used with the offset.
    pub fn server_now(&self) -> Option<i64> {
        let held = self.server_clock.lock().unwrap();
        match &held.anchor {
            Some(anchor) if !be_block_commit::is_pinned() => {
                let elapsed = anchor.clock.now().saturating_sub(anchor.at);
                Some(
                    anchor
                        .server
                        .saturating_add(i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)),
                )
            }
            _ => held
                .offset
                .map(|offset| now_milliseconds().saturating_add(offset)),
        }
    }

    /// Takes the server's clock it answered with as the session opened, unless it would pull
    /// commit times back too far (`be_block_commit::trusts_measurement`).
    fn measured_server(&self, server: i64, clock: &crate::Clock) {
        let local = now_milliseconds();
        if !be_block_commit::trusts_measurement(local, server) {
            return;
        }
        *self.server_clock.lock().unwrap() = ServerClock {
            anchor: Some(Anchor {
                server,
                at: clock.now(),
                clock: clock.clone(),
            }),
            offset: Some(server.saturating_sub(local)),
        };
    }

    pub fn detach(&self) {
        *self.connection.lock().unwrap() = Connection::offline();
    }

    pub fn is_attached(&self) -> bool {
        !self.connection().is_closed()
    }

    async fn open_session(&self) -> Result<(), ClientError> {
        let connection = self.connection();
        let organization = self.organization;
        let be_block_protocol::OrganizationOpened {
            role,
            kind,
            escrows,
            pinned,
            server,
            time,
            client: opened,
            ..
        } = client::organizations::open_organization(&connection, organization).await??;
        connection.opened(opened);
        connection.hold_with(Arc::clone(&self.refs) as Arc<dyn crate::connection::Held>);
        self.measured_server(time, connection.clock());
        *self.session.lock().unwrap() = (role, kind);
        self.keys
            .set_server(server.account, RecoveryPublic(server.public));
        let pins = self.refs.contacts().into_iter().collect();
        let verified =
            crate::contacts::contacts(&connection, self.account, self.keys.identity(), &pins)
                .await?;
        crate::contacts::register_identity(&connection, self.keys.identity()).await?;
        self.keys.set_pinned(crate::contacts::open_pin(
            self.keys.identity(),
            organization,
            &pinned,
        ));
        let made = escrows.is_empty();
        self.learn_administrators().await;
        self.keys.set_escrows(escrows);
        self.adopt_contacts(&verified);
        self.persist_session()?;
        self.refresh_keys(None).await?;
        if made && kind == OrganizationKind::Company && role == OrganizationRole::Administrator {
            let _ = self.make_escrow().await;
        }
        self.save_pin().await?;
        if kind == OrganizationKind::Company && role == OrganizationRole::Administrator {
            self.ensure_organization_settings().await?;
        }
        self.persist_session()
    }

    pub fn server(&self) -> Option<ServerAccount> {
        self.keys.server().map(|(account, public)| ServerAccount {
            account,
            public: public.0,
        })
    }

    pub fn organization_settings(&self) -> Uuid {
        be_block::organization_settings(self.organization)
    }

    pub async fn share_with_server(&self, block: Uuid) -> Result<(), ClientError> {
        // A standing grant, which takes no record (`signed_grant`).
        let (account, public) = self.keys.server().ok_or(ClientError::Unexpected)?;
        self.share_sealed(block, account, Access::View, &public, None)
            .await
    }

    async fn ensure_organization_settings(&self) -> Result<(), ClientError> {
        let settings = self.organization_settings();
        let made = match self.summary(settings).await {
            Ok(summary) => summary,
            Err(ClientError::Refused(ErrorCode::BlockNotFound, _)) => {
                match self
                    .create_with_id::<be_block::SettingsContent>(settings, BlockParent::Root)
                    .await
                {
                    // Another administrator's device made it since.
                    Ok(_) | Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => {}
                    Err(error) => return Err(error),
                }
                self.summary(settings).await?
            }
            Err(error) => return Err(error),
        };
        // Shared each time, as nothing this device reads says whether the server holds it: a
        // device that stopped between making the settings and sharing them would otherwise leave
        // the server refusing what the policies govern (`PolicyUnreadable`). Sharing again
        // changes nothing. A device that cannot open the settings' key yet leaves it to one
        // that can.
        match self.share_with_server(settings).await.map_err(locked) {
            Ok(()) | Err(ClientError::Locked(_)) => {}
            Err(error) => return Err(error),
        }
        if made.head.is_none() {
            self.save(settings, &be_block::SettingsContent::default(), None)
                .await?;
        }
        Ok(())
    }

    pub async fn organization_policies(&self) -> Result<OrganizationPolicies, ClientError> {
        if self.kind() != OrganizationKind::Company {
            return Ok(OrganizationPolicies::default());
        }
        let settings = match self
            .open::<be_block::SettingsContent>(self.organization_settings())
            .await
        {
            Ok(settings) => settings,
            Err(ClientError::Refused(ErrorCode::BlockNotFound, _)) => None,
            Err(error) => return Err(error),
        };
        let Some(block) = settings
            .and_then(|settings| settings.root().policy(OrganizationPolicies::CONTENT_TYPE))
        else {
            return Ok(OrganizationPolicies::default());
        };
        Ok(self
            .open::<be_block::OrganizationPoliciesContent>(block)
            .await?
            .map(|content| content.root())
            .unwrap_or_default())
    }

    fn persist_session(&self) -> Result<(), ClientError> {
        let (role, kind) = *self.session.lock().unwrap();
        Ok(self.refs.set_session(Session {
            role,
            kind,
            escrows: self.keys.escrows(),
            pinned: self.keys.pins(),
            server: self.server(),
            server_offset: self.server_clock.lock().unwrap().offset,
        })?)
    }

    pub(crate) fn adopt_escrows(&self, chain: Vec<EscrowKey>) {
        self.keys.set_escrows(chain);
        let _ = self.persist_session();
    }

    /// Reads the organization from its log, checked from the root this account pinned: its
    /// administrators, for the escrow keys they make (`verify_escrow`), and its members, whose
    /// escrow wraps are opened. None are known when the log does not check, so no newer escrow
    /// key is taken and no escrow wrap opened.
    pub(crate) async fn learn_administrators(&self) {
        match self.checked_organization().await {
            // A log short of what was kept leaves the organization as this device last held
            // it, or judged by what was kept (`KeyBook::kept`).
            Err(ClientError::Withheld(_)) => {}
            // Nor does a read that did not get through: the last one this run took stands.
            Err(error) if error.is_unreachable() => {}
            checked => self.keys.set_organization(checked.ok().flatten().as_ref()),
        }
        self.keys.absorb(KeySet::default());
    }

    /// Takes the keys opened from escrow wraps (`KeyBook::escrowed`) whose sender administers
    /// the organization (holding the escrow key, they read everything it opens, and pass on a
    /// root only one they pinned), or whom the block's log names with Edit or above: checked from the root this device pinned for the block, or,
    /// for one it pinned none for (reached only through escrow), from the root the wrap carries,
    /// which is then pinned. A member could so root a block id an administrator never pinned at
    /// themselves (plans/signed_access.md, "Accepted limits"); the server, which is no member,
    /// cannot. Others wait: a grant may come.
    pub(crate) async fn settle_escrowed(&self) {
        // Again while a pass takes a key: a root it pinned may settle, or conflict with, a wrap
        // a pass looked at before.
        while self.settle_escrowed_once().await > 0 {}
    }

    async fn settle_escrowed_once(&self) -> usize {
        let mut taken = 0;
        for escrowed in self.keys.escrowed() {
            // An administrator's word stands for escrow wraps alone (it holds the escrow key).
            let administrator = escrowed.escrow && self.keys.is_administrator(escrowed.sender);
            let (pinned, weak) = match (self.keys.root_pin(escrowed.block), escrowed.root) {
                (
                    crate::keys::RootPin::Pinned(root) | crate::keys::RootPin::Weak(root),
                    Some(carried),
                ) if carried != root => {
                    self.keys.conflicting_escrowed(escrowed.key, carried);
                    continue;
                }
                (crate::keys::RootPin::Pinned(root), _) => (root, false),
                (crate::keys::RootPin::Weak(root), _) => (root, !administrator),
                (crate::keys::RootPin::Unknown, Some(carried)) => (carried, !administrator),
                _ => continue,
            };
            if administrator {
                self.keys.take_escrowed(escrowed.key, pinned, false);
                taken += 1;
                continue;
            }
            let Ok(entries) = self.served_log(escrowed.block).await else {
                continue;
            };
            let Ok(around) = self.logs_around(escrowed.block).await else {
                continue;
            };
            let checked = around
                .check(self.organization, escrowed.block, pinned, &entries)
                // A log short of what an earlier check kept is withheld: the key waits.
                .and_then(|checked| match self.keys.served(escrowed.block, &checked) {
                    crate::keys::Served::Take => Some(checked),
                    crate::keys::Served::Stale(newer) => Some(newer.0),
                    crate::keys::Served::Withheld => None,
                });
            let granted = checked
                .and_then(|checked| checked.grants.get(&escrowed.sender.into_bytes()).copied())
                .is_some_and(|(level, _)| level >= be_block_keys::Level::Edit);
            if granted {
                self.keys.take_escrowed(escrowed.key, pinned, weak);
                taken += 1;
            }
        }
        if taken > 0 {
            self.keys.absorb(KeySet::default());
        }
        taken
    }

    pub(crate) fn adopt_pinned(&self, pinned: crate::contacts::Pinned) {
        self.keys.set_pinned(Some(pinned));
        let _ = self.persist_session();
    }

    /// The organization's access log, root first, once its root checks against the one this
    /// account pinned (`KeyBook::organization_root`): from making the organization or the
    /// invitation it joined by. The personal organization has none.
    pub async fn organization_log(&self) -> Result<Vec<be_block_keys::Entry>, ClientError> {
        let served = crate::ask(client::organizations::get_organization_log(
            &self.connection(),
        ))
        .await?;
        let organization = self.organization;
        let Some(pinned) = self.keys.organization_root() else {
            return match served.is_empty() {
                true => Ok(Vec::new()),
                false => Err(ClientError::Locked(organization)),
            };
        };
        checked_log(
            be_block_keys::LogKind::Organization,
            organization,
            pinned,
            &served,
        )
    }

    pub(crate) fn adopt_contacts(&self, contacts: &[crate::Contact]) {
        for contact in contacts {
            if let Some(signing) = contact.signing {
                let _ = self
                    .refs
                    .add_signer(contact.account, signing.0, true, self.server_now());
            }
        }
        let verified: Vec<(Uuid, RecoveryPublic)> = contacts
            .iter()
            .filter_map(|contact| Some((contact.account, contact.public?)))
            .collect();
        // Pins only move forward: a contact the server stops listing keeps its pin, so it is
        // not trusted afresh from an older record if it comes back.
        let mut pinned: HashMap<Uuid, crate::contacts::OwnContact> =
            self.refs.contacts().into_iter().collect();
        for record in contacts.iter().filter_map(crate::Contact::record) {
            let account = Uuid::from_bytes(record.card.account);
            if pinned
                .get(&account)
                .is_none_or(|pin| pin.verified <= record.verified)
            {
                pinned.insert(account, record);
            }
        }
        let _ = self.refs.set_contacts(pinned.into_iter().collect());
        self.keys.set_contacts(verified);
        let _ = self.refs.set_contact_names(
            contacts
                .iter()
                .map(|contact| (contact.account, contact.display_name.clone()))
                .collect(),
        );
    }

    /// A contact's name as its verified card gives it, kept for when there is no connection.
    pub fn contact_name(&self, account: Uuid) -> Option<String> {
        self.refs.contact_name(account)
    }

    /// The name of `block` as this device last heard it, with no network: from its kept
    /// summary, opened with the keys kept here.
    pub fn kept_name(&self, block: Uuid) -> Option<String> {
        self.metadata(&self.refs.summary(block)?).ok()?.name()
    }

    pub fn kind(&self) -> OrganizationKind {
        self.session.lock().unwrap().1
    }

    pub fn current_key(&self, block: Uuid) -> Option<Uuid> {
        self.current.lock().unwrap().get(&block).copied()
    }

    pub fn keys(&self) -> &KeyBook {
        &self.keys
    }

    pub async fn refresh_keys(&self, blocks: Option<Vec<Uuid>>) -> Result<usize, ClientError> {
        let mut keys = KeySet::default();
        let mut after = None;
        loop {
            let page = client::keys::get_keys(&self.connection(), blocks.clone(), after).await??;
            keys.keys.extend(page.keys.keys);
            keys.wrapped.extend(page.keys.wrapped);
            match page.next {
                Some(next) if after.is_none_or(|after| next > after) => after = Some(next),
                Some(_) => return Err(ClientError::Unexpected),
                None => break,
            }
        }
        self.refs.add_keys(&keys)?;
        let unknown = keys.wrapped.iter().any(|wrap| {
            wrap.wrapped_by == WrappedBy::Account(self.account)
                && self.keys.sender(wrap.sender).is_none()
        });
        if unknown {
            self.refresh_contacts().await?;
        }
        if self.keys.escrow_senders_unknown(&keys) {
            self.learn_administrators().await;
        }
        // A contact's seal is checked against the block's log as of the head its epoch names
        // (`KeyBook::sender_may`, signed access 3d), so that log is read first for each block a
        // contact sealed a key of that is new here, when this device pinned its root.
        let mut sealed_by_contacts: Vec<Uuid> = keys
            .wrapped
            .iter()
            .filter(|wrap| {
                wrap.wrapped_by == WrappedBy::Account(self.account)
                    && wrap.sender != self.account
                    && self.keys.contact(wrap.sender).is_some()
                    && !self.keys.trusted(wrap.key)
            })
            .filter_map(|wrap| {
                keys.keys
                    .iter()
                    .find(|row| row.id == wrap.key)
                    .copied()
                    .or_else(|| self.keys.row(wrap.key))
            })
            .filter(|row| row.kind != KeyKind::Escrow)
            .map(|row| row.block)
            .filter(|block| matches!(self.keys.root_pin(*block), crate::keys::RootPin::Pinned(_)))
            .collect();
        sealed_by_contacts.sort();
        sealed_by_contacts.dedup();
        for block in sealed_by_contacts {
            let _ = self.checked_access(block).await;
        }
        let opened = self.keys.absorb(keys);
        self.settle_escrowed().await;
        // Each scope's log, once, so a write offline later is judged by what was kept of it
        // (`Peer::outdated`) rather than refused.
        if !self.local {
            use futures_util::StreamExt;
            let unkept: Vec<Uuid> = self
                .keys
                .unkept_scopes()
                .into_iter()
                .filter(|scope| !self.uncreated(*scope))
                .collect();
            futures_util::stream::iter(unkept)
                .map(|scope| async move { self.bounded_read(scope).await })
                .buffer_unordered(8)
                .collect::<Vec<_>>()
                .await;
        }
        Ok(opened)
    }

    pub fn note(&self, summary: &BlockSummary) {
        if let Some(key) = summary.key {
            self.make_current(summary.id, key);
        }
        self.observe(std::slice::from_ref(summary));
        let _ = self
            .refs
            .set_listing(Listing::Block(summary.id), std::slice::from_ref(summary));
        if !summary.access.can_edit() && self.refs.has_pending(summary.id) {
            let _ = self.move_to_unsent(summary.id, None);
        }
    }

    /// Keeps the server's heads `summaries` name (`behind`).
    fn observe(&self, summaries: &[BlockSummary]) {
        let _ = self.refs.observe(summaries, self.behind());
    }

    /// Whether a remote head this device hears of is behind the one it knows. The server's head
    /// only ever moves on to a new commit, so a head the known one descends from was heard of
    /// late: a summary read before this device published the newer one and answered after, or
    /// a merge of a divergence a later merge already settled.
    pub(crate) fn behind(&self) -> impl Fn(CommitId, CommitId) -> bool + '_ {
        |head, known| match self.commits.try_get(head) {
            // The common case, a publish or a merge on the known head: the newer, at once.
            Ok(Some(commit)) if commit.parents.contains(&known) => false,
            Ok(Some(_)) => self.commits.is_ancestor(head, known).unwrap_or(false),
            // A head this device does not hold is no ancestor of one it does.
            Ok(None) | Err(_) => false,
        }
    }

    fn make_current(&self, block: Uuid, key: Uuid) {
        let mut current = self.current.lock().unwrap();
        let older = current.get(&block).is_some_and(|known| {
            let epoch = |id| self.keys.row(id).map(|row| row.epoch);
            matches!((epoch(key), epoch(*known)), (Some(new), Some(old)) if new < old)
        });
        if !older {
            current.insert(block, key);
        }
    }

    /// Keys this account may open were stored (`Event::KeysChanged`): a key looked for and
    /// missing is asked for again, by `learn` or a read, rather than taken for locked.
    pub fn keys_changed(&self) {
        keys_arrived(&self.tried, &self.keys_arrived);
    }

    /// How many times keys arrived (`keys_changed`): a read that found a key missing, taken
    /// before it started, tells whether keys arrived during it, so it should read again rather
    /// than take the block for locked.
    pub fn arrivals(&self) -> u64 {
        self.keys_arrived.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn learn(&self, summaries: &[BlockSummary]) -> Result<(), ClientError> {
        let mut wanted = Vec::new();
        let mut arrivals = self.arrivals();
        {
            let mut tried = self.tried.lock().unwrap();
            for summary in summaries {
                self.note(summary);
                if let Some(key) = summary.key
                    && summary.access.can_view()
                    && !self.keys.has(key)
                    && tried.insert(key)
                {
                    wanted.push((summary.id, key));
                }
            }
        }
        while !wanted.is_empty() {
            self.refresh_keys(Some(wanted.iter().map(|(block, _)| *block).collect()))
                .await?;
            self.again(&mut wanted, &mut arrivals);
        }
        Ok(())
    }

    /// Keeps the keys of `wanted` to ask for once more: those still missing, if keys arrived
    /// while they were asked for (`keys_changed` since `arrivals`), since what arrived may
    /// have come after the server answered.
    fn again<T>(&self, wanted: &mut Vec<(T, Uuid)>, arrivals: &mut u64) {
        let now = self.arrivals();
        if now == *arrivals {
            wanted.clear();
            return;
        }
        *arrivals = now;
        let mut tried = self.tried.lock().unwrap();
        wanted.retain(|(_, key)| {
            tried.insert(*key);
            !self.keys.has(*key)
        });
    }

    async fn retrying<T>(
        &self,
        attempt: impl Fn() -> Result<T, ClientError>,
    ) -> Result<T, ClientError> {
        let mut arrivals = self.arrivals();
        let key = match attempt() {
            Err(ClientError::Store(StoreError::MissingKey(key))) => key,
            outcome => return outcome,
        };
        if !self.tried.lock().unwrap().insert(key) {
            return Err(ClientError::Locked(key));
        }
        let mut wanted = vec![((), key)];
        while !wanted.is_empty() {
            self.refresh_keys(None).await?;
            self.again(&mut wanted, &mut arrivals);
        }
        attempt().map_err(locked)
    }

    /// The key `block` is sealed with now. One its scope's log outdates gets the scope a new
    /// epoch first (signed access 3d), and when that cannot go through, the write fails as
    /// `Locked`: what is sealed here may be uploaded at once (a part, a relay, a journal entry).
    pub async fn sealing_key(&self, block: Uuid) -> Result<Uuid, ClientError> {
        self.sealing_key_for(block, false).await
    }

    /// `sealing_key` for what goes only into the outbox (a commit and the content it holds),
    /// which `send_pending` judges again before it is sent: offline, an outdated key is used
    /// rather than refuse a person's edit, and the edit is set aside then if it still is.
    async fn sealing_key_kept(&self, block: Uuid) -> Result<Uuid, ClientError> {
        self.sealing_key_for(block, true).await
    }

    async fn sealing_key_for(&self, block: Uuid, outbox: bool) -> Result<Uuid, ClientError> {
        let known = self.current.lock().unwrap().get(&block).copied();
        let key = match known {
            Some(key) => key,
            None => {
                let summary = self.summary(block).await?;
                summary.key.ok_or(ClientError::Locked(block))?
            }
        };
        // A key the server holds (the settings' and those opened through them) seals only the
        // settings block itself, which the server reads by design: never a block the server's
        // graph places there, since the server could place any block there.
        let settings = block == self.organization_settings();
        let usable = |key: Uuid| self.keys.trusted(key) || (settings && self.keys.server_held(key));
        if !usable(key) {
            self.refresh_keys(Some(vec![block])).await?;
        }
        if !usable(key) {
            return Err(ClientError::Locked(key));
        }
        // The server names the current key; a key of another block would seal this one where
        // that block's readers read it.
        if self.keys.block_of(key) != Some(block) {
            return Err(ClientError::Forged(block));
        }
        let newest = || {
            let epoch = self.keys.row(key).map_or(0, |row| row.epoch);
            self.keys
                .keys_of(block, KeyKind::Data)
                .into_iter()
                .rev()
                .find(|row| {
                    row.epoch > epoch && usable(row.id) && self.keys.block_of(row.id) == Some(block)
                })
                .map_or(key, |row| row.id)
        };
        let chosen = newest();
        // An epoch its scope's log outdates (named before a revocation, a downgrade or a
        // closing, whose subject may hold it) is written under no more: the scope gets a new
        // epoch first (signed access 3d).
        let kept = match self.refs.summary(block) {
            Some(kept) => Some(kept),
            None => self.summary(block).await.ok(),
        };
        if let Some(scope) = kept.and_then(|kept| scope_of(&kept).ok())
            && self.outdated(chosen, scope).await
        {
            match Box::pin(self.rotate(scope)).await {
                Ok(()) => {}
                // A person's edit made offline is never refused: it is sealed and kept here,
                // and judged again before it is sent (`Peer::send_pending`), which sets it aside
                // rather than send it under an outdated key.
                Err(error) if outbox && error.is_unreachable() => return Ok(chosen),
                Err(_) => return Err(ClientError::Locked(block)),
            }
            let rotated = newest();
            if self.keys.outdated(rotated, scope) {
                return Err(ClientError::Locked(block));
            }
            return Ok(rotated);
        }
        Ok(chosen)
    }

    /// Whether the logs of `scope` and of the scopes above whose keys `key` was opened through
    /// outdate it (`KeyBook::outdated`): each read first when this device holds none of it (as
    /// on a new device, or after a restart: logs are kept in memory alone), or when the key
    /// names a head it does not reach, which may be newer.
    ///
    /// A log whose read fails, or that does not reach what this device kept of it, is judged by
    /// what was kept (`KeyBook::outdated`); each is asked for once per key in a run, so a key
    /// naming a head no log the server serves reaches is not read for on every save. A local
    /// account has no server to withhold anything.
    ///
    /// A scope whose creation still waits in the outbox is not read: the server holds nothing
    /// of it to withhold, and what this device kept on making it stands. A read that does not
    /// answer within `LOG_READ_LIMIT` leaves what was kept.
    async fn outdated(&self, key: Uuid, scope: Uuid) -> bool {
        if self.local {
            return false;
        }
        for log in self.keys.logs_to_read(key, scope) {
            if !self.uncreated(log) && self.log_reads.lock().unwrap().insert((key, log)) {
                let _ = self.bounded_read(log).await;
            }
        }
        self.keys.outdated(key, scope)
    }

    /// Whether `commit`, an edit of `block` waiting to be sent, is sealed under a key the logs
    /// show outdated (`KeyBook::shown_outdated`), each read first as `outdated` reads them.
    async fn sent_outdated(&self, block: Uuid, commit: CommitId) -> Result<bool, ClientError> {
        let key = self.commits.vault().sealed_under(commit.hash())?;
        let kept = match self.refs.summary(block) {
            Some(kept) => Some(kept),
            None => self.summary(block).await.ok(),
        };
        let Some(scope) = kept.and_then(|kept| scope_of(&kept).ok()) else {
            return Ok(false);
        };
        for log in self.keys.logs_to_read(key, scope) {
            if !self.uncreated(log) && self.log_reads.lock().unwrap().insert((key, log)) {
                let _ = self.bounded_read(log).await;
            }
        }
        Ok(self.keys.shown_outdated(key, scope))
    }

    /// The block `hash` is an object of, when it is sealed under a key of it that the logs
    /// this device holds or kept show outdated (`KeyBook::shown_outdated`): an object not
    /// sealed, or under a key it cannot place, is none. The block's scope comes from its kept
    /// summary, else the server's; one whose scope cannot be told counts as outdated.
    pub(crate) async fn sealed_outdated(&self, hash: Hash) -> Option<Uuid> {
        let key = self.commits.vault().sealed_under(hash).ok()?;
        let block = self.keys.block_of(key)?;
        let summary = match self.refs.summary(block) {
            Some(kept) => Some(kept),
            None => self.summary(block).await.ok(),
        };
        let Some(scope) = summary.and_then(|summary| scope_of(&summary).ok()) else {
            return Some(block);
        };
        self.keys.shown_outdated(key, scope).then_some(block)
    }

    /// `checked_access`, given up after `LOG_READ_LIMIT`: a key judged by a log that does not
    /// arrive is judged by what was kept of it.
    async fn bounded_read(&self, block: Uuid) -> Result<(), ClientError> {
        let clock = self.connection().clock().clone();
        let read = std::pin::pin!(self.checked_access(block));
        match futures_util::future::select(read, clock.sleep(LOG_READ_LIMIT)).await {
            futures_util::future::Either::Left((read, _)) => read.map(|_| ()),
            futures_util::future::Either::Right(_) => {
                Err(ClientError::TimedOut("reading an access log"))
            }
        }
    }

    /// The newest scope key of `scope` this device holds; one its log outdates (as
    /// `sealing_key` finds a data key) is wrapped under no more, so the scope gets a new epoch
    /// first.
    async fn scope_key(&self, scope: Uuid) -> Result<Uuid, ClientError> {
        let key = match self.keys.scope_key(scope) {
            Some(key) => key,
            None => {
                self.refresh_keys(Some(vec![scope])).await?;
                self.keys
                    .scope_key(scope)
                    .ok_or(ClientError::Locked(scope))?
            }
        };
        if self.outdated(key, scope).await {
            Box::pin(self.rotate(scope))
                .await
                .map_err(|_| ClientError::Locked(scope))?;
            return self
                .keys
                .scope_key(scope)
                .filter(|key| !self.keys.outdated(*key, scope))
                .ok_or(ClientError::Locked(scope));
        }
        Ok(key)
    }

    async fn scope_key_under(&self, parent: Uuid) -> Result<Uuid, ClientError> {
        let summary = self.summary(parent).await?;
        self.note(&summary);
        self.scope_key(scope_of(&summary)?).await
    }

    async fn new_keys(
        &self,
        block: Uuid,
        parent: BlockParent,
        owner_only: bool,
    ) -> Result<(Uuid, KeySet), ClientError> {
        if owner_only {
            let (scope, mut keys) = self.keys.mint_own_scope(block)?;
            let (data, more) = self.keys.mint_data(block, scope)?;
            keys.extend(more);
            return Ok((data, keys));
        }
        let under = match parent {
            BlockParent::Block(parent) => self.scope_key_under(parent).await?,
            BlockParent::Root | BlockParent::Detached => {
                let (scope, keys) = self.keys.mint_scope(block, None)?;
                let (data, more) = self.keys.mint_data(block, scope)?;
                let mut keys = keys;
                keys.extend(more);
                return Ok((data, keys));
            }
        };
        Ok(self.keys.mint_data(block, under)?)
    }

    pub async fn read_manifest(&self, manifest: &Manifest) -> Result<Vec<u8>, ClientError> {
        self.fetch_range(manifest, 0, u64::MAX).await?;
        self.retrying(|| Ok(self.commits.vault().read(manifest)?))
            .await
    }

    /// The stored form and the parts it was saved in, for `BlockContent::from_stored`.
    pub async fn load_manifest(&self, manifest: &Manifest) -> Result<Stored, ClientError> {
        self.fetch_range(manifest, 0, u64::MAX).await?;
        self.retrying(|| Ok(self.commits.vault().load(manifest)?))
            .await
    }

    /// The stored form's first part, without the rest (`be_block_content::Loads::FirstPart`).
    pub async fn load_first_part(&self, manifest: &Manifest) -> Result<Stored, ClientError> {
        self.fetch_nodes(manifest).await?;
        let end = self
            .commits
            .vault()
            .outline(manifest)?
            .map_or(0, |root| root.first_end());
        self.fetch_range(manifest, 0, end).await?;
        self.retrying(|| Ok(self.commits.vault().load_first(manifest)?))
            .await
    }

    /// A range of a saved part's bytes, fetching only the objects under it.
    /// The block of the key `part`'s top object is sealed under, as this device placed that
    /// key (`KeyBook::block_of`), fetching the object if it is not held: none for a key this
    /// device does not place.
    pub async fn block_of_part(
        &self,
        part: be_block_store::PartRef,
    ) -> Result<Option<Uuid>, ClientError> {
        let hash = part.hash();
        if !self.commits.vault().store().has(hash)? {
            self.fetch(&[hash]).await?;
        }
        let key = self.commits.vault().sealed_under(hash)?;
        Ok(self.keys.block_of(key))
    }

    pub async fn read_part(
        &self,
        part: be_block_store::PartRef,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, ClientError> {
        let manifest = Manifest {
            content_type: Uuid::nil(),
            root: Some(part),
        };
        self.read_span(&manifest, offset, length).await
    }

    /// Fetches the objects of the manifest tree under a range of the stored form, a level of
    /// nodes at a time.
    async fn fetch_range(
        &self,
        manifest: &Manifest,
        offset: u64,
        length: u64,
    ) -> Result<(), ClientError> {
        loop {
            let missing = self
                .commits
                .vault()
                .missing_objects(manifest, offset, length)?;
            if missing.is_empty() {
                return Ok(());
            }
            self.fetch(&missing).await?;
        }
    }

    /// Every object of a manifest tree, fetching the nodes this peer lacks but not the chunks.
    pub async fn objects_of(&self, manifest: &Manifest) -> Result<Vec<Hash>, ClientError> {
        self.fetch_nodes(manifest).await?;
        Ok(self.commits.vault().objects(manifest)?)
    }

    async fn fetch_nodes(&self, manifest: &Manifest) -> Result<(), ClientError> {
        loop {
            let missing = self.commits.vault().missing_nodes(manifest)?;
            if missing.is_empty() {
                return Ok(());
            }
            self.fetch(&missing).await?;
        }
    }

    /// What a commit of `block` references: `Commit::references` (the content's own and its
    /// metadata's) and then those of the parts of its manifest tree, resolved, each once. Reading
    /// it fetches the commit's nodes, never its chunks.
    pub async fn references_of(
        &self,
        block: Uuid,
        commit: &Commit,
    ) -> Result<Vec<Uuid>, ClientError> {
        self.fetch_nodes(&commit.manifest).await?;
        let parts = self.commits.vault().references(&commit.manifest)?;
        let mut references = commit.references.clone();
        references.extend(self.resolve(block, parts));
        Ok(deduplicated(references))
    }

    /// The objects the server holds for a commit besides the commit itself: its manifest
    /// tree's, and for a merge each other parent and its tree's, as `seal` lists them.
    async fn held_by(&self, commit: &Commit) -> Result<Vec<Hash>, ClientError> {
        let mut held = self.objects_of(&commit.manifest).await?;
        for parent in commit.parents.iter().skip(1) {
            held.push(parent.hash());
            held.extend(
                self.objects_of(&self.load_reached(*parent).await?.manifest)
                    .await?,
            );
        }
        Ok(held)
    }

    pub async fn write_content<'a>(
        &self,
        block: Uuid,
        content_type: Uuid,
        parts: impl Into<Part<'a>>,
    ) -> Result<Manifest, ClientError> {
        let key = self.sealing_key(block).await?;
        self.write_content_under(key, content_type, parts).await
    }

    /// `write_content` for a commit headed for the outbox (`prepare_write`): written here and
    /// uploaded only with that commit, which is judged before it is sent, so offline it takes
    /// the key at hand (`sealing_key_kept`).
    async fn write_content_kept<'a>(
        &self,
        block: Uuid,
        content_type: Uuid,
        parts: impl Into<Part<'a>>,
    ) -> Result<Manifest, ClientError> {
        let key = self.sealing_key_kept(block).await?;
        self.write_content_under(key, content_type, parts).await
    }

    async fn write_content_under<'a>(
        &self,
        key: Uuid,
        content_type: Uuid,
        parts: impl Into<Part<'a>>,
    ) -> Result<Manifest, ClientError> {
        let parts = parts.into();
        let vault = self.commits.vault();
        // A reused part this device never read (one another device put) names its key in its
        // top object alone: fetch those first, so only a part sealed under another key is
        // fetched whole.
        let tops: Vec<Hash> = vault
            .stale_parts(key, &parts)?
            .iter()
            .map(|part| part.hash())
            .collect();
        self.fetch(&tops).await?;
        let stale = vault.stale_parts(key, &parts)?;
        for part in stale {
            let manifest = Manifest {
                content_type,
                root: Some(part),
            };
            self.fetch_range(&manifest, 0, u64::MAX).await?;
        }
        Ok(vault.write(key, content_type, parts)?)
    }

    pub fn account(&self) -> Uuid {
        self.account
    }

    pub fn organization(&self) -> Uuid {
        self.organization
    }

    /// The members no record in the organization's log signed in yet, as key work last listed
    /// them: every such member, to an administrator; this account alone, to a member it names,
    /// whose escrow wraps administrators do not open until one is.
    pub fn unsigned_members(&self) -> Vec<Uuid> {
        self.unsigned.lock().unwrap().clone()
    }

    pub fn role(&self) -> OrganizationRole {
        self.session.lock().unwrap().0
    }

    pub fn token(&self) -> String {
        self.token.lock().unwrap().clone()
    }

    pub fn commits(&self) -> &CommitStore<S> {
        &self.commits
    }

    pub fn connection(&self) -> Arc<Connection> {
        self.connection.lock().unwrap().clone()
    }

    /// Whether `block`'s local edits were discarded and no join has said since that this
    /// account may edit it (`refs::Dropped`): nothing typed into it would be kept or sealed.
    pub fn discarded(&self, block: Uuid) -> bool {
        self.refs.discarded(block)
    }

    /// Every block `discarded` answers for, as the refs database keeps them across restarts.
    pub fn all_discarded(&self) -> Vec<Uuid> {
        self.refs.all_discarded()
    }

    /// Joins `block`'s session (`Connection::join_session`). The server answers whether this
    /// account may edit it: once it may, edits of it are kept again where they were discarded
    /// (`refs::Dropped`), since a listing this device holds may be older than its access.
    pub async fn join_session(
        &self,
        block: Uuid,
        tag: Hash,
    ) -> Result<
        (
            ClientId,
            be_block_protocol::SessionState,
            Option<be_block_protocol::Journal>,
        ),
        ClientError,
    > {
        let (client, state, journal, may_edit) = self.connection().join_session(block, tag).await?;
        if may_edit {
            self.refs.may_edit_again(block)?;
        }
        Ok((client, state, journal))
    }

    /// Whether `descendant` has `ancestor` in its history (a commit is its own ancestor).
    pub async fn descends(
        &self,
        descendant: CommitId,
        ancestor: CommitId,
    ) -> Result<bool, ClientError> {
        self.fetch_history(descendant).await?;
        Ok(self.commits.is_ancestor(ancestor, descendant)?)
    }

    /// Takes `walk` (`MergeBaseWalk`) to where its two heads meet, fetching only the commits it
    /// reaches, a level of both sides in one round of requests.
    pub async fn merge_base(&self, walk: &mut MergeBaseWalk) -> Result<MergeBase, ClientError> {
        loop {
            let frontier: Vec<_> = walk.frontier().map(CommitId::hash).collect();
            self.try_fetch_all(&frontier).await?;
            if let Some(found) = walk.step(&self.commits)? {
                return Ok(found);
            }
        }
    }

    pub fn local_head(&self, block: Uuid) -> Option<CommitId> {
        self.refs.head(block)
    }

    /// For a commit this device holds that sealed a session: the commit it was sealed on and
    /// where in its session it stands (`Commit::sealed_through`).
    pub fn sealed_on(
        &self,
        commit: CommitId,
    ) -> Option<(Option<CommitId>, be_block_commit::SealedThrough)> {
        let commit = self.commits.try_get(commit).ok().flatten()?;
        let through = commit.sealed_through?;
        match commit.parents.as_slice() {
            [] => Some((None, through)),
            [parent] => Some((Some(*parent), through)),
            _ => None,
        }
    }

    /// The blocks the search indexer let go after their commit could not be read, kept on this
    /// device across restarts (guides/blocks/host.md, "Catching up").
    pub fn let_go_blocks(&self) -> Vec<(Uuid, Option<ListedBlock>)> {
        self.refs.let_go_blocks()
    }

    /// Keeps `block` as one the search indexer let go, with what a catch-up listed of it.
    pub fn let_go(&self, block: Uuid, listed: Option<ListedBlock>) -> Result<(), ClientError> {
        Ok(self.refs.let_go(block, listed)?)
    }

    /// Forgets `block` as one the search indexer let go: it was read, or is no longer indexed.
    pub fn forget_let_go(&self, block: Uuid) -> Result<(), ClientError> {
        Ok(self.refs.forget_let_go(block)?)
    }

    pub fn known_remote_head(&self, block: Uuid) -> Option<CommitId> {
        self.refs.remote(block)
    }

    pub fn remember(&self, block: Uuid, head: CommitId) -> Result<(), ClientError> {
        Ok(self.refs.set_head(block, head)?)
    }

    pub async fn create<C: BlockContent>(&self, parent: BlockParent) -> Result<Uuid, ClientError> {
        self.create_with_id::<C>(Uuid::now_v7(), parent).await
    }

    pub async fn create_with_id<C: BlockContent>(
        &self,
        block: Uuid,
        parent: BlockParent,
    ) -> Result<Uuid, ClientError> {
        self.create_block(
            block,
            C::CONTENT_TYPE,
            parent,
            &BlockMetadata::default(),
            false,
        )
        .await
        .map(|summary| summary.id)
    }

    /// Makes a block. With `owner_only`, only this account reads it, on any of its devices: its
    /// own scope root, whose key is sealed to this account alone and never escrowed
    /// (`KindVisibility::Owner`), made only in this account's own settings.
    pub async fn create_block(
        &self,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
        metadata: &BlockMetadata,
        owner_only: bool,
    ) -> Result<BlockSummary, ClientError> {
        // A local account makes the block here (`keep_creation`).
        if self.local {
            return self
                .keep_creation(block, content_type, parent, metadata, owner_only)
                .await;
        }
        let creation = self
            .creation(block, content_type, parent, metadata, owner_only)
            .await?;
        // A parent whose creation waits in the outbox is not on the server yet: this one waits
        // behind it.
        if let BlockParent::Block(above) = parent
            && self.refs.creating(above)
        {
            return self.create_locally(creation);
        }
        let response = crate::ask(client::blocks::create_block(
            &self.connection(),
            block,
            content_type,
            parent,
            creation.metadata.clone(),
            creation.references.clone(),
            creation.key,
            creation.keys.clone(),
            owner_only,
            creation.root.clone(),
        ))
        .await;
        let response = match response {
            Err(error) if error.is_unreachable() => return self.create_locally(creation),
            response => response?,
        };
        self.recorded(&creation.keys);
        self.note(&response);
        self.escrow_created(&creation.keys).await;
        Ok(response)
    }

    /// Makes a block on this device and keeps its creation in the outbox, for the drain to send
    /// in order with what comes after it (guides/blocks/client.md, "Saving and the outbox").
    pub async fn keep_creation(
        &self,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
        metadata: &BlockMetadata,
        owner_only: bool,
    ) -> Result<BlockSummary, ClientError> {
        // A local account refuses a second block under an id it holds, as a server does.
        if self.local && self.refs.summary(block).is_some() {
            return Err(ClientError::Refused(
                ErrorCode::BlockAlreadyExists,
                format!("block {block} already exists"),
            ));
        }
        let creation = self
            .creation(block, content_type, parent, metadata, owner_only)
            .await?;
        self.create_locally(creation)
    }

    async fn creation(
        &self,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
        metadata: &BlockMetadata,
        owner_only: bool,
    ) -> Result<Creation, ClientError> {
        let root = self.new_root(block);
        let (key, keys) = self.new_keys(block, parent, owner_only).await?;
        let references = bounded(deduplicated(self.resolve(block, metadata.references())))?;
        let metadata = self.commits.vault().seal(key, &metadata.encode())?;
        Ok(Creation {
            block,
            content_type,
            parent,
            metadata,
            references,
            key,
            keys,
            owner_only,
            root,
        })
    }

    /// The root of a new block's access log, signed by this account and pinned here before the
    /// block's keys are made, which carry it.
    pub(crate) fn new_root(&self, block: Uuid) -> Vec<u8> {
        let root = be_block_keys::Entry::root(
            self.keys.identity(),
            self.keys.account().into_bytes(),
            be_block_keys::LogKind::Block,
            block.into_bytes(),
        );
        self.keys.pin_root(block, root.hash());
        let kept = self.keys.keep_made(block, root.hash());
        let _ = self.refs.keep_log(block, kept);
        root.encode()
    }

    /// The block's access log, root first, once it checks from the root this device pinned
    /// (`KeyBook::root`: from making the block, or carried by a trusted key of it) to its head.
    /// A log rooted anywhere else is the server's, and refused (`ClientError::Forged`).
    pub async fn access_log(&self, block: Uuid) -> Result<Vec<be_block_keys::Entry>, ClientError> {
        Ok(self.checked_access(block).await?.0)
    }

    /// The block's log as `access_log` checks it, and what its records give.
    pub async fn checked_access(
        &self,
        block: Uuid,
    ) -> Result<(Vec<be_block_keys::Entry>, be_block_keys::Checked), ClientError> {
        let pinned = match self.keys.root_pin(block) {
            // A weak root checks what the member who sent it says; `root_pin` tells which.
            crate::keys::RootPin::Pinned(root) | crate::keys::RootPin::Weak(root) => root,
            crate::keys::RootPin::Conflicting => return Err(ClientError::Forged(block)),
            crate::keys::RootPin::Unknown => return Err(ClientError::Locked(block)),
        };
        let entries = self.served_log(block).await?;
        let cites = entries.iter().any(|entry| {
            entry
                .record
                .authority()
                .is_some_and(|authority| authority != be_block_keys::Authority::Log)
        });
        let around = match cites {
            true => self.logs_around(block).await?,
            false => Around::default(),
        };
        let checked = around
            .check(self.organization, block, pinned, &entries)
            .ok_or(ClientError::Forged(block))?;
        // A log short of what an earlier check kept is withheld: nothing acts on it, and what
        // waits on it (a seal to judge, an escrowed key to take) waits for a whole one.
        match self.keys.served(block, &checked) {
            crate::keys::Served::Take => {}
            // An older answer than one this run already took: that one stands, with its
            // entries, so a record signed on it names the newer head.
            crate::keys::Served::Stale(newer) => {
                let (checked, entries) = *newer;
                return Ok((entries, checked));
            }
            crate::keys::Served::Withheld => return Err(ClientError::Withheld(block)),
        }
        if let Some(kept) = self.keys.note_log(block, &checked, &entries) {
            let _ = self.refs.keep_log(block, kept);
        }
        Ok((entries, checked))
    }

    async fn served_log(&self, block: Uuid) -> Result<Vec<be_block_keys::Entry>, ClientError> {
        let served = crate::ask(client::blocks::get_access_log(&self.connection(), block)).await?;
        served
            .iter()
            .map(|entry| be_block_keys::Entry::decode(entry))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ClientError::Forged(block))
    }

    /// The logs a grant in `block`'s log may name its authority in: the organization's, checked
    /// from the root this account pinned, and those of the blocks above it, in the server's tree
    /// (trusted for which block is above which until parents are signed). An ancestor's log is
    /// checked from the root this device pinned for it, or, for one it pinned none for, from the
    /// root the server serves; one whose served root conflicts with its pin is left out.
    async fn logs_around(&self, block: Uuid) -> Result<Around, ClientError> {
        let mut around = Around {
            block,
            ..Around::default()
        };
        if let Some(pinned) = self.keys.organization_root() {
            let entries = self.organization_log().await?;
            around.held.insert(
                (be_block_keys::LogKind::Organization, self.organization),
                (pinned, entries),
            );
        }
        let mut at = block;
        while let Ok(summary) = self.summary(at).await
            && let Some(BlockParent::Block(parent)) = summary.parent
        {
            around.above.push(parent);
            at = parent;
            let Ok(entries) = self.served_log(parent).await else {
                continue;
            };
            let pinned = match (self.keys.root_pin(parent), entries.first()) {
                (crate::keys::RootPin::Pinned(root) | crate::keys::RootPin::Weak(root), _) => root,
                (crate::keys::RootPin::Unknown, Some(root)) => root.hash(),
                _ => continue,
            };
            around
                .held
                .insert((be_block_keys::LogKind::Block, parent), (pinned, entries));
        }
        Ok(around)
    }

    /// `account`'s keys as this device verified them: from a contact card or a key change.
    pub fn verified_identity(&self, account: Uuid) -> Option<be_block_keys::PublicIdentity> {
        let public = self.keys.contact(account)?;
        let signing = self
            .refs
            .signers(account)
            .into_iter()
            .rev()
            .find(|signer| signer.verified)?;
        Some(be_block_keys::PublicIdentity {
            public,
            signing: be_block_keys::SigningPublic(signing.key),
        })
    }

    /// The grant record `grant` sends: `None` for the standing grant to the server and for
    /// revoking one the log does not name; for anyone inside, an opening (`Record::Opening`).
    /// It names the keys this device verified for `account`, or those the log already names for
    /// it, and where this device's Manage comes from: the block's log, the organization's
    /// (administering it), or a block above.
    async fn signed_grant(
        &self,
        block: Uuid,
        account: Uuid,
        access: Access,
        given: Option<be_block_keys::PublicIdentity>,
    ) -> Result<Option<Vec<u8>>, ClientError> {
        use be_block_keys::{Authority, Grant, LogKind, Opening, Record};
        if self
            .keys
            .server()
            .is_some_and(|(server, _)| server == account)
        {
            return Ok(None);
        }
        let level = grant_level(access);
        let (entries, checked) = self.checked_access(block).await?;
        let me = self.keys.account().into_bytes();
        let manages = checked.manager(LogKind::Block, me).is_some();
        let authority = |own: bool| async move {
            match own || manages {
                true => Ok(Authority::Log),
                false => self
                    .logs_around(block)
                    .await?
                    .authority(self.organization, me)
                    .ok_or(ClientError::Refused(
                        ErrorCode::PermissionDenied,
                        format!("this account does not manage block {block}"),
                    )),
            }
        };
        let sign = |record: Record| {
            be_block_keys::Entry::sign(
                self.keys.identity(),
                me,
                block.into_bytes(),
                entries.last().map(be_block_keys::Entry::hash),
                record,
            )
            .encode()
        };
        if account == be_block_graph::ANYONE_INSIDE {
            if level == be_block_keys::Level::None
                && checked.open_inside == be_block_keys::Level::None
            {
                return Ok(None);
            }
            let authority = authority(false).await?;
            return Ok(Some(sign(Record::Opening(Opening { level, authority }))));
        }
        let named = checked.grants.get(&account.into_bytes()).copied();
        if level == be_block_keys::Level::None
            && named.is_none_or(|(named, _)| named == be_block_keys::Level::None)
        {
            return Ok(None);
        }
        let identity = given
            .or_else(|| self.verified_identity(account))
            .or(named.map(|(_, identity)| identity))
            .ok_or_else(|| {
                ClientError::Refused(
                    ErrorCode::NotAContact,
                    "share only with your contacts: send them a contact link first".into(),
                )
            })?;
        let own = account.into_bytes() == me && level == be_block_keys::Level::None;
        let authority = authority(own).await?;
        Ok(Some(sign(Record::Grant(Grant {
            grantee: account.into_bytes(),
            identity,
            level,
            authority,
        }))))
    }

    /// Defines a block's metadata on this device and keeps the change in the outbox: a block
    /// with no content yet gets its metadata set, one with content a commit carrying it, both
    /// sent by the drain in order.
    pub async fn keep_definition(
        &self,
        block: Uuid,
        edit: impl Fn(&mut BlockMetadata),
    ) -> Result<BlockSummary, ClientError> {
        let summary = match self.refs.summary(block) {
            Some(summary) => self.refs.current(vec![summary]).remove(0),
            None => self.summary(block).await?,
        };
        let head = self.local_head(block).or(summary.head);
        let current = match head {
            Some(head) => self.metadata_of(block, head).await?,
            None => self.metadata_of_summary(&summary).await?,
        };
        let mut next = current.clone();
        edit(&mut next);
        let next = current.with_defined(next.defined);
        if next == current {
            return Ok(summary);
        }
        let Some(head) = head else {
            let sealed = self.seal_metadata(block, &next).await?;
            let references = bounded(deduplicated(self.resolve(block, next.references())))?;
            return self.define_locally(summary, sealed, references);
        };
        let commit = self.load_head(block, head).await?;
        let described = self.resolve(block, current.references());
        let mut references = commit.references;
        references.retain(|reference| !described.contains(reference));
        let write = Write::written(commit.manifest, Some(head))
            .described(references, next.derived)
            .defined(next.defined);
        self.seal(block, write).await?;
        Ok(self.refs.summary(block).unwrap_or(summary))
    }

    pub async fn define(
        &self,
        block: Uuid,
        edit: impl Fn(&mut BlockMetadata),
    ) -> Result<BlockSummary, ClientError> {
        for _ in 0..8 {
            let summary = self.summary(block).await?;
            let head = match self.pending_commits(block).is_empty() {
                true => summary.head,
                false => self.local_head(block).or(summary.head),
            };
            let current = match head {
                Some(head) => self.metadata_of(block, head).await?,
                None => self.metadata_of_summary(&summary).await?,
            };
            let mut next = current.clone();
            edit(&mut next);
            let next = current.with_defined(next.defined);
            if next == current {
                return Ok(summary);
            }
            let Some(head) = head else {
                let sealed = self.seal_metadata(block, &next).await?;
                let references = bounded(deduplicated(self.resolve(block, next.references())))?;
                if !self.is_attached() || self.refs.has_structural(block) {
                    return self.define_locally(summary, sealed, references);
                }
                let response = crate::ask(client::blocks::set_metadata(
                    &self.connection(),
                    block,
                    sealed.clone(),
                    references.clone(),
                ))
                .await;
                match response {
                    Ok(block) => return Ok(block),
                    Err(ClientError::Refused(ErrorCode::BlockHasContent, _)) => continue,
                    Err(error) if offline::unreachable(&error) => {
                        return self.define_locally(summary, sealed, references);
                    }
                    Err(error) => return Err(error),
                }
            };
            let commit = self.load_head(block, head).await?;
            let described = self.resolve(block, current.references());
            let mut references = commit.references;
            references.retain(|reference| !described.contains(reference));
            let write = Write::written(commit.manifest, Some(head))
                .described(references, next.derived)
                .defined(next.defined);
            let saved = self.write(block, write).await;
            let saved = match saved {
                Err(error)
                    if offline::unreachable(&error) && self.local_head(block) != Some(head) =>
                {
                    return Ok(self.refs.summary(block).unwrap_or(summary));
                }
                saved => saved?,
            };
            if saved.published().is_some() {
                return self.summary(block).await;
            }
        }
        Err(ClientError::Contended)
    }

    /// The metadata of `block`'s head `commit` (`load_head`).
    pub async fn metadata_of(
        &self,
        block: Uuid,
        commit: CommitId,
    ) -> Result<BlockMetadata, ClientError> {
        Ok(BlockMetadata::decode(
            &self.load_head(block, commit).await?.metadata,
        )?)
    }

    pub async fn metadata_of_summary(
        &self,
        summary: &BlockSummary,
    ) -> Result<BlockMetadata, ClientError> {
        self.retrying(|| self.open_metadata(summary)).await
    }

    async fn metadata_at(
        &self,
        block: Uuid,
        expected: Option<CommitId>,
    ) -> Result<BlockMetadata, ClientError> {
        match expected {
            Some(head) => self.base_metadata(block, head).await,
            None => self.metadata_of_summary(&self.summary(block).await?).await,
        }
    }

    /// The metadata of `head`, a commit `block`'s next commit goes on: from the commit when this
    /// device holds it, else from the block's summary when that names `head` and nothing sealed
    /// here since has revised it (its sealed metadata is then the head's own, as its publisher
    /// sealed it), else fetched. A device that knows a block's head only from its summary can
    /// seal on it offline.
    async fn base_metadata(
        &self,
        block: Uuid,
        head: CommitId,
    ) -> Result<BlockMetadata, ClientError> {
        if let Some(commit) = self.commits.try_get(head)? {
            self.sealed_in(block, head)?;
            return Ok(BlockMetadata::decode(&commit.metadata)?);
        }
        if self.pending_commits(block).is_empty()
            && let Some(summary) = self.refs.summary(block)
            && summary.head == Some(head)
            && let Ok(metadata) = self.open_metadata(&summary)
        {
            return Ok(metadata);
        }
        self.metadata_of(block, head).await
    }

    pub fn metadata(&self, summary: &BlockSummary) -> Result<BlockMetadata, ClientError> {
        self.open_metadata(summary).map_err(locked)
    }

    /// A summary's metadata, which the server names: refused unless one of the summary's
    /// block's keys sealed it, as for a head (`load_head`).
    fn open_metadata(&self, summary: &BlockSummary) -> Result<BlockMetadata, ClientError> {
        if summary.metadata.is_empty() {
            return Ok(BlockMetadata::default());
        }
        let plain = self.open_for(summary.id, &summary.metadata)?;
        Ok(BlockMetadata::decode(&plain)?)
    }

    async fn seal_metadata(
        &self,
        block: Uuid,
        metadata: &BlockMetadata,
    ) -> Result<Vec<u8>, ClientError> {
        let key = self.sealing_key(block).await?;
        Ok(self.commits.vault().seal(key, &metadata.encode())?)
    }

    pub async fn ensure<C: BlockContent>(
        &self,
        block: Uuid,
        parent: BlockParent,
    ) -> Result<(), ClientError> {
        match self.create_with_id::<C>(block, parent).await {
            Ok(_) => Ok(()),
            Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub async fn ensure_type(
        &self,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
    ) -> Result<(), ClientError> {
        let known = self.refs.summary(block);
        if known.is_some_and(|summary| summary.content_type == content_type || !self.is_attached())
        {
            return Ok(());
        }
        match self
            .create_block(
                block,
                content_type,
                parent,
                &BlockMetadata::default(),
                false,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub async fn summary(&self, block: Uuid) -> Result<BlockSummary, ClientError> {
        // A local account holds every block it has: one it does not hold is not there, as a
        // server answers of a block it does not know.
        if self.local {
            return match self.refs.summary(block) {
                Some(summary) => Ok(self.refs.current(vec![summary]).remove(0)),
                None => Err(ClientError::Refused(
                    ErrorCode::BlockNotFound,
                    format!("block {block} is not on this device"),
                )),
            };
        }
        // The server does not know a block whose creation waits in the outbox, or was refused.
        if self.uncreated(block)
            && let Some(summary) = self.refs.summary(block)
        {
            return Ok(self.refs.current(vec![summary]).remove(0));
        }
        self.server_summary(block).await
    }

    /// What the server says of a block, or offline what this device last heard.
    pub(super) async fn server_summary(&self, block: Uuid) -> Result<BlockSummary, ClientError> {
        let response = crate::ask(client::blocks::list_blocks(
            &self.connection(),
            Listing::Block(block),
            None,
        ))
        .await
        .and_then(|listed| {
            listed
                .blocks
                .into_iter()
                .next()
                .ok_or(ClientError::Unexpected)
        });
        match response {
            Ok(block) => {
                self.note(&block);
                Ok(self.summary_with_outbox(block))
            }
            Err(error) if error.is_unreachable() => match self.refs.summary(block) {
                Some(summary) => Ok(self.refs.current(vec![summary]).remove(0)),
                None => Err(error),
            },
            Err(ClientError::Refused(code, message)) => {
                if offline::lost_access(code)
                    && !self.refs.creating(block)
                    && self.refs.refused(block).is_none()
                {
                    let _ = self.refs.lose_access(block);
                    let _ = self.move_to_unsent(block, None);
                }
                Err(ClientError::Refused(code, message))
            }
            Err(error) => Err(error),
        }
    }

    /// The blocks of `listing`, which this connection follows from then on (`unlist` stops
    /// it); offline, what this device last heard, and for a local account what it holds.
    pub async fn list(&self, listing: Listing) -> Result<Vec<BlockSummary>, ClientError> {
        if self.local {
            return Ok(self.refs.current(self.refs.local_listing(listing)));
        }
        if let Some(block) = about(listing)
            && self.refs.creating(block)
        {
            return Ok(self.listed_here(listing, block));
        }
        match self.list_online(listing).await {
            Ok((revision, blocks)) => {
                self.observe(&blocks);
                let blocks = blocks
                    .into_iter()
                    .map(|summary| self.summary_with_outbox(summary))
                    .collect();
                let blocks = self.with_outbox(listing, blocks);
                let _ = self.refs.listed(listing, &blocks, revision);
                Ok(blocks)
            }
            Err(error) if error.is_unreachable() => match self.refs.cached_listing(listing) {
                Some(blocks) => Ok(self.refs.current(blocks)),
                None => Err(error),
            },
            Err(error) => Err(error),
        }
    }

    /// The server's answer to `listing`, every page of it (in the order of block ids, each
    /// after the last), with the summaries of each page's `rest` read after it. Summaries read
    /// after the first page are newer than the listing's revision, so one the listing would no
    /// longer hold is left out (`still_in`). A block's references are put in its own order.
    async fn list_online(&self, listing: Listing) -> Result<(u64, Vec<BlockSummary>), ClientError> {
        let mut first_revision = None;
        let mut last_revision;
        let mut after = None;
        // The listing in order: a summary given, or an id whose summary is read below.
        let mut entries: Vec<Result<BlockSummary, Uuid>> = Vec::new();
        loop {
            let page = crate::ask(client::blocks::list_blocks(
                &self.connection(),
                listing,
                after,
            ))
            .await?;
            let later = first_revision.is_some();
            first_revision.get_or_insert(page.revision);
            last_revision = page.revision;
            entries.extend(
                page.blocks
                    .into_iter()
                    .filter(|summary| !later || still_in(listing, summary))
                    .map(Ok),
            );
            entries.extend(page.rest.into_iter().map(Err));
            match page.next {
                // Pages go in the order of block ids, so each starts past the last.
                Some(next) if after.is_none_or(|after| next > after) => after = Some(next),
                Some(_) => return Err(ClientError::Unexpected),
                None => break,
            }
        }
        let ids: Vec<Uuid> = entries
            .iter()
            .filter_map(|entry| entry.as_ref().err().copied())
            .collect();
        let mut read: HashMap<Uuid, BlockSummary> = self
            .read_summaries(&ids)
            .await?
            .into_iter()
            .filter(|summary| still_in(listing, summary))
            .map(|summary| (summary.id, summary))
            .collect();
        let mut seen = HashSet::new();
        let mut blocks: Vec<BlockSummary> = entries
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(summary) => Some(summary),
                Err(id) => read.remove(&id),
            })
            .filter(|summary| seen.insert(summary.id))
            .collect();
        // A block's references come in the order of their ids; the block's own summary, read
        // without listing it, gives their order. Read after the pages, it is as new as any of
        // them, so a block it no longer references has left the listing. (A summary older than
        // the pages, as one kept offline would be, could miss a reference newly made: such
        // blocks would go to the end rather than be dropped.)
        if let Listing::References(referrer) = listing
            && blocks.len() > 1
            && let Some(summary) = self.read_summaries(&[referrer]).await?.pop()
        {
            let order: HashMap<Uuid, usize> = summary
                .references
                .iter()
                .enumerate()
                .map(|(index, reference)| (*reference, index))
                .collect();
            if summary.revision >= last_revision {
                blocks.retain(|block| order.contains_key(&block.id));
            }
            blocks.sort_by_key(|block| order.get(&block.id).copied().unwrap_or(usize::MAX));
        }
        Ok((first_revision.unwrap_or_default(), blocks))
    }

    /// The summaries of `blocks` the server still lets this device know of, read
    /// `MAX_SUMMARIES_ASKED` at a time with every group asked at once, and the rest of a group
    /// asked again when an answer holds only its first.
    async fn read_summaries(&self, blocks: &[Uuid]) -> Result<Vec<BlockSummary>, ClientError> {
        let ask =
            |group: &[Uuid]| client::blocks::read_summaries(&self.connection(), group.to_vec());
        let asked: Vec<_> = blocks
            .chunks(be_block_protocol::MAX_SUMMARIES_ASKED)
            .map(|group| (group, ask(group)))
            .collect();
        let mut summaries = Vec::with_capacity(blocks.len());
        for (mut group, mut answer) in asked {
            loop {
                let read = crate::ask(answer).await?;
                if read.is_empty() || read.len() > group.len() {
                    return Err(ClientError::Unexpected);
                }
                group = &group[read.len()..];
                summaries.extend(read.into_iter().flatten());
                if group.is_empty() {
                    break;
                }
                answer = ask(group);
            }
        }
        Ok(summaries)
    }

    /// A listing about a block whose creation waits in the outbox, which the server cannot
    /// answer yet: what this device has. What is in force at the block before it is made is
    /// what is in force at its parent.
    fn listed_here(&self, listing: Listing, block: Uuid) -> Vec<BlockSummary> {
        if let Some(cached) = self.refs.cached_listing(listing) {
            return self.refs.current(self.with_outbox(listing, cached));
        }
        let parent = self
            .refs
            .summary(block)
            .and_then(|summary| summary.parent)
            .and_then(|parent| parent.block());
        let blocks = match (listing, parent) {
            (Listing::Block(_), _) => self.refs.summary(block).into_iter().collect(),
            (Listing::OpenSettings(_), Some(parent)) => self
                .refs
                .cached_listing(Listing::OpenSettings(parent))
                .map(|cached| self.refs.current(cached))
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        // As of the newest revision this device knows, as every offline answer is: a block
        // made here has none of its own yet.
        self.refs.current(self.with_outbox(listing, blocks))
    }

    /// A summary the server answered, as this device has it: where a move waiting in the
    /// outbox puts the block, and the metadata a waiting definition gives it
    /// (`RefsDb::with_outbox`).
    pub fn summary_with_outbox(&self, summary: BlockSummary) -> BlockSummary {
        self.refs.with_outbox(summary)
    }

    /// A listing the server answered, as this device has it: what a creation or move waiting in
    /// the outbox puts in it or takes out.
    pub fn with_outbox(
        &self,
        listing: Listing,
        mut blocks: Vec<BlockSummary>,
    ) -> Vec<BlockSummary> {
        for (_, op) in self.refs.outbox() {
            let (block, parent) = match op {
                Op::Create(creation) => (creation.block, creation.parent),
                Op::Move(moving) => (moving.block, moving.parent),
                Op::Publish(_) | Op::Define(_) | Op::Hold(_) => continue,
            };
            let here = match listing {
                Listing::Children(under) => under == parent,
                Listing::Deleted => parent == BlockParent::Detached,
                _ => continue,
            };
            blocks.retain(|summary| summary.id != block);
            if here && let Some(summary) = self.refs.summary(block) {
                blocks.push(summary);
            }
        }
        blocks
    }

    pub async fn remote_head(&self, block: Uuid) -> Result<Option<CommitId>, ClientError> {
        Ok(self.summary(block).await?.head)
    }

    pub async fn newest_head(&self, block: Uuid) -> Result<Option<CommitId>, ClientError> {
        if !self.pending_commits(block).is_empty()
            && let Some(head) = self.local_head(block)
        {
            return Ok(Some(head));
        }
        self.remote_head(block).await
    }

    /// Anything that asks history a question (`resume`, `common_ancestor`) needs this first, or
    /// an unfetched ancestor looks like a divergence.
    pub async fn fetch_history(&self, head: CommitId) -> Result<(), ClientError> {
        let mut frontier = vec![head];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = frontier.pop() {
            if !seen.insert(id) {
                continue;
            }
            if !self.try_fetch(id.hash()).await? {
                continue;
            }
            if let Some(commit) = self.commits.try_get(id)? {
                frontier.extend(commit.parents);
            }
        }
        Ok(())
    }

    /// A commit reached by hash from one already checked (a parent, a repository's history):
    /// bound by that hash, whichever key sealed it. A block's head, which the server names,
    /// goes through `load_head`.
    pub async fn load_reached(&self, commit: CommitId) -> Result<Commit, ClientError> {
        self.fetch(&[commit.hash()]).await?;
        self.retrying(|| Ok(self.commits.get(commit)?)).await
    }

    /// `commit` as `block`'s head, which the server names: refused unless one of `block`'s keys
    /// sealed it (`sealed_in`), since the server could name another block's commit, which a
    /// device would show as this block's and write back under this block's key.
    pub async fn load_head(&self, block: Uuid, commit: CommitId) -> Result<Commit, ClientError> {
        let loaded = self.load_reached(commit).await?;
        self.sealed_in(block, commit)?;
        Ok(loaded)
    }

    /// Whether `commit`, held, was sealed under one of `block`'s keys, as this device placed
    /// them (`KeyBook::block_of`); a commit's subtree is sealed under its key.
    pub fn sealed_in(&self, block: Uuid, commit: CommitId) -> Result<(), ClientError> {
        let key = self.commits.vault().sealed_under(commit.hash())?;
        match self.keys.block_of(key) == Some(block) {
            true => Ok(()),
            false => Err(ClientError::Forged(block)),
        }
    }

    /// Each block's summary, the content of its head and its journal: what its session's owner
    /// sequenced since its base, opened, for a reader to apply with the block's type
    /// (`be_block_type_host::Replicas::read_contents`). The journal is the server's, or offline
    /// this device's.
    pub async fn read_contents(
        &self,
        blocks: &[(Uuid, Option<CommitId>)],
    ) -> Result<Vec<BlockRead>, ClientError> {
        match self.read_contents_online(blocks).await {
            Err(ClientError::Disconnected(reason)) => {
                self.read_contents_cached(blocks, reason).await
            }
            read => read,
        }
    }

    async fn read_contents_online(
        &self,
        blocks: &[(Uuid, Option<CommitId>)],
    ) -> Result<Vec<BlockRead>, ClientError> {
        let ask = |group: &[(Uuid, Option<CommitId>)]| {
            let ids: Vec<Uuid> = group.iter().map(|(id, _)| *id).collect();
            let held = group
                .iter()
                .filter_map(|(_, head)| *head)
                .filter(|head| self.holds(*head))
                .collect();
            client::blocks::read_contents(&self.connection(), ids, held)
        };
        let asked: Vec<_> = blocks
            .chunks(CONTENTS_BLOCKS)
            .map(|group| (group, ask(group)))
            .collect();
        let mut summaries = Vec::with_capacity(blocks.len());
        for (mut group, mut answer) in asked {
            loop {
                let be_block_protocol::Contents {
                    blocks,
                    objects,
                    journals,
                } = answer.await??;
                // The server answers the first of the blocks asked whose summaries fit in its
                // answer, at least one; the rest are asked for again.
                if blocks.is_empty()
                    || blocks.len() > group.len()
                    || !blocks
                        .iter()
                        .zip(group)
                        .all(|(summary, (id, _))| summary.id == *id)
                    || journals.len() != blocks.len()
                {
                    return Err(ClientError::Unexpected);
                }
                let store = self.commits.vault().store();
                for object in &objects {
                    store.put(object)?;
                }
                for summary in &blocks {
                    self.note(summary);
                }
                group = &group[blocks.len()..];
                let blocks = blocks
                    .into_iter()
                    .map(|summary| self.summary_with_outbox(summary));
                summaries.extend(blocks.zip(journals));
                if group.is_empty() {
                    break;
                }
                answer = ask(group);
            }
        }
        futures_util::future::try_join_all(summaries.into_iter().map(
            |(summary, journal)| async move {
                let stored = match summary.head {
                    Some(head) => {
                        let commit = self.load_head(summary.id, head).await?;
                        let stored = self.load_manifest(&commit.manifest).await?;
                        self.read_whole(summary.id, head);
                        Some(stored)
                    }
                    None => None,
                };
                let journal =
                    journal.and_then(|journal| self.open_journal(summary.id, journal).ok());
                Ok(BlockRead {
                    summary,
                    stored,
                    journal,
                })
            },
        ))
        .await
    }

    /// Records that this device has read a block's content at `head` whole, so it can read it
    /// again offline (`held_head`).
    pub fn read_whole(&self, block: Uuid, head: CommitId) {
        if self.holds(head) {
            let _ = self.refs.hold(block, head);
        }
    }

    /// The newest head of a block whose content this device holds whole, for reading it
    /// offline: its own commit, else what the server last said, else what it last read whole. A
    /// head the server moved to while this device was not reading the block is not held.
    pub fn held_head(&self, block: Uuid) -> Option<CommitId> {
        [
            self.local_head(block),
            self.known_remote_head(block),
            self.refs.summary(block).and_then(|summary| summary.head),
            self.refs.held(block),
        ]
        .into_iter()
        .flatten()
        .find(|head| self.holds(*head))
    }

    fn holds(&self, head: CommitId) -> bool {
        if self.whole.lock().unwrap().contains(&head) {
            return true;
        }
        let vault = self.commits.vault();
        let whole = matches!(
            self.commits.try_get(head),
            Ok(Some(commit)) if vault.holds_all(&commit.manifest).unwrap_or(false)
        );
        if whole {
            self.whole.lock().unwrap().insert(head);
        }
        whole
    }

    pub async fn open<C: BlockContent>(&self, block: Uuid) -> Result<Option<C>, ClientError> {
        if let Some(head) = self.local_head(block)
            && !self.pending_commits(block).is_empty()
        {
            return Ok(Some(C::from_stored(&self.read_at(block, head).await?)?));
        }
        let read = self
            .read_contents(&[(block, self.local_head(block))])
            .await?;
        let Some(BlockRead {
            summary,
            stored: Some(stored),
            ..
        }) = read.into_iter().next()
        else {
            return Ok(None);
        };
        if let Some(head) = summary.head {
            self.remember(block, head)?;
        }
        Ok(Some(C::from_stored(&stored)?))
    }

    /// `block`'s content at `commit`, which the server may have named (`load_head`).
    pub async fn open_commit<C: BlockContent>(
        &self,
        block: Uuid,
        commit: CommitId,
    ) -> Result<C, ClientError> {
        let commit = self.load_head(block, commit).await?;
        Ok(C::from_stored(
            &self.load_manifest(&commit.manifest).await?,
        )?)
    }

    pub async fn save<C: BlockContent>(
        &self,
        block: Uuid,
        content: &C,
        expected: Option<CommitId>,
    ) -> Result<Saved, ClientError> {
        self.write(block, self.content_write(content, expected))
            .await
    }

    pub async fn bookmark<C: BlockContent>(
        &self,
        block: Uuid,
        content: &C,
        expected: Option<CommitId>,
        label: &str,
    ) -> Result<Saved, ClientError> {
        let write = self.content_write(content, expected);
        self.write(block, write.bookmark(label)).await
    }

    fn content_write<'a, C: BlockContent>(
        &self,
        content: &'a C,
        expected: Option<CommitId>,
    ) -> Write<'a> {
        Write::encoded(C::CONTENT_TYPE, content.parts(), expected).described(
            content.references_in(self.organization()),
            derived_metadata(content),
        )
    }

    /// Seals the write and publishes it, or offline keeps it in the outbox (`Saved::Kept`):
    /// `seal` then `publish_sealed`. `Kept` when the commit waits in the outbox behind what goes
    /// first. On `Rejected` it drops the commit from the outbox, since the caller retries or
    /// merges.
    pub async fn write(&self, block: Uuid, write: Write<'_>) -> Result<Saved, ClientError> {
        let before = self.local_head(block);
        let commit = match self.seal(block, write).await? {
            Sealed::Unchanged(head) => return Ok(Saved::Unchanged(head)),
            Sealed::Sealed(commit) => commit,
        };
        if !self.is_attached() && !self.local {
            return Ok(Saved::Kept(commit));
        }
        let saved = match self.publish_sealed(block, commit).await {
            Err(error) if offline::unreachable(&error) => return Ok(Saved::Kept(commit)),
            saved => saved?,
        };
        if let Saved::Rejected { .. } = saved {
            self.withdraw(block, commit, before)?;
        }
        Ok(saved)
    }

    async fn prepare_write(
        &self,
        block: Uuid,
        write: &mut Write<'_>,
    ) -> Result<(Manifest, Vec<CommitId>, BlockMetadata), ClientError> {
        let content = std::mem::replace(&mut write.content, Content::Written(Manifest::default()));
        let manifest = match content {
            Content::Encoded(content_type, parts) => {
                self.write_content_kept(block, content_type, parts).await?
            }
            Content::Written(manifest) => {
                let parts = manifest
                    .root
                    .map_or_else(|| Part::from(Vec::new()), Part::Reused);
                self.write_content_kept(block, manifest.content_type, parts)
                    .await?
            }
        };
        let mut parents = Vec::new();
        if let Some((base, ours)) = write.merging {
            let mut sides = Vec::new();
            for commit in [base, Some(ours), write.expected] {
                sides.push(match commit {
                    Some(commit) => self.metadata_of(block, commit).await?,
                    None => BlockMetadata::default(),
                });
            }
            write.defined = Some(BlockMetadata::merge_defined(&sides[0], &sides[2], &sides[1]).0);
            parents.push(ours);
        }
        let defined = match write.defined.take() {
            Some(defined) => defined,
            None => self.metadata_at(block, write.expected).await?.defined,
        };
        let derived = std::mem::take(&mut write.derived);
        let mut references = self.resolve(block, std::mem::take(&mut write.references));
        let metadata = BlockMetadata { defined, derived };
        references.extend(self.resolve(block, metadata.references()));
        write.references = bounded(deduplicated(references))?;
        Ok((manifest, parents, metadata))
    }

    pub async fn stash(&self, block: Uuid, mut write: Write<'_>) -> Result<CommitId, ClientError> {
        let (manifest, _, metadata) = self.prepare_write(block, &mut write).await?;
        let key = self.sealing_key(block).await?;
        let parent = write
            .expected
            .and_then(|expected| self.commits.get(expected).ok());
        let commit = Commit::new(manifest, self.account, self.commit_time(parent.as_ref()))
            .with_parent(write.expected)
            .with_references(write.references)
            .with_metadata(metadata.encode());
        let id = self.commits.put(key, &commit)?;
        let mut objects = vec![id.hash()];
        objects.extend(self.objects_of(&commit.manifest).await?);
        match self.push(&objects).await {
            Err(error) if offline::unreachable(&error) => self.commits.vault().store().sync()?,
            pushed => pushed?,
        }
        Ok(id)
    }

    pub async fn seal(&self, block: Uuid, mut write: Write<'_>) -> Result<Sealed, ClientError> {
        // Its local edits were discarded: none of them is sealed, so none reaches the server
        // once this account may edit it again (`refs::Dropped`). Answered as unchanged, so a
        // replica closing with them lets go of them rather than waiting on a seal.
        if self.refs.discarded(block) {
            return match write.expected {
                Some(head) => Ok(Sealed::Unchanged(head)),
                None => Err(ClientError::Discarded(block)),
            };
        }
        let (manifest, extra_parents, metadata) = self.prepare_write(block, &mut write).await?;
        let Write {
            references,
            expected,
            bookmark,
            sealed_through,
            ..
        } = write;
        // The commit it goes on, to tell a write that changes nothing: unknown offline when only
        // its summary is held, and then the write is sealed all the same.
        let previous = match expected {
            Some(head) => match self.load_head(block, head).await {
                Ok(previous) => Some(previous),
                Err(error) if offline::unreachable(&error) => None,
                Err(error) => return Err(error),
            },
            None => None,
        };
        let time = self.commit_time(previous.as_ref());
        let encoded = metadata.encode();
        if extra_parents.is_empty()
            && bookmark.is_none()
            && let Some(previous) = &previous
            && previous.manifest == manifest
            && previous.references == references
            && previous.metadata == encoded
        {
            return Ok(Sealed::Unchanged(expected.expect("a previous commit")));
        }
        let mut parents: Vec<_> = expected.into_iter().collect();
        let mut chunks = self.objects_of(&manifest).await?;
        let mut listed = references.clone();
        listed.extend(self.resolve(block, self.commits.vault().references(&manifest)?));
        let listed = deduplicated(listed);
        for parent in &extra_parents {
            chunks.push(parent.hash());
            chunks.extend(
                self.objects_of(&self.commits.get(*parent)?.manifest)
                    .await?,
            );
        }
        parents.extend(extra_parents);
        let pinned = bookmark.is_some();
        let kind = match bookmark {
            _ if parents.len() > 1 => be_block_commit::CommitKind::Merge,
            Some(label) => be_block_commit::CommitKind::Bookmark(label),
            None => be_block_commit::CommitKind::Autosave,
        };
        // Into the outbox, judged again before it is sent (`send_pending`).
        let key = self.sealing_key_kept(block).await?;
        let commit = Commit::new(manifest, self.account, time)
            .with_parents(parents.clone())
            .with_kind(kind)
            .with_references(references.clone())
            .with_metadata(encoded)
            .with_sealed_through(sealed_through);
        let id = self.commits.put(key, &commit)?;
        let sealed = self.commits.vault().seal(key, &metadata.encode())?;
        self.commits.vault().store().sync()?;
        let queued = self.refs.push_publish(Pending {
            block,
            commit: id,
            expected,
            parents,
            chunks,
            time,
            pinned,
            references: listed.clone(),
            metadata: sealed.clone(),
        })?;
        // Discarded while it was sealed: as if the discard had come first.
        if queued.is_none() {
            return match expected {
                Some(head) => Ok(Sealed::Unchanged(head)),
                None => Err(ClientError::Discarded(block)),
            };
        }
        if !self.is_attached()
            && let Some(mut summary) = self.refs.summary(block)
        {
            summary.metadata = sealed;
            summary.references = listed;
            self.refs.revise(summary)?;
        }
        Ok(Sealed::Sealed(id))
    }

    pub async fn publish_sealed(
        &self,
        block: Uuid,
        commit: CommitId,
    ) -> Result<Saved, ClientError> {
        let Some((sequence, pending)) = self.pending_publish(block, commit) else {
            return Err(ClientError::MissingObject(commit.hash()));
        };
        // With no server, what waits ahead of it is settled first, in order, and it is taken
        // if it goes on the head the block has then, or else refused as the server would.
        if self.local {
            self.settle_locally()?;
            return Ok(match self.pending_publish(block, commit) {
                None => Saved::Published(commit),
                Some(_) => Saved::Rejected {
                    head: self.known_remote_head(block),
                },
            });
        }
        // A creation, move or definition of the block waits in the outbox, or a commit of it
        // sealed before this one that this one does not build on (a definition's commit on the
        // same head): the drain sends them in order, and the server refuses this one as stale
        // once they moved its head on, to be merged with them (`Drained::Diverged`). Offline,
        // sending fails as any other does.
        if self.is_attached()
            && (self.refs.has_structural(block)
                || self.refs.refused(block).is_some()
                || self.beside(block, sequence, commit)?)
        {
            return Ok(Saved::Kept(commit));
        }
        Ok(match self.send_pending(&pending).await? {
            PublishOutcome::Published(head) => {
                self.settle(sequence, &pending, head)?;
                Saved::Published(head)
            }
            PublishOutcome::Rejected(head) => Saved::Rejected { head },
        })
    }

    async fn send_pending(&self, pending: &Pending) -> Result<PublishOutcome, ClientError> {
        if self.local {
            return self.publish_locally(pending);
        }
        let Pending {
            block,
            commit: id,
            expected,
            ref parents,
            ref chunks,
            time,
            pinned,
            ref references,
            ref metadata,
        } = *pending;
        // Sealed offline under a key judged then by what was kept, or not at all: judged again
        // now, and never sent under an epoch the logs show outdated (signed access 3d). Refused
        // as the server would refuse it, so the edit is set aside, not lost.
        if self.sent_outdated(block, id).await? {
            return Err(ClientError::Outdated(block));
        }
        let previous = match expected {
            Some(head) => self.commits.try_get(head)?,
            None => None,
        };
        let manifest = self.commits.get(id)?.manifest;
        let merging = parents.len() > usize::from(expected.is_some());
        let (held, listed) = match &previous {
            Some(previous) => (
                self.held_by(previous).await?,
                self.references_of(block, previous).await?,
            ),
            None => (Vec::new(), Vec::new()),
        };
        let held = ObjectChange::between(&held, chunks);
        let listed = ReferenceChange::between(&listed, references);
        let mut hashes = vec![id.hash()];
        hashes.extend(held.added.iter().copied());
        let mut carried = match merging {
            true => None,
            false => self.carried(id, &manifest, previous.as_ref())?,
        };
        if carried.is_none() {
            self.push(&hashes).await?;
        }
        let response = loop {
            let publish = |objects| {
                crate::ask(client::blocks::publish(
                    &self.connection(),
                    block,
                    id,
                    expected,
                    parents.clone(),
                    held.clone(),
                    time,
                    pinned,
                    listed.clone(),
                    metadata.clone(),
                    objects,
                ))
            };
            match carried.take() {
                Some(objects) => match publish(objects).await {
                    Err(ClientError::Refused(ErrorCode::ObjectNotFound, _)) => {
                        self.push(&hashes).await?;
                    }
                    response => break response?,
                },
                None => break publish(Vec::new()).await?,
            }
        };
        if let PublishOutcome::Rejected(Some(head)) = response {
            self.refs.set_remote(block, head, self.behind())?;
        }
        Ok(response)
    }

    fn carried(
        &self,
        commit: CommitId,
        manifest: &Manifest,
        previous: Option<&Commit>,
    ) -> Result<Option<Vec<Vec<u8>>>, ClientError> {
        let vault = self.commits.vault();
        let store = vault.store();
        store.sync()?;
        let sent: HashSet<Hash> = previous
            .and_then(|previous| vault.objects(&previous.manifest).ok())
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mut hashes = vec![commit.hash()];
        hashes.extend(vault.objects_besides(manifest, &sent)?);
        let mut room = CARRIED_BYTES;
        let mut carried = Vec::with_capacity(hashes.len());
        for hash in hashes {
            // A part another device put (`put_part`) is on the server and maybe not here.
            let Some(bytes) = store.get(hash)? else {
                return Ok(None);
            };
            if bytes.len() > room {
                return Ok(None);
            }
            room -= bytes.len();
            carried.push(bytes);
        }
        Ok(Some(carried))
    }

    /// Uploads the objects among `objects` the server lacks and has `block` hold them all.
    pub async fn hold(
        &self,
        block: Uuid,
        objects: &[Hash],
        hold: be_block_protocol::Hold,
    ) -> Result<(), ClientError> {
        if objects.is_empty() {
            return Ok(());
        }
        if self.is_attached() {
            match self.send_hold(block, objects, hold).await {
                Err(error) if offline::unreachable(&error) => {}
                sent => return sent,
            }
        }
        self.hold_later(block, objects, hold)
    }

    async fn send_hold(
        &self,
        block: Uuid,
        objects: &[Hash],
        hold: be_block_protocol::Hold,
    ) -> Result<(), ClientError> {
        self.push(objects).await?;
        client::objects::hold_objects(&self.connection(), block, objects.to_vec(), hold).await??;
        Ok(())
    }

    /// Seals `bytes` as a part of `block` under its current key and uploads it, held for the
    /// block until a commit that uses it publishes (`host.put-part` in be-sdk-type-api), so an
    /// operation can name it by its ref and every replica save a commit that reuses it.
    pub async fn put_part(
        &self,
        block: Uuid,
        bytes: &[u8],
    ) -> Result<be_block_store::PartRef, ClientError> {
        let key = self.sealing_key(block).await?;
        let vault = self.commits.vault();
        let manifest = vault.write(key, Uuid::nil(), Part::from(bytes))?;
        let part = manifest
            .root
            .ok_or_else(|| ClientError::Encoding("a part needs at least one byte".to_owned()))?;
        let objects = vault.objects(&manifest)?;
        self.hold(block, &objects, be_block_protocol::Hold::UntilCommitted)
            .await?;
        Ok(part)
    }

    pub fn set_resolver(&self, resolver: Resolver) {
        *self.resolver.lock().unwrap() = Some(resolver);
    }

    pub fn references<C: BlockContent>(&self, block: Uuid, content: &C) -> Vec<Uuid> {
        self.resolve(block, content.references_in(self.organization()))
    }

    pub fn resolve(&self, block: Uuid, references: Vec<Uuid>) -> Vec<Uuid> {
        let resolver = self.resolver.lock().unwrap().clone();
        match resolver {
            Some(resolver) => resolver(block, references),
            None => references,
        }
    }

    pub async fn manifest(&self, block: Uuid) -> Result<Option<Manifest>, ClientError> {
        let Some(head) = self.remote_head(block).await? else {
            return Ok(None);
        };
        Ok(Some(self.load_head(block, head).await?.manifest))
    }

    pub async fn stream_header<C: Streamed>(
        &self,
        block: Uuid,
    ) -> Result<Option<C::Header>, ClientError> {
        let Some(manifest) = self.manifest(block).await? else {
            return Ok(None);
        };
        let prefix = self
            .read_span(&manifest, 0, be_block_content::HEADER_PREFIX_BYTES as u64)
            .await?;
        let start = payload_start(&prefix)?;
        let header = self.read_span(&manifest, 0, start as u64).await?;
        let (header, _) = be_block_content::decode_streamed::<C::Header>(&header[..start])?;
        Ok(Some(header))
    }

    pub async fn stream_range<C: Streamed>(
        &self,
        block: Uuid,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, ClientError> {
        let Some(manifest) = self.manifest(block).await? else {
            return Ok(Vec::new());
        };
        let prefix = self
            .read_span(&manifest, 0, be_block_content::HEADER_PREFIX_BYTES as u64)
            .await?;
        let start = payload_start(&prefix)? as u64;
        self.read_span(&manifest, start + offset, length).await
    }

    pub async fn read_span(
        &self,
        manifest: &Manifest,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>, ClientError> {
        self.fetch_range(manifest, offset, length).await?;
        self.retrying(|| Ok(self.commits.vault().read_range(manifest, offset, length)?))
            .await
    }

    /// The block's history the server keeps, or offline the part of it this device holds
    /// (`history_held`).
    pub async fn history(&self, block: Uuid) -> Result<Vec<HistoryEntry>, ClientError> {
        let listed = match self.is_attached() {
            true => client::blocks::list_history(&self.connection(), block)
                .await
                .map_err(ClientError::from)
                .and_then(|listed| listed.map_err(ClientError::from)),
            false => Err(ClientError::Disconnected("offline".into())),
        };
        match listed {
            Err(error) if offline::unreachable(&error) => self.history_held(block),
            listed => listed,
        }
    }

    /// One page of what changed after the sequence `since` in `root`'s tree, from `after`
    /// (`ChangeCursor { changed: since, block: Uuid::max() }` starts): see `changed_pages`.
    pub async fn changed_page(
        &self,
        root: Uuid,
        after: ChangeCursor,
        limit: u32,
    ) -> Result<ChangedPage, ClientError> {
        Ok(client::blocks::changed_since(&self.connection(), root, after, limit).await??)
    }

    /// Everything that changed after the sequence `since` in `root`'s tree, every page of it:
    /// the summaries of the blocks in the tree that changed, the blocks that left it, and the
    /// sequence to continue from.
    pub async fn changed_since(
        &self,
        root: Uuid,
        since: u64,
    ) -> Result<(Vec<BlockSummary>, Vec<Uuid>, u64), ClientError> {
        const PAGE: u32 = 200;
        let (mut summaries, mut removed) = (Vec::new(), Vec::new());
        let mut after = ChangeCursor {
            changed: since,
            block: Uuid::max(),
        };
        loop {
            let page = self.changed_page(root, after, PAGE).await?;
            self.learn(&page.summaries).await?;
            summaries.extend(page.summaries);
            removed.extend(page.removed);
            match page.next {
                Some(next) => after = next,
                None => return Ok((summaries, removed, page.sequence)),
            }
        }
    }

    pub async fn set_parent(&self, block: Uuid, parent: BlockParent) -> Result<(), ClientError> {
        if !self.is_attached() || self.refs.has_structural(block) {
            return self.move_locally(block, parent);
        }
        let from = match parent {
            BlockParent::Block(_) => self.summary(block).await.ok().and_then(|held| held.parent),
            BlockParent::Root | BlockParent::Detached => None,
        };
        match self.send_parent(block, parent).await {
            Err(error) if offline::unreachable(&error) => self.move_locally(block, parent),
            Err(error) => Err(error),
            Ok(()) => {
                let _ = self.follow_prefixes(block, from, parent).await;
                Ok(())
            }
        }
    }

    pub(crate) async fn send_parent(
        &self,
        block: Uuid,
        parent: BlockParent,
    ) -> Result<(), ClientError> {
        let keys = match parent {
            BlockParent::Detached => KeySet::default(),
            BlockParent::Root | BlockParent::Block(_) => self.rescope(block, parent, false).await?,
        };
        client::blocks::set_parent(&self.connection(), block, parent, keys.clone()).await??;
        self.recorded(&keys);
        Ok(())
    }

    fn recorded(&self, keys: &KeySet) {
        let _ = self.refs.add_keys(keys);
        self.keys.record(keys);
        for row in keys.keys.iter().filter(|row| row.kind == KeyKind::Data) {
            self.make_current(row.block, row.id);
        }
    }

    /// Shares `block` with `account`, whose keys this device verified as `identity`: seals it
    /// the block's key and grants `access` by a signed record naming those keys.
    pub async fn share(
        &self,
        block: Uuid,
        account: Uuid,
        access: Access,
        identity: &be_block_keys::PublicIdentity,
    ) -> Result<(), ClientError> {
        self.share_sealed(block, account, access, &identity.public, Some(*identity))
            .await
    }

    async fn share_sealed(
        &self,
        block: Uuid,
        account: Uuid,
        access: Access,
        public: &RecoveryPublic,
        identity: Option<be_block_keys::PublicIdentity>,
    ) -> Result<(), ClientError> {
        let grant = || self.grant_as(block, account, access, identity);
        if !access.can_view() {
            return grant().await;
        }
        // The key goes first: the grant tells the account's devices at once, and a device that
        // finds no key for the block then shows it locked until something else opens a key.
        // Not when this device thinks the grant will be refused: a refused grant leaves the
        // block as it was, not made a scope of its own, and one it was wrong about is keyed
        // after.
        match self.summary(block).await?.access.can_manage() {
            true => {
                self.seal_share(block, account, public).await?;
                grant().await
            }
            false => {
                grant().await?;
                self.seal_share(block, account, public).await
            }
        }
    }

    pub async fn open_inside(&self, block: Uuid, access: Access) -> Result<(), ClientError> {
        let grant = || self.grant(block, be_block_graph::ANYONE_INSIDE, access);
        if !access.can_view() {
            return grant().await;
        }
        // As `share`. Its author opens a kind of settings while editing it, so editing is the
        // least the server takes.
        let keyed_first = self.summary(block).await?.access.can_edit();
        if !keyed_first {
            grant().await?;
        }
        let keys = self.own_scope(block).await?;
        self.put_keys(keys).await?;
        if keyed_first {
            grant().await?;
        }
        Ok(())
    }

    pub async fn seal_share(
        &self,
        block: Uuid,
        account: Uuid,
        public: &be_block_keys::RecoveryPublic,
    ) -> Result<(), ClientError> {
        let keys = self.own_scope(block).await?;
        self.put_keys(keys).await?;
        let scope = self
            .keys
            .scope_key(block)
            .ok_or(ClientError::Locked(block))?;
        let sealed = self.keys.seal_to(scope, account, public)?;
        self.put_keys(KeySet {
            keys: Vec::new(),
            wrapped: vec![sealed],
        })
        .await
    }

    pub async fn offer(
        &self,
        block: Uuid,
        access: Access,
    ) -> Result<crate::GrantOffer, ClientError> {
        let keys = self.own_scope(block).await?;
        self.put_keys(keys).await?;
        let key = self
            .keys
            .scope_key(block)
            .ok_or(ClientError::Locked(block))?;
        let secret = self
            .keys
            .carried_plain(key)
            .map_err(|_| ClientError::Locked(block))?;
        Ok(crate::GrantOffer {
            organization: self.organization,
            block,
            access,
            key,
            secret,
        })
    }

    async fn own_scope(&self, block: Uuid) -> Result<KeySet, ClientError> {
        let parent = parent_of(&self.summary(block).await?)?;
        self.rescope(block, parent, true).await
    }

    /// Once the server holds a creation, wraps its scope keys under this device's escrow when
    /// they were made under an earlier one: an escrow key adopted meanwhile (`make_escrow`) was
    /// minted after the creation read which escrow to wrap them under. A creation the server
    /// took before that adoption is in the key work the rotation reads after it; if this fails,
    /// the key stays in that key work for the next `tend`.
    pub(crate) async fn escrow_created(&self, keys: &KeySet) {
        let Some(escrow) = self.keys.escrow() else {
            return;
        };
        let mut late = KeySet::default();
        for row in keys.keys.iter().filter(|row| row.kind == KeyKind::Scope) {
            let wrapped = keys
                .wrapped
                .iter()
                .any(|wrap| wrap.key == row.id && wrap.wrapped_by == WrappedBy::Key(escrow.id));
            if !wrapped && let Ok(Some(wrap)) = self.keys.escrow_wrap_under(row.id, &escrow) {
                late.wrapped.push(wrap);
            }
        }
        let _ = self.put_keys(late).await;
    }

    async fn put_keys(&self, keys: KeySet) -> Result<(), ClientError> {
        if keys.is_empty() {
            return Ok(());
        }
        client::keys::put_keys(&self.connection(), keys.clone()).await??;
        self.recorded(&keys);
        Ok(())
    }

    async fn rescope(
        &self,
        block: Uuid,
        parent: BlockParent,
        own: bool,
    ) -> Result<KeySet, ClientError> {
        let moved = self.summary(block).await?;
        let shared = own
            || (moved.scope == Some(block) && matches!(moved.parent, Some(BlockParent::Block(_))));
        let parent_scope = match parent {
            BlockParent::Block(parent) => Some(self.scope_key_under(parent).await?),
            BlockParent::Root | BlockParent::Detached => None,
        };
        let mut keys = KeySet::default();
        if moved.owner_only {
            return Ok(keys);
        }
        let under = match (shared, parent_scope) {
            (false, Some(under)) => under,
            (_, parent_scope) => {
                let own = match self.keys.scope_key(block) {
                    Some(own) => own,
                    None => {
                        let (own, minted) = self.keys.mint_scope(block, parent_scope)?;
                        keys.extend(minted);
                        own
                    }
                };
                if let Some(parent_scope) = parent_scope
                    && !self
                        .keys
                        .wrapped_by(own)
                        .contains(&WrappedBy::Key(parent_scope))
                    && keys.keys.is_empty()
                {
                    keys.wrapped.push(self.keys.wrap(own, parent_scope)?);
                }
                own
            }
        };
        let wrapped_under = |key: Uuid| self.keys.wrapped_by(key).contains(&WrappedBy::Key(under));
        if moved.key.is_some_and(wrapped_under) {
            return Ok(keys);
        }
        let mut level = vec![moved];
        while !level.is_empty() {
            let mut below = Vec::new();
            for node in level {
                if node.id != block && (!node.access.can_view() || node.owner_only) {
                    continue;
                }
                if node.id != block && node.scope == Some(node.id) {
                    if let Some(nested) = self.keys.scope_key(node.id)
                        && !wrapped_under(nested)
                        && !self.keys.owner_only_covers(nested)
                    {
                        keys.wrapped.push(self.keys.wrap(nested, under)?);
                    }
                    continue;
                }
                if !node.key.is_some_and(wrapped_under) {
                    for row in self.keys.keys_of(node.id, KeyKind::Data) {
                        if self.keys.trusted(row.id) && !wrapped_under(row.id) {
                            keys.wrapped.push(self.keys.wrap(row.id, under)?);
                        }
                    }
                    let (_, fresh) = self.keys.mint_data(node.id, under)?;
                    keys.extend(fresh);
                }
                below.push(self.children(BlockParent::Block(node.id)));
            }
            level = futures_util::future::try_join_all(below)
                .await?
                .into_iter()
                .flatten()
                .collect();
        }
        Ok(keys)
    }

    pub async fn children(&self, parent: BlockParent) -> Result<Vec<BlockSummary>, ClientError> {
        self.list(Listing::Children(parent)).await
    }

    pub async fn unlist(&self, listing: Listing) -> Result<(), ClientError> {
        match crate::ask(client::blocks::unlist(&self.connection(), listing)).await {
            Ok(_) => Ok(()),
            Err(error) if offline::unreachable(&error) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub async fn collect_detached(&self) -> Result<Vec<Uuid>, ClientError> {
        Ok(client::blocks::collect_detached(&self.connection())
            .await??
            .blocks)
    }

    pub async fn grant(
        &self,
        block: Uuid,
        account: Uuid,
        access: Access,
    ) -> Result<(), ClientError> {
        self.grant_as(block, account, access, None).await
    }

    async fn grant_as(
        &self,
        block: Uuid,
        account: Uuid,
        access: Access,
        identity: Option<be_block_keys::PublicIdentity>,
    ) -> Result<(), ClientError> {
        let signed = self.signed_grant(block, account, access, identity).await?;
        client::blocks::set_access(&self.connection(), block, account, access, signed).await??;
        Ok(())
    }

    /// The record granting `access` to `account`, whose keys are `identity`, as `grant` signs
    /// it: for an account this device made the keys of (an agent), sent with its making.
    pub async fn grant_record(
        &self,
        block: Uuid,
        account: Uuid,
        access: Access,
        identity: be_block_keys::PublicIdentity,
    ) -> Result<Vec<u8>, ClientError> {
        self.signed_grant(block, account, access, Some(identity))
            .await?
            .ok_or(ClientError::Unexpected)
    }

    pub async fn list_access(&self, block: Uuid) -> Result<AccessList, ClientError> {
        Ok(client::blocks::list_access(&self.connection(), block).await??)
    }

    pub async fn relay(
        &self,
        block: Uuid,
        to: Option<ClientId>,
        kind: RelayKind,
        payload: &[u8],
    ) -> Result<Answer<()>, ClientError> {
        let key = self.sealing_key(block).await?;
        let sealed = self.commits.vault().seal(key, payload)?;
        Ok(self.connection().relay(block, to, kind, sealed))
    }

    /// A relay of `block`'s session (`Peer::open_for`).
    pub fn unseal(&self, block: Uuid, payload: &[u8]) -> Result<Vec<u8>, ClientError> {
        self.open_for(block, payload).map_err(locked)
    }

    /// `sealed` bytes the server hands over as `block`'s (its metadata, a relay of its session,
    /// its journal, a notification about it): refused unless one of `block`'s keys sealed them,
    /// since the server could hand over another block's, which a device would show as this
    /// block's.
    pub fn open_for(&self, block: Uuid, sealed: &[u8]) -> Result<Vec<u8>, ClientError> {
        let key = be_block_store::sealed_key(sealed)?;
        if self.keys.has(key) && self.keys.block_of(key) != Some(block) {
            return Err(ClientError::Forged(block));
        }
        Ok(self.commits.vault().open(sealed)?)
    }

    pub fn first_search_for(&self, key: Uuid) -> bool {
        self.tried.lock().unwrap().insert(key)
    }
}

pub(crate) fn scope_of(summary: &BlockSummary) -> Result<Uuid, ClientError> {
    summary.scope.ok_or_else(|| unseen(summary.id))
}

pub(crate) fn parent_of(summary: &BlockSummary) -> Result<BlockParent, ClientError> {
    summary.parent.ok_or_else(|| unseen(summary.id))
}

fn unseen(block: Uuid) -> ClientError {
    ClientError::Refused(
        ErrorCode::PermissionDenied,
        format!("block {block} is not readable by this account"),
    )
}

fn locked(error: impl Into<ClientError>) -> ClientError {
    match error.into() {
        ClientError::Store(StoreError::MissingKey(key)) => ClientError::Locked(key),
        other => other,
    }
}

/// Refuses a block's references past what the server keeps (`MAX_REFERENCES`) before anything
/// is sealed or put in the outbox, so a change that could never be sent is never kept.
fn bounded(references: Vec<Uuid>) -> Result<Vec<Uuid>, ClientError> {
    match references.len() <= be_block_protocol::MAX_REFERENCES {
        true => Ok(references),
        false => Err(ClientError::Refused(
            ErrorCode::InvalidRequest,
            format!(
                "a block may reference at most {} blocks, not {}",
                be_block_protocol::MAX_REFERENCES,
                references.len()
            ),
        )),
    }
}

fn deduplicated(references: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::new();
    references
        .into_iter()
        .filter(|reference| seen.insert(*reference))
        .collect()
}

/// The block a listing is about, which the server must know to answer it.
/// Whether a summary read after its listing's revision still belongs to it, as far as the
/// summary tells: a block the reader may only know exists carries no parent or references, so
/// it is kept.
fn still_in(listing: Listing, summary: &BlockSummary) -> bool {
    let viewed = summary.access.can_view();
    match listing {
        Listing::Children(parent) => !viewed || summary.parent == Some(parent),
        Listing::Deleted => !viewed || (summary.parent.is_some() && !summary.held),
        Listing::Backrefs(target) => !viewed || summary.references.contains(&target),
        Listing::Shared | Listing::OpenSettings(_) => viewed,
        Listing::References(_) | Listing::Block(_) => true,
    }
}

fn about(listing: Listing) -> Option<Uuid> {
    match listing {
        Listing::Block(block)
        | Listing::OpenSettings(block)
        | Listing::References(block)
        | Listing::Backrefs(block)
        | Listing::Children(BlockParent::Block(block)) => Some(block),
        Listing::Children(_) | Listing::Shared | Listing::Deleted => None,
    }
}

/// The log of `log` as the server served it, once it checks from the pinned root to its head
/// (`be_block_keys::check_log`).
fn checked_log(
    kind: be_block_keys::LogKind,
    log: Uuid,
    pinned: be_block_keys::EntryHash,
    served: &[Vec<u8>],
) -> Result<Vec<be_block_keys::Entry>, ClientError> {
    let entries = served
        .iter()
        .map(|entry| be_block_keys::Entry::decode(entry))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ClientError::Forged(log))?;
    be_block_keys::check_log(kind, log.into_bytes(), pinned, &entries, &|_, _| None)
        .map_err(|_| ClientError::Forged(log))?;
    Ok(entries)
}

/// `access` as an access log's level.
fn grant_level(access: Access) -> be_block_keys::Level {
    match access {
        Access::None => be_block_keys::Level::None,
        Access::KnowExists => be_block_keys::Level::KnowExists,
        Access::View => be_block_keys::Level::View,
        Access::Edit => be_block_keys::Level::Edit,
        Access::Manage => be_block_keys::Level::Manage,
    }
}

/// The logs around a block's (`Peer::logs_around`): each with the root it is checked from, and
/// the blocks above it, nearest first.
#[derive(Default)]
struct Around {
    block: Uuid,
    held: HashMap<
        (be_block_keys::LogKind, Uuid),
        (be_block_keys::EntryHash, Vec<be_block_keys::Entry>),
    >,
    above: Vec<Uuid>,
}

impl Around {
    fn logs<'a>(
        &'a self,
        organization: Uuid,
        log: &'a dyn Fn(
            be_block_keys::LogKind,
            [u8; 16],
        ) -> Option<(be_block_keys::EntryHash, Vec<be_block_keys::Entry>)>,
        above: &'a dyn Fn([u8; 16], [u8; 16]) -> bool,
    ) -> be_block_keys::Logs<'a> {
        be_block_keys::Logs {
            organization: organization.into_bytes(),
            log,
            above,
        }
    }

    fn log(
        &self,
        kind: be_block_keys::LogKind,
        log: [u8; 16],
    ) -> Option<(be_block_keys::EntryHash, Vec<be_block_keys::Entry>)> {
        self.held.get(&(kind, Uuid::from_bytes(log))).cloned()
    }

    /// Whether `above` is over `below`: both in this chain, `above` the further up.
    fn is_above(&self, above: [u8; 16], below: [u8; 16]) -> bool {
        let at = |id: [u8; 16]| {
            let id = Uuid::from_bytes(id);
            match id == self.block {
                true => Some(0),
                false => self.above.iter().position(|up| *up == id).map(|at| at + 1),
            }
        };
        matches!((at(above), at(below)), (Some(above), Some(below)) if above > below)
    }

    /// `block`'s log checked from `pinned`, with the authority its grants name read from here.
    fn check(
        &self,
        organization: Uuid,
        block: Uuid,
        pinned: be_block_keys::EntryHash,
        entries: &[be_block_keys::Entry],
    ) -> Option<be_block_keys::Checked> {
        let log = |kind, log| self.log(kind, log);
        let above = |above, below| self.is_above(above, below);
        let logs = self.logs(organization, &log, &above);
        let outside = |signer: [u8; 16], authority: &be_block_keys::Authority| {
            be_block_keys::answer(&logs, block.into_bytes(), signer, authority)
        };
        be_block_keys::check_log(
            be_block_keys::LogKind::Block,
            block.into_bytes(),
            pinned,
            entries,
            &outside,
        )
        .ok()
    }

    /// Where `account`'s Manage comes from outside the block's own log, as of each log's head:
    /// administering the organization, else the nearest block above it manages.
    fn authority(&self, organization: Uuid, account: [u8; 16]) -> Option<be_block_keys::Authority> {
        use be_block_keys::{Authority, LogKind};
        if let Some((pinned, entries)) = self.held.get(&(LogKind::Organization, organization)) {
            let checked = be_block_keys::check_log(
                LogKind::Organization,
                organization.into_bytes(),
                *pinned,
                entries,
                &|_, _| None,
            )
            .ok()?;
            if checked.manager(LogKind::Organization, account).is_some() {
                return Some(Authority::Organization(checked.head));
            }
        }
        self.above.iter().find_map(|&up| {
            let (pinned, entries) = self.held.get(&(LogKind::Block, up))?;
            let checked = self.check(organization, up, *pinned, entries)?;
            checked
                .manager(LogKind::Block, account)
                .map(|_| Authority::Inherited {
                    block: up.into_bytes(),
                    head: checked.head,
                })
        })
    }
}
