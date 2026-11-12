use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use be_block::{
    BlockContent, BlockMetadata, Entries, OrganizationPolicies, Root, Streamed, derived_metadata,
    payload_start,
};
use be_block_commit::{Commit, CommitId, CommitStore, now_milliseconds};
use be_block_graph::{Access, BlockParent};
use be_block_keys::{RecoveryPublic, RecoverySecret};
use be_block_protocol::{
    AccessList, BlockSummary, ChangeCursor, ChangedPage, ClientId, ErrorCode, EscrowKey,
    HistoryEntry, KeyKind, KeySet, Listing, ObjectChange, OrganizationKind, OrganizationRole,
    PublishOutcome, ReferenceChange, ServerAccount, SessionState, WrappedBy, client,
};
use be_block_store::{
    ChunkerConfig, Hash, Keyring, Manifest, ObjectStore, Part, StoreError, Stored, Vault,
};
use uuid::Uuid;

use crate::{
    ClientError,
    connection::{Answer, Connection},
    keys::KeyBook,
    refs::{Creation, Pending, Refs, RefsDb, Scope, Session},
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
const CONTENTS_BLOCKS: usize = 256;

mod authorship;
mod import;
mod journal;
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
    Unchanged(CommitId),
    Published(CommitId),
    Rejected { head: Option<CommitId> },
}

impl Saved {
    pub fn published(self) -> Option<CommitId> {
        match self {
            Self::Unchanged(head) | Self::Published(head) => Some(head),
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
    },
    Defined {
        block: Uuid,
    },
    Refused {
        block: Uuid,
        code: ErrorCode,
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

pub struct Peer<S: ObjectStore> {
    connection: Mutex<Arc<Connection>>,
    commits: CommitStore<S>,
    token: Mutex<String>,
    account: Uuid,
    organization: Uuid,
    session: Mutex<(OrganizationRole, OrganizationKind)>,
    refs: Refs,
    resolver: Mutex<Option<Resolver>>,
    keys: KeyBook,
    current: Mutex<HashMap<Uuid, Uuid>>,
    tried: Mutex<HashSet<Uuid>>,
    /// How many outbox drains are under way: announcements wait for their end
    /// (`send_announcements`).
    draining: std::sync::atomic::AtomicUsize,
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
                client::organizations::create_organization(
                    &connection,
                    config.organization_name.clone(),
                )
                .await??
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
        let _ = refs.add_signer(account, config.identity.signing().0, true);
        let keys = KeyBook::new(account, organization, config.identity, ring.clone());
        keys.set_contacts(
            refs.contacts()
                .into_iter()
                .map(|(account, public)| (account, RecoveryPublic(public))),
        );
        if let Some(session) = &session {
            keys.set_escrows(session.escrows.clone());
            keys.set_pinned(session.pinned.clone());
            if let Some(server) = session.server {
                keys.set_server(server.account, RecoveryPublic(server.public));
            }
        }
        keys.absorb(refs.keys());
        let peer = Self {
            connection: Mutex::new(connection),
            commits: CommitStore::new(Vault::new(store, ring).with_chunker(config.chunker)),
            token: Mutex::new(String::new()),
            account,
            organization,
            session: Mutex::new(session.as_ref().map_or(
                (OrganizationRole::Editor, OrganizationKind::default()),
                |session| (session.role, session.kind),
            )),
            refs,
            resolver: Mutex::new(None),
            keys,
            current: Mutex::new(HashMap::new()),
            tried: Mutex::new(HashSet::new()),
            draining: std::sync::atomic::AtomicUsize::new(0),
        };
        for summary in peer.refs.summaries() {
            if let Some(key) = summary.key {
                peer.make_current(summary.id, key);
            }
        }
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
            ..
        } = client::organizations::open_organization(&connection, organization).await??;
        *self.session.lock().unwrap() = (role, kind);
        self.keys
            .set_server(server.account, RecoveryPublic(server.public));
        let verified =
            crate::contacts::contacts(&connection, self.account, self.keys.identity()).await?;
        let public = client::keys::get_recovery_key(&connection).await??.public;
        if public.is_none() {
            client::keys::set_recovery_key(
                &connection,
                self.keys.identity().public().0,
                self.keys.identity().signing().0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .await??;
        }
        self.keys.set_pinned(crate::contacts::open_pin(
            self.keys.identity(),
            organization,
            &pinned,
        ));
        let made = escrows.is_empty();
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
        let (account, public) = self.keys.server().ok_or(ClientError::Unexpected)?;
        self.share(block, account, Access::View, &public).await
    }

    async fn ensure_organization_settings(&self) -> Result<(), ClientError> {
        let settings = self.organization_settings();
        let made = match self.summary(settings).await {
            Ok(summary) => summary,
            Err(ClientError::Refused(ErrorCode::BlockNotFound, _)) => {
                self.create_with_id::<be_block::SettingsContent>(settings, BlockParent::Root)
                    .await?;
                self.share_with_server(settings).await?;
                self.summary(settings).await?
            }
            Err(error) => return Err(error),
        };
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
            pinned: self.keys.pinned(),
            server: self.server(),
        })?)
    }

    pub(crate) fn adopt_escrows(&self, chain: Vec<EscrowKey>) {
        self.keys.set_escrows(chain);
        let _ = self.persist_session();
    }

    pub(crate) fn adopt_pinned(&self, pinned: Option<EscrowKey>) {
        self.keys.set_pinned(pinned);
        let _ = self.persist_session();
    }

    pub(crate) fn adopt_contacts(&self, contacts: &[crate::Contact]) {
        for contact in contacts {
            if let Some(signing) = contact.signing {
                let _ = self.refs.add_signer(contact.account, signing.0, true);
            }
        }
        let verified: Vec<(Uuid, RecoveryPublic)> = contacts
            .iter()
            .filter_map(|contact| Some((contact.account, contact.public?)))
            .collect();
        let _ = self.refs.set_contacts(
            verified
                .iter()
                .map(|(account, public)| (*account, public.0))
                .collect(),
        );
        self.keys.set_contacts(verified);
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
        let keys = client::keys::get_keys(&self.connection(), blocks).await??;
        self.refs.add_keys(&keys)?;
        let unknown = keys.wrapped.iter().any(|wrap| {
            wrap.wrapped_by == WrappedBy::Account(self.account)
                && self.keys.contact(wrap.sender).is_none()
        });
        if unknown {
            self.refresh_contacts().await?;
        }
        let opened = self.keys.absorb(keys);
        if opened > 0 {
            self.tried.lock().unwrap().clear();
        }
        Ok(opened)
    }

    pub fn note(&self, summary: &BlockSummary) {
        if let Some(key) = summary.key {
            self.make_current(summary.id, key);
        }
        let _ = self.refs.observe(std::slice::from_ref(summary));
        let _ = self
            .refs
            .set_listing(Listing::Block(summary.id), std::slice::from_ref(summary));
        if !summary.access.can_edit() && self.refs.has_pending(summary.id) {
            let _ = self.move_to_unsent(summary.id);
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

    pub async fn learn(&self, summaries: &[BlockSummary]) -> Result<(), ClientError> {
        let mut wanted = Vec::new();
        {
            let mut tried = self.tried.lock().unwrap();
            for summary in summaries {
                self.note(summary);
                if let Some(key) = summary.key
                    && summary.access.can_view()
                    && !self.keys.has(key)
                    && tried.insert(key)
                {
                    wanted.push(summary.id);
                }
            }
        }
        if !wanted.is_empty() {
            self.refresh_keys(Some(wanted)).await?;
        }
        Ok(())
    }

    async fn retrying<T>(
        &self,
        attempt: impl Fn() -> Result<T, ClientError>,
    ) -> Result<T, ClientError> {
        match attempt() {
            Err(ClientError::Store(StoreError::MissingKey(key))) => {
                if self.tried.lock().unwrap().insert(key) {
                    self.refresh_keys(None).await?;
                    return attempt().map_err(locked);
                }
                Err(ClientError::Locked(key))
            }
            outcome => outcome,
        }
    }

    pub async fn sealing_key(&self, block: Uuid) -> Result<Uuid, ClientError> {
        let known = self.current.lock().unwrap().get(&block).copied();
        let key = match known {
            Some(key) => key,
            None => {
                let summary = self.summary(block).await?;
                summary.key.ok_or(ClientError::Locked(block))?
            }
        };
        if !self.keys.trusted(key) {
            self.refresh_keys(Some(vec![block])).await?;
        }
        if !self.keys.trusted(key) {
            return Err(ClientError::Locked(key));
        }
        let epoch = self.keys.row(key).map_or(0, |row| row.epoch);
        let newest = self
            .keys
            .keys_of(block, KeyKind::Data)
            .into_iter()
            .rev()
            .find(|row| row.epoch > epoch && self.keys.trusted(row.id));
        Ok(newest.map_or(key, |row| row.id))
    }

    async fn scope_key(&self, scope: Uuid) -> Result<Uuid, ClientError> {
        if let Some(key) = self.keys.scope_key(scope) {
            return Ok(key);
        }
        self.refresh_keys(Some(vec![scope])).await?;
        self.keys.scope_key(scope).ok_or(ClientError::Locked(scope))
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

    /// The stored form's first part, without the rest (`be_block::Loads::FirstPart`).
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
                self.objects_of(&self.load_commit(*parent).await?.manifest)
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

    /// Whether `descendant` has `ancestor` in its history (a commit is its own ancestor).
    pub async fn descends(
        &self,
        descendant: CommitId,
        ancestor: CommitId,
    ) -> Result<bool, ClientError> {
        self.fetch_history(descendant).await?;
        Ok(self.commits.is_ancestor(ancestor, descendant)?)
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
        let (key, keys) = self.new_keys(block, parent, owner_only).await?;
        let references = deduplicated(self.resolve(block, metadata.references()));
        let metadata = self.commits.vault().seal(key, &metadata.encode())?;
        let response = crate::ask(client::blocks::create_block(
            &self.connection(),
            block,
            content_type,
            parent,
            metadata.clone(),
            references.clone(),
            key,
            keys.clone(),
            owner_only,
        ))
        .await;
        let response = match response {
            Err(error) if error.is_unreachable() => {
                return self.create_locally(Creation {
                    block,
                    content_type,
                    parent,
                    metadata,
                    references,
                    key,
                    keys,
                    owner_only,
                });
            }
            response => response?,
        };
        self.recorded(&keys);
        self.note(&response);
        Ok(response)
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
                Some(head) => self.metadata_of(head).await?,
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
                let references = deduplicated(self.resolve(block, next.references()));
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
            let commit = self.load_commit(head).await?;
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

    pub async fn metadata_of(&self, commit: CommitId) -> Result<BlockMetadata, ClientError> {
        Ok(BlockMetadata::decode(
            &self.load_commit(commit).await?.metadata,
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
            Some(head) => self.metadata_of(head).await,
            None => self.metadata_of_summary(&self.summary(block).await?).await,
        }
    }

    pub fn metadata(&self, summary: &BlockSummary) -> Result<BlockMetadata, ClientError> {
        self.open_metadata(summary).map_err(locked)
    }

    fn open_metadata(&self, summary: &BlockSummary) -> Result<BlockMetadata, ClientError> {
        if summary.metadata.is_empty() {
            return Ok(BlockMetadata::default());
        }
        let plain = self.commits.vault().open(&summary.metadata)?;
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
        let response = crate::ask(client::blocks::read_block(&self.connection(), block)).await;
        match response {
            Ok(block) => {
                self.note(&block);
                Ok(block)
            }
            Err(error) if error.is_unreachable() => match self.refs.summary(block) {
                Some(summary) => Ok(self.refs.current(vec![summary]).remove(0)),
                None => Err(error),
            },
            Err(ClientError::Refused(code, message)) => {
                if offline::lost_access(code) && !self.refs.creating(block) {
                    let _ = self.refs.lose_access(block);
                    let _ = self.move_to_unsent(block);
                }
                Err(ClientError::Refused(code, message))
            }
            Err(error) => Err(error),
        }
    }

    async fn listed(&self, listing: Listing) -> Result<Vec<BlockSummary>, ClientError> {
        match crate::ask(client::blocks::list_blocks(&self.connection(), listing)).await {
            Ok(blocks) => {
                let _ = self.refs.set_listing(listing, &blocks);
                let _ = self.refs.observe(&blocks);
                Ok(blocks)
            }
            Err(error) if error.is_unreachable() => match self.refs.listing(&listing) {
                Some(blocks) => Ok(self.refs.current(blocks)),
                None => Err(error),
            },
            Err(error) => Err(error),
        }
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

    pub async fn load_commit(&self, commit: CommitId) -> Result<Commit, ClientError> {
        self.fetch(&[commit.hash()]).await?;
        self.retrying(|| Ok(self.commits.get(commit)?)).await
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
        let mut asked = Vec::new();
        for group in blocks.chunks(CONTENTS_BLOCKS) {
            let ids: Vec<Uuid> = group.iter().map(|(id, _)| *id).collect();
            let held = group
                .iter()
                .filter_map(|(_, head)| *head)
                .filter(|head| self.holds(*head))
                .collect();
            let answer = client::blocks::read_contents(&self.connection(), ids.clone(), held);
            asked.push((ids, answer));
        }
        let mut summaries = Vec::with_capacity(blocks.len());
        for (ids, answer) in asked {
            let be_block_protocol::Contents {
                blocks,
                objects,
                journals,
            } = answer.await??;
            if !blocks.iter().map(|summary| summary.id).eq(ids) || journals.len() != blocks.len() {
                return Err(ClientError::Unexpected);
            }
            let store = self.commits.vault().store();
            for object in &objects {
                store.put(object)?;
            }
            for summary in &blocks {
                self.note(summary);
            }
            summaries.extend(blocks.into_iter().zip(journals));
        }
        futures_util::future::try_join_all(summaries.into_iter().map(
            |(summary, journal)| async move {
                let stored = match summary.head {
                    Some(head) => {
                        let commit = self.load_commit(head).await?;
                        Some(self.load_manifest(&commit.manifest).await?)
                    }
                    None => None,
                };
                let journal = journal.and_then(|journal| self.open_journal(journal).ok());
                Ok(BlockRead {
                    summary,
                    stored,
                    journal,
                })
            },
        ))
        .await
    }

    fn holds(&self, head: CommitId) -> bool {
        let vault = self.commits.vault();
        matches!(
            self.commits.try_get(head),
            Ok(Some(commit)) if vault
                .missing_objects(&commit.manifest, 0, u64::MAX)
                .is_ok_and(|missing| missing.is_empty())
        )
    }

    pub async fn open<C: BlockContent>(&self, block: Uuid) -> Result<Option<C>, ClientError> {
        if let Some(head) = self.local_head(block)
            && !self.pending_commits(block).is_empty()
        {
            return Ok(Some(C::from_stored(&self.read_at(head).await?)?));
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

    pub async fn open_commit<C: BlockContent>(&self, commit: CommitId) -> Result<C, ClientError> {
        let commit = self.load_commit(commit).await?;
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

    /// `seal` then `publish_sealed`. On `Rejected` it drops the commit from the outbox, since
    /// the caller retries or merges.
    pub async fn write(&self, block: Uuid, write: Write<'_>) -> Result<Saved, ClientError> {
        let before = self.local_head(block);
        let commit = match self.seal(block, write).await? {
            Sealed::Unchanged(head) => return Ok(Saved::Unchanged(head)),
            Sealed::Sealed(commit) => commit,
        };
        let saved = self.publish_sealed(block, commit).await?;
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
                self.write_content(block, content_type, parts).await?
            }
            Content::Written(manifest) => {
                let parts = manifest
                    .root
                    .map_or_else(|| Part::from(Vec::new()), Part::Reused);
                self.write_content(block, manifest.content_type, parts)
                    .await?
            }
        };
        let mut parents = Vec::new();
        if let Some((base, ours)) = write.merging {
            let mut sides = Vec::new();
            for commit in [base, Some(ours), write.expected] {
                sides.push(match commit {
                    Some(commit) => self.metadata_of(commit).await?,
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
        write.references = deduplicated(references);
        Ok((manifest, parents, metadata))
    }

    pub async fn stash(&self, block: Uuid, mut write: Write<'_>) -> Result<CommitId, ClientError> {
        let (manifest, _, metadata) = self.prepare_write(block, &mut write).await?;
        let key = self.sealing_key(block).await?;
        let commit = Commit::new(manifest, self.account, now_milliseconds())
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
        let (manifest, extra_parents, metadata) = self.prepare_write(block, &mut write).await?;
        let Write {
            references,
            expected,
            bookmark,
            sealed_through,
            ..
        } = write;
        let previous = match expected {
            Some(head) => Some(self.load_commit(head).await?),
            None => None,
        };
        let time = now_milliseconds();
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
        let key = self.sealing_key(block).await?;
        let commit = Commit::new(manifest, self.account, time)
            .with_parents(parents.clone())
            .with_kind(kind)
            .with_references(references.clone())
            .with_metadata(encoded)
            .with_sealed_through(sealed_through);
        let id = self.commits.put(key, &commit)?;
        let sealed = self.commits.vault().seal(key, &metadata.encode())?;
        self.commits.vault().store().sync()?;
        self.refs.push_publish(Pending {
            block,
            commit: id,
            expected,
            parents,
            chunks,
            time,
            pinned,
            references: listed,
            metadata: sealed.clone(),
        })?;
        if !self.is_attached()
            && let Some(mut summary) = self.refs.summary(block)
        {
            summary.metadata = sealed;
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
        let saved = self.send_pending(&pending).await?;
        if let Saved::Published(head) = saved {
            self.settle(sequence, &pending, head, true)?;
        }
        Ok(saved)
    }

    async fn send_pending(&self, pending: &Pending) -> Result<Saved, ClientError> {
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
        match response {
            PublishOutcome::Published(head) => Ok(Saved::Published(head)),
            PublishOutcome::Rejected(head) => {
                if let Some(head) = head {
                    self.refs.set_remote(block, head)?;
                }
                Ok(Saved::Rejected { head })
            }
        }
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

    pub fn watch(&self, block: Uuid) {
        self.connection().watch(block);
    }

    pub fn unwatch(
        &self,
        block: Uuid,
    ) -> Result<impl Future<Output = Result<(), ClientError>> + Send + use<S>, ClientError> {
        self.connection().unwatch(block);
        Ok(async move { Ok(()) })
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
        Ok(Some(self.load_commit(head).await?.manifest))
    }

    pub async fn stream_header<C: Streamed>(
        &self,
        block: Uuid,
    ) -> Result<Option<C::Header>, ClientError> {
        let Some(manifest) = self.manifest(block).await? else {
            return Ok(None);
        };
        let prefix = self
            .read_span(&manifest, 0, be_block::HEADER_PREFIX_BYTES as u64)
            .await?;
        let start = payload_start(&prefix)?;
        let header = self.read_span(&manifest, 0, start as u64).await?;
        let (header, _) = be_block::decode_streamed::<C::Header>(&header[..start])?;
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
            .read_span(&manifest, 0, be_block::HEADER_PREFIX_BYTES as u64)
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

    pub async fn history(&self, block: Uuid) -> Result<Vec<HistoryEntry>, ClientError> {
        Ok(client::blocks::list_history(&self.connection(), block).await??)
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

    pub async fn share(
        &self,
        block: Uuid,
        account: Uuid,
        access: Access,
        public: &be_block_keys::RecoveryPublic,
    ) -> Result<(), ClientError> {
        self.grant(block, account, access).await?;
        if !access.can_view() {
            return Ok(());
        }
        self.seal_share(block, account, public).await
    }

    pub async fn open_inside(&self, block: Uuid, access: Access) -> Result<(), ClientError> {
        self.grant(block, be_block_graph::ANYONE_INSIDE, access)
            .await?;
        if !access.can_view() {
            return Ok(());
        }
        let keys = self.own_scope(block).await?;
        self.put_keys(keys).await
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
            .trusted_plain(key)
            .map_err(|_| ClientError::Locked(block))?;
        Ok(crate::GrantOffer {
            organization: self.organization,
            block,
            access,
            key,
            secret: *secret.as_bytes(),
        })
    }

    async fn own_scope(&self, block: Uuid) -> Result<KeySet, ClientError> {
        let parent = parent_of(&self.summary(block).await?)?;
        self.rescope(block, parent, true).await
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
                        && !self.keys.owner_only(nested)
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

    pub async fn shared(&self) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::Shared).await
    }

    pub async fn children(&self, parent: BlockParent) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::Children(parent)).await
    }

    pub async fn deleted(&self) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::Deleted).await
    }

    pub async fn unlist(&self, listing: Listing) -> Result<(), ClientError> {
        match crate::ask(client::blocks::unlist(&self.connection(), listing)).await {
            Ok(_) => Ok(()),
            Err(error) if offline::unreachable(&error) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub async fn backrefs(&self, block: Uuid) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::Backrefs(block)).await
    }

    pub async fn open_settings(&self, block: Uuid) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::OpenSettings(block)).await
    }

    pub async fn referenced(&self, block: Uuid) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::References(block)).await
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
        client::blocks::set_access(&self.connection(), block, account, access).await??;
        Ok(())
    }

    pub async fn list_access(&self, block: Uuid) -> Result<AccessList, ClientError> {
        Ok(client::blocks::list_access(&self.connection(), block).await??)
    }

    /// Joins a block's session: this connection's id, the session's state, and the block's
    /// journal as of joining, sealed (`open_journal`).
    pub fn join_session(
        &self,
        block: Uuid,
    ) -> Result<
        impl Future<
            Output = Result<
                (ClientId, SessionState, Option<be_block_protocol::Journal>),
                ClientError,
            >,
        > + Send
        + use<S>,
        ClientError,
    > {
        Ok(self.connection().join_session(block))
    }

    pub fn leave_session(
        &self,
        block: Uuid,
    ) -> Result<impl Future<Output = Result<(), ClientError>> + Send + use<S>, ClientError> {
        Ok(self.connection().leave_session(block))
    }

    pub fn claim_ownership(
        &self,
        block: Uuid,
        generation: u64,
    ) -> Result<impl Future<Output = Result<SessionState, ClientError>> + Send + use<S>, ClientError>
    {
        Ok(self.connection().claim_ownership(block, generation))
    }

    pub fn heartbeat(
        &self,
        block: Uuid,
        generation: u64,
        clean_at: Option<CommitId>,
    ) -> Result<impl Future<Output = Result<SessionState, ClientError>> + Send + use<S>, ClientError>
    {
        Ok(self.connection().heartbeat(block, generation, clean_at))
    }

    pub async fn relay(
        &self,
        block: Uuid,
        to: Option<ClientId>,
        payload: &[u8],
    ) -> Result<Answer<()>, ClientError> {
        let key = self.sealing_key(block).await?;
        let sealed = self.commits.vault().seal(key, payload)?;
        Ok(self.connection().relay(block, to, sealed))
    }

    pub fn unseal(&self, payload: &[u8]) -> Result<Vec<u8>, ClientError> {
        self.commits.vault().open(payload).map_err(locked)
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

fn deduplicated(references: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::new();
    references
        .into_iter()
        .filter(|reference| seen.insert(*reference))
        .collect()
}
