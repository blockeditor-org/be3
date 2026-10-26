use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use be_block::{
    BlockContent, BlockMetadata, Entries, OrganizationPolicies, Root, Streamed, derived_metadata,
    payload_start,
};
use be_commit::{
    Commit, CommitId, CommitStore, CommitSummary, RetentionPolicy, now_milliseconds, plan_retention,
};
use be_graph::{Access, BlockParent};
use be_keys::{RecoveryPublic, RecoverySecret};
use be_protocol::{
    AccessList, BlockSummary, ByteBuf, ClientId, ClientMessage, ErrorCode, EscrowKey, HistoryEntry,
    KeyKind, KeySet, Listing, OrganizationKind, OrganizationRole, ServerAccount, ServerMessage,
    SessionState, WrappedBy,
};
use be_store::{ChunkerConfig, Hash, Keyring, Manifest, ObjectStore, StoreError, Vault};
use uuid::Uuid;

use crate::{
    ClientError,
    connection::{Answer, Connection},
    keys::KeyBook,
    refs::{Creation, Pending, Refs, RefsDb, Scope, Session},
};

const CARRIED_BYTES: usize = be_protocol::MAX_FRAME_BYTES / 2;
const CONTENTS_BLOCKS: usize = 256;
const FETCHING: usize = 64;

mod import;
mod offline;
mod prefixes;
mod rotation;

pub use import::Imported;
pub use prefixes::Registry;

#[derive(Clone)]
pub enum Credentials {
    Token(String),
    Header,
}

impl Credentials {
    fn message(self, request: u64) -> ClientMessage {
        match self {
            Self::Header => ClientMessage::AuthenticateHeader { request },
            Self::Token(token) => ClientMessage::Authenticate { request, token },
        }
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
}

pub type Resolver = Arc<dyn Fn(Uuid, Vec<Uuid>) -> Vec<Uuid> + Send + Sync>;

impl<S: ObjectStore> Peer<S> {
    pub async fn connect(config: PeerConfig, store: S) -> Result<Self, ClientError> {
        let connection = Connection::connect(&config.url, &crate::Clock::real()).await?;
        let response = connection
            .request(|request| config.credentials.clone().message(request))
            .await?;
        let ServerMessage::Authenticated { account, token, .. } = response else {
            return Err(ClientError::Unexpected);
        };
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
                let response = connection
                    .request(|request| ClientMessage::CreateOrganization {
                        request,
                        name: config.organization_name.clone(),
                    })
                    .await?;
                let ServerMessage::Organizations { organizations, .. } = response else {
                    return Err(ClientError::Unexpected);
                };
                organizations.first().ok_or(ClientError::Unexpected)?.id
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
        let response = connection
            .request(|request| ClientMessage::OpenOrganization {
                request,
                organization,
            })
            .await?;
        let ServerMessage::OrganizationOpened {
            role,
            kind,
            escrows,
            pinned,
            server,
            ..
        } = response
        else {
            return Err(ClientError::Unexpected);
        };
        *self.session.lock().unwrap() = (role, kind);
        self.keys
            .set_server(server.account, RecoveryPublic(server.public));
        let verified =
            crate::contacts::contacts(&connection, self.account, self.keys.identity()).await?;
        let ServerMessage::RecoveryKey { public, .. } = connection
            .request(|request| ClientMessage::GetRecoveryKey { request })
            .await?
        else {
            return Err(ClientError::Unexpected);
        };
        if public.is_none() {
            connection
                .request(|request| ClientMessage::SetRecoveryKey {
                    request,
                    public: self.keys.identity().public().0,
                    sealed: Vec::new(),
                    own: Vec::new(),
                    changes: Vec::new(),
                })
                .await?;
        }
        self.keys.set_pinned(crate::contacts::open_pin(
            self.keys.identity(),
            organization,
            &pinned,
        ));
        let made = escrows.is_empty();
        self.keys.set_escrows(escrows);
        self.adopt_contacts(
            verified
                .into_iter()
                .filter_map(|contact| Some((contact.account, contact.public?))),
        );
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

    pub(crate) fn adopt_contacts(
        &self,
        verified: impl IntoIterator<Item = (Uuid, RecoveryPublic)>,
    ) {
        let verified: Vec<_> = verified.into_iter().collect();
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
        let ServerMessage::Keys { keys, .. } = self
            .connection()
            .request(|request| ClientMessage::GetKeys {
                request,
                blocks: blocks.clone(),
            })
            .await?
        else {
            return Err(ClientError::Unexpected);
        };
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
            let _ = self.refs.move_to_unsent(summary.id, now_milliseconds());
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
    ) -> Result<(Uuid, KeySet), ClientError> {
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
        self.fetch(&manifest.chunk_hashes()).await?;
        self.retrying(|| Ok(self.commits.vault().read(manifest)?))
            .await
    }

    pub async fn write_content(
        &self,
        block: Uuid,
        content_type: Uuid,
        data: &[u8],
    ) -> Result<Manifest, ClientError> {
        let key = self.sealing_key(block).await?;
        Ok(self.commits.vault().write(key, content_type, data)?)
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

    pub fn local_head(&self, block: Uuid) -> Option<CommitId> {
        self.refs.head(block)
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
        self.create_block(block, C::CONTENT_TYPE, parent, &BlockMetadata::default())
            .await
            .map(|summary| summary.id)
    }

    pub async fn create_block(
        &self,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
        metadata: &BlockMetadata,
    ) -> Result<BlockSummary, ClientError> {
        let (key, keys) = self.new_keys(block, parent).await?;
        let references = deduplicated(self.resolve(block, metadata.references()));
        let metadata = self.commits.vault().seal(key, &metadata.encode())?;
        let response = self
            .connection()
            .request(|request| ClientMessage::CreateBlock {
                request,
                block,
                content_type,
                parent,
                metadata: metadata.clone(),
                references: references.clone(),
                key,
                keys: keys.clone(),
            })
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
                });
            }
            response => response?,
        };
        match response {
            ServerMessage::Block { block, .. } => {
                self.recorded(&keys);
                self.note(&block);
                Ok(block)
            }
            _ => Err(ClientError::Unexpected),
        }
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
                let response = self
                    .connection()
                    .request(|request| ClientMessage::SetMetadata {
                        request,
                        block,
                        metadata: sealed.clone(),
                        references: references.clone(),
                    })
                    .await;
                match response {
                    Ok(ServerMessage::Block { block, .. }) => return Ok(block),
                    Ok(_) => return Err(ClientError::Unexpected),
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
            let saved = self
                .publish_manifest(
                    block,
                    commit.manifest,
                    references,
                    Some(head),
                    now_milliseconds(),
                    false,
                    Vec::new(),
                    &next,
                )
                .await;
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
            .create_block(block, content_type, parent, &BlockMetadata::default())
            .await
        {
            Ok(_) => Ok(()),
            Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub async fn summary(&self, block: Uuid) -> Result<BlockSummary, ClientError> {
        let response = self
            .connection()
            .request(|request| ClientMessage::ReadBlock { request, block })
            .await;
        match response {
            Ok(ServerMessage::Block { block, .. }) => {
                self.note(&block);
                Ok(block)
            }
            Ok(_) => Err(ClientError::Unexpected),
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

    async fn listed(
        &self,
        listing: Listing,
        message: impl FnOnce(u64) -> ClientMessage,
    ) -> Result<Vec<BlockSummary>, ClientError> {
        match self.connection().request(message).await {
            Ok(ServerMessage::Blocks { blocks, .. }) => {
                let _ = self.refs.set_listing(listing, &blocks);
                let _ = self.refs.observe(&blocks);
                Ok(blocks)
            }
            Ok(_) => Err(ClientError::Unexpected),
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

    pub async fn fetch(&self, hashes: &[Hash]) -> Result<(), ClientError> {
        use futures_util::StreamExt;
        let store = self.commits.vault().store();
        let mut seen = std::collections::HashSet::new();
        let mut missing = Vec::new();
        for hash in hashes {
            if seen.insert(*hash) && !store.has(*hash)? {
                missing.push(*hash);
            }
        }
        let connection = self.connection();
        let connection = &connection;
        let mut answers = futures_util::stream::iter(missing.into_iter().map(|hash| async move {
            let answer = connection
                .request(|request| ClientMessage::GetObject { request, hash })
                .await;
            (hash, answer)
        }))
        .buffered(FETCHING);
        while let Some((hash, answer)) = answers.next().await {
            let ServerMessage::Object { bytes, .. } = answer? else {
                return Err(ClientError::Unexpected);
            };
            let bytes = bytes.ok_or(ClientError::MissingObject(hash))?;
            if store.put(&bytes)? != hash {
                return Err(ClientError::Tampered(hash));
            }
        }
        Ok(())
    }

    pub async fn push(&self, hashes: &[Hash]) -> Result<(), ClientError> {
        self.commits.vault().store().sync()?;
        let response = self
            .connection()
            .request(|request| ClientMessage::MissingObjects {
                request,
                hashes: hashes.to_vec(),
            })
            .await?;
        let ServerMessage::Missing {
            hashes: missing, ..
        } = response
        else {
            return Err(ClientError::Unexpected);
        };
        let mut pending = std::collections::VecDeque::new();
        for group in self.object_groups(&missing)? {
            if pending.len() >= import::IN_FLIGHT
                && let Some(sent) = pending.pop_front()
            {
                sent.await?;
            }
            pending.push_back(self.send_objects(group)?);
        }
        for sent in pending {
            sent.await?;
        }
        Ok(())
    }

    pub async fn try_fetch(&self, hash: Hash) -> Result<bool, ClientError> {
        if self.commits.vault().store().has(hash)? {
            return Ok(true);
        }
        let response = self
            .connection()
            .request(|request| ClientMessage::GetObject { request, hash })
            .await?;
        let ServerMessage::Object { bytes, .. } = response else {
            return Err(ClientError::Unexpected);
        };
        let Some(bytes) = bytes else {
            return Ok(false);
        };
        if self.commits.vault().store().put(&bytes)? != hash {
            return Err(ClientError::Tampered(hash));
        }
        Ok(true)
    }

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

    pub async fn read_contents(
        &self,
        blocks: &[(Uuid, Option<CommitId>)],
    ) -> Result<Vec<(BlockSummary, Option<Vec<u8>>)>, ClientError> {
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
    ) -> Result<Vec<(BlockSummary, Option<Vec<u8>>)>, ClientError> {
        let mut asked = Vec::new();
        for group in blocks.chunks(CONTENTS_BLOCKS) {
            let ids: Vec<Uuid> = group.iter().map(|(id, _)| *id).collect();
            let held = group
                .iter()
                .filter_map(|(_, head)| *head)
                .filter(|head| self.holds(*head))
                .collect();
            let answer = self
                .connection()
                .send(|request| ClientMessage::ReadContents {
                    request,
                    blocks: ids.clone(),
                    held,
                })?;
            asked.push((ids, answer));
        }
        let mut summaries = Vec::with_capacity(blocks.len());
        for (ids, answer) in asked {
            let ServerMessage::Contents {
                blocks, objects, ..
            } = answer.await?
            else {
                return Err(ClientError::Unexpected);
            };
            if !blocks.iter().map(|summary| summary.id).eq(ids) {
                return Err(ClientError::Unexpected);
            }
            let store = self.commits.vault().store();
            for object in &objects {
                store.put(object)?;
            }
            for summary in &blocks {
                self.note(summary);
            }
            summaries.extend(blocks);
        }
        futures_util::future::try_join_all(summaries.into_iter().map(|summary| async move {
            let content = match summary.head {
                Some(head) => {
                    let commit = self.load_commit(head).await?;
                    Some(self.read_manifest(&commit.manifest).await?)
                }
                None => None,
            };
            Ok((summary, content))
        }))
        .await
    }

    fn holds(&self, head: CommitId) -> bool {
        let store = self.commits.vault().store();
        matches!(
            self.commits.try_get(head),
            Ok(Some(commit)) if commit
                .manifest
                .chunk_hashes()
                .into_iter()
                .all(|hash| store.has(hash).unwrap_or(false))
        )
    }

    pub async fn open<C: BlockContent>(&self, block: Uuid) -> Result<Option<C>, ClientError> {
        if let Some(head) = self.local_head(block)
            && !self.pending_commits(block).is_empty()
        {
            return Ok(Some(C::decode(&self.read_at(head).await?)?));
        }
        let read = self
            .read_contents(&[(block, self.local_head(block))])
            .await?;
        let Some((summary, Some(bytes))) = read.into_iter().next() else {
            return Ok(None);
        };
        if let Some(head) = summary.head {
            self.remember(block, head)?;
        }
        Ok(Some(C::decode(&bytes)?))
    }

    pub async fn open_commit<C: BlockContent>(&self, commit: CommitId) -> Result<C, ClientError> {
        let commit = self.load_commit(commit).await?;
        Ok(C::decode(&self.read_manifest(&commit.manifest).await?)?)
    }

    pub async fn save<C: BlockContent>(
        &self,
        block: Uuid,
        content: &C,
        expected: Option<CommitId>,
    ) -> Result<Saved, ClientError> {
        self.save_content(block, content, expected, false).await
    }

    pub async fn bookmark<C: BlockContent>(
        &self,
        block: Uuid,
        content: &C,
        expected: Option<CommitId>,
        label: &str,
    ) -> Result<Saved, ClientError> {
        let _ = label;
        self.save_content(block, content, expected, true).await
    }

    async fn save_content<C: BlockContent>(
        &self,
        block: Uuid,
        content: &C,
        expected: Option<CommitId>,
        pinned: bool,
    ) -> Result<Saved, ClientError> {
        self.save_bytes(
            block,
            C::CONTENT_TYPE,
            &content.encode(),
            content.references_in(self.organization()),
            derived_metadata(content),
            expected,
            now_milliseconds(),
            pinned,
            Vec::new(),
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn publish_encoded(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: &[u8],
        references: Vec<Uuid>,
        derived: Entries,
        expected: Option<CommitId>,
        merging: Option<(Option<CommitId>, CommitId)>,
    ) -> Result<Saved, ClientError> {
        let (parents, defined) = self.merging(expected, merging).await?;
        self.save_bytes(
            block,
            content_type,
            bytes,
            references,
            derived,
            expected,
            now_milliseconds(),
            false,
            parents,
            defined,
        )
        .await
    }

    pub async fn stash_encoded(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: &[u8],
        references: Vec<Uuid>,
        derived: Entries,
        parent: Option<CommitId>,
    ) -> Result<CommitId, ClientError> {
        let metadata = self.metadata_at(block, parent).await?.with_derived(derived);
        let mut references = references;
        references.extend(metadata.references());
        let references = deduplicated(self.resolve(block, references));
        let key = self.sealing_key(block).await?;
        let manifest = self.commits.vault().write(key, content_type, bytes)?;
        let commit = Commit::new(manifest, self.account, now_milliseconds())
            .with_parent(parent)
            .with_references(references)
            .with_metadata(metadata.encode());
        let id = self.commits.put(key, &commit)?;
        let mut objects = vec![id.hash()];
        objects.extend(commit.manifest.chunk_hashes());
        match self.push(&objects).await {
            Err(error) if offline::unreachable(&error) => self.commits.vault().store().sync()?,
            pushed => pushed?,
        }
        Ok(id)
    }

    pub async fn save_encoded(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: &[u8],
        references: Vec<Uuid>,
        derived: Entries,
        expected: Option<CommitId>,
        defined: Option<Entries>,
    ) -> Result<Saved, ClientError> {
        self.save_bytes(
            block,
            content_type,
            bytes,
            references,
            derived,
            expected,
            now_milliseconds(),
            false,
            Vec::new(),
            defined,
        )
        .await
    }

    async fn save_bytes(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: &[u8],
        references: Vec<Uuid>,
        derived: Entries,
        expected: Option<CommitId>,
        time: i64,
        pinned: bool,
        extra_parents: Vec<CommitId>,
        defined: Option<Entries>,
    ) -> Result<Saved, ClientError> {
        let (manifest, references, metadata) = self
            .prepare_bytes(
                block,
                content_type,
                bytes,
                references,
                derived,
                expected,
                defined,
            )
            .await?;
        self.publish_manifest(
            block,
            manifest,
            references,
            expected,
            time,
            pinned,
            extra_parents,
            &metadata,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn prepare_bytes(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: &[u8],
        references: Vec<Uuid>,
        derived: Entries,
        expected: Option<CommitId>,
        defined: Option<Entries>,
    ) -> Result<(Manifest, Vec<Uuid>, BlockMetadata), ClientError> {
        let references = self.resolve(block, references);
        let manifest = self.write_content(block, content_type, bytes).await?;
        let defined = match defined {
            Some(defined) => defined,
            None => self.metadata_at(block, expected).await?.defined,
        };
        Ok((manifest, references, BlockMetadata { defined, derived }))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn seal_encoded(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: &[u8],
        references: Vec<Uuid>,
        derived: Entries,
        expected: Option<CommitId>,
        merging: Option<(Option<CommitId>, CommitId)>,
    ) -> Result<Sealed, ClientError> {
        let (parents, defined) = self.merging(expected, merging).await?;
        let (manifest, references, metadata) = self
            .prepare_bytes(
                block,
                content_type,
                bytes,
                references,
                derived,
                expected,
                defined,
            )
            .await?;
        self.seal_manifest(
            block,
            manifest,
            references,
            expected,
            now_milliseconds(),
            false,
            parents,
            &metadata,
        )
        .await
    }

    async fn merging(
        &self,
        expected: Option<CommitId>,
        merging: Option<(Option<CommitId>, CommitId)>,
    ) -> Result<(Vec<CommitId>, Option<Entries>), ClientError> {
        let Some((base, ours)) = merging else {
            return Ok((Vec::new(), None));
        };
        let mut sides = Vec::new();
        for commit in [base, Some(ours), expected] {
            sides.push(match commit {
                Some(commit) => self.metadata_of(commit).await?,
                None => BlockMetadata::default(),
            });
        }
        let defined = BlockMetadata::merge_defined(&sides[0], &sides[2], &sides[1]).0;
        Ok((vec![ours], Some(defined)))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn publish_manifest(
        &self,
        block: Uuid,
        manifest: Manifest,
        references: Vec<Uuid>,
        expected: Option<CommitId>,
        time: i64,
        pinned: bool,
        extra_parents: Vec<CommitId>,
        metadata: &BlockMetadata,
    ) -> Result<Saved, ClientError> {
        let before = self.local_head(block);
        let commit = match self
            .seal_manifest(
                block,
                manifest,
                references,
                expected,
                time,
                pinned,
                extra_parents,
                metadata,
            )
            .await?
        {
            Sealed::Unchanged(head) => return Ok(Saved::Unchanged(head)),
            Sealed::Sealed(commit) => commit,
        };
        let saved = self.publish_sealed(block, commit).await?;
        if let Saved::Rejected { .. } = saved {
            self.withdraw(block, commit, before)?;
        }
        Ok(saved)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn seal_manifest(
        &self,
        block: Uuid,
        manifest: Manifest,
        references: Vec<Uuid>,
        expected: Option<CommitId>,
        time: i64,
        pinned: bool,
        extra_parents: Vec<CommitId>,
        metadata: &BlockMetadata,
    ) -> Result<Sealed, ClientError> {
        let previous = match expected {
            Some(head) => Some(self.load_commit(head).await?),
            None => None,
        };
        let mut references = references;
        references.extend(self.resolve(block, metadata.references()));
        let references = deduplicated(references);
        let encoded = metadata.encode();
        if extra_parents.is_empty()
            && let Some(previous) = &previous
            && previous.manifest == manifest
            && previous.references == references
            && previous.metadata == encoded
        {
            return Ok(Sealed::Unchanged(expected.expect("a previous commit")));
        }
        let mut parents: Vec<_> = expected.into_iter().collect();
        let mut chunks = manifest.chunk_hashes();
        for parent in &extra_parents {
            chunks.push(parent.hash());
            chunks.extend(self.commits.get(*parent)?.manifest.chunk_hashes());
        }
        parents.extend(extra_parents);
        let kind = if parents.len() > 1 {
            be_commit::CommitKind::Merge
        } else if pinned {
            be_commit::CommitKind::Bookmark(String::new())
        } else {
            be_commit::CommitKind::Autosave
        };
        let key = self.sealing_key(block).await?;
        let commit = Commit::new(manifest, self.account, time)
            .with_parents(parents.clone())
            .with_kind(kind)
            .with_references(references.clone())
            .with_metadata(encoded);
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
            references,
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
        let mut hashes = vec![id.hash()];
        hashes.extend(chunks.iter().copied());
        let mut carried = match merging {
            true => None,
            false => self.carried(id, &manifest, previous.as_ref())?,
        };
        if carried.is_none() {
            self.push(&hashes).await?;
        }
        let response = loop {
            let publish = |request, objects| ClientMessage::Publish {
                request,
                block,
                commit: id,
                expected,
                parents: parents.clone(),
                chunks: chunks.clone(),
                time,
                pinned,
                references: references.clone(),
                metadata: metadata.clone(),
                objects,
            };
            match carried.take() {
                Some(objects) => {
                    match self
                        .connection()
                        .request(|request| publish(request, objects))
                        .await
                    {
                        Err(ClientError::Refused(ErrorCode::ObjectNotFound, _)) => {
                            self.push(&hashes).await?;
                        }
                        response => break response?,
                    }
                }
                None => {
                    break self
                        .connection()
                        .request(|request| publish(request, Vec::new()))
                        .await?;
                }
            }
        };
        match response {
            ServerMessage::Published { head, .. } => Ok(Saved::Published(head)),
            ServerMessage::Rejected { head, .. } => {
                if let Some(head) = head {
                    self.refs.set_remote(block, head)?;
                }
                Ok(Saved::Rejected { head })
            }
            _ => Err(ClientError::Unexpected),
        }
    }

    fn carried(
        &self,
        commit: CommitId,
        manifest: &Manifest,
        previous: Option<&Commit>,
    ) -> Result<Option<Vec<ByteBuf>>, ClientError> {
        let store = self.commits.vault().store();
        store.sync()?;
        let mut sent: HashSet<Hash> = previous
            .map(|previous| previous.manifest.chunk_hashes())
            .unwrap_or_default()
            .into_iter()
            .collect();
        let mut hashes = vec![commit.hash()];
        for hash in manifest.chunk_hashes() {
            if sent.insert(hash) {
                hashes.push(hash);
            }
        }
        let mut room = CARRIED_BYTES;
        let mut carried = Vec::with_capacity(hashes.len());
        for hash in hashes {
            let bytes = store.get(hash)?.ok_or(ClientError::MissingObject(hash))?;
            if bytes.len() > room {
                return Ok(None);
            }
            room -= bytes.len();
            carried.push(ByteBuf::from(bytes));
        }
        Ok(Some(carried))
    }

    pub async fn hold(&self, block: Uuid, objects: &[Hash]) -> Result<(), ClientError> {
        if objects.is_empty() {
            return Ok(());
        }
        self.push(objects).await?;
        self.connection()
            .request(|request| ClientMessage::HoldObjects {
                request,
                block,
                objects: objects.to_vec(),
            })
            .await?;
        Ok(())
    }

    pub async fn watch(&self, block: Uuid) -> Result<(), ClientError> {
        self.connection()
            .request(|request| ClientMessage::Watch { request, block })
            .await?;
        Ok(())
    }

    pub async fn unwatch(&self, block: Uuid) -> Result<(), ClientError> {
        self.connection()
            .request(|request| ClientMessage::Unwatch { request, block })
            .await?;
        Ok(())
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
        let needed: Vec<_> = self
            .commits
            .vault()
            .chunks_in_range(manifest, offset, length)
            .into_iter()
            .map(|chunk| chunk.hash)
            .collect();
        self.fetch(&needed).await?;
        self.retrying(|| Ok(self.commits.vault().read_range(manifest, offset, length)?))
            .await
    }

    pub async fn history(&self, block: Uuid) -> Result<Vec<HistoryEntry>, ClientError> {
        let response = self
            .connection()
            .request(|request| ClientMessage::ListHistory { request, block })
            .await?;
        match response {
            ServerMessage::History { entries, .. } => Ok(entries),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn prune(&self, block: Uuid, policy: &RetentionPolicy) -> Result<usize, ClientError> {
        let kept_for = self.organization_policies().await?.history_kept_for();
        let policy = &policy.clone().keeping_all_within(kept_for);
        let entries = self.history(block).await?;
        let summaries: Vec<_> = entries
            .iter()
            .map(|entry| CommitSummary {
                id: entry.commit,
                time: entry.time,
                pinned: entry.pinned,
            })
            .collect();
        let plan = plan_retention(&summaries, now_milliseconds(), policy);
        if plan.drop.is_empty() {
            return Ok(0);
        }
        let response = self
            .connection()
            .request(|request| ClientMessage::PruneHistory {
                request,
                block,
                drop: plan.drop.clone(),
            })
            .await?;
        match response {
            ServerMessage::Collected { objects, .. } => Ok(objects),
            _ => Err(ClientError::Unexpected),
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
        self.connection()
            .request(|request| ClientMessage::SetParent {
                request,
                block,
                parent,
                keys: keys.clone(),
            })
            .await?;
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
        public: &be_keys::RecoveryPublic,
    ) -> Result<(), ClientError> {
        self.grant(block, account, access).await?;
        if !access.can_view() {
            return Ok(());
        }
        self.seal_share(block, account, public).await
    }

    pub async fn open_inside(&self, block: Uuid, access: Access) -> Result<(), ClientError> {
        self.grant(block, be_graph::ANYONE_INSIDE, access).await?;
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
        public: &be_keys::RecoveryPublic,
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
        self.connection()
            .request(|request| ClientMessage::PutKeys {
                request,
                keys: keys.clone(),
            })
            .await?;
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
                if node.id != block && node.scope == Some(node.id) {
                    if let Some(nested) = self.keys.scope_key(node.id)
                        && !wrapped_under(nested)
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
        self.listed(Listing::Shared, |request| ClientMessage::ListShared {
            request,
        })
        .await
    }

    pub async fn children(&self, parent: BlockParent) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::Children(parent), |request| {
            ClientMessage::ListChildren { request, parent }
        })
        .await
    }

    pub async fn deleted(&self) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::Deleted, |request| ClientMessage::ListDeleted {
            request,
        })
        .await
    }

    pub async fn unlist(&self, listing: Listing) -> Result<(), ClientError> {
        match self
            .connection()
            .request(|request| ClientMessage::Unlist { request, listing })
            .await
        {
            Ok(_) => Ok(()),
            Err(error) if offline::unreachable(&error) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub async fn backrefs(&self, block: Uuid) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::Backrefs(block), |request| {
            ClientMessage::ListBackrefs { request, block }
        })
        .await
    }

    pub async fn open_settings(&self, block: Uuid) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::OpenSettings(block), |request| {
            ClientMessage::ListOpenSettings { request, block }
        })
        .await
    }

    pub async fn referenced(&self, block: Uuid) -> Result<Vec<BlockSummary>, ClientError> {
        self.listed(Listing::References(block), |request| {
            ClientMessage::ListReferences { request, block }
        })
        .await
    }

    pub async fn delete(&self, block: Uuid) -> Result<(), ClientError> {
        self.connection()
            .request(|request| ClientMessage::DeleteBlock { request, block })
            .await?;
        Ok(())
    }

    pub async fn collect_detached(&self) -> Result<Vec<Uuid>, ClientError> {
        let response = self
            .connection()
            .request(|request| ClientMessage::CollectDetached { request })
            .await?;
        match response {
            ServerMessage::Collected { blocks, .. } => Ok(blocks),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn grant(
        &self,
        block: Uuid,
        account: Uuid,
        access: Access,
    ) -> Result<(), ClientError> {
        self.connection()
            .request(|request| ClientMessage::SetAccess {
                request,
                block,
                account,
                access,
            })
            .await?;
        Ok(())
    }

    pub async fn list_access(&self, block: Uuid) -> Result<AccessList, ClientError> {
        let response = self
            .connection()
            .request(|request| ClientMessage::ListAccess { request, block })
            .await?;
        match response {
            ServerMessage::AccessList { list, .. } => Ok(list),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn join_session(&self, block: Uuid) -> Result<(ClientId, SessionState), ClientError> {
        let response = self
            .connection()
            .request(|request| ClientMessage::JoinSession { request, block })
            .await?;
        match response {
            ServerMessage::Session { client, state, .. } => Ok((client, state)),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn leave_session(&self, block: Uuid) -> Result<(), ClientError> {
        self.connection()
            .request(|request| ClientMessage::LeaveSession { request, block })
            .await?;
        Ok(())
    }

    pub async fn claim_ownership(
        &self,
        block: Uuid,
        generation: u64,
    ) -> Result<SessionState, ClientError> {
        let response = self
            .connection()
            .request(|request| ClientMessage::ClaimOwnership {
                request,
                block,
                generation,
            })
            .await?;
        match response {
            ServerMessage::Session { state, .. } => Ok(state),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub fn heartbeat(
        &self,
        block: Uuid,
        generation: u64,
        clean_at: Option<CommitId>,
    ) -> Result<impl Future<Output = Result<SessionState, ClientError>> + Send + use<S>, ClientError>
    {
        let answer = self.connection().send(|request| ClientMessage::Heartbeat {
            request,
            block,
            generation,
            clean_at,
        })?;
        Ok(async move {
            match answer.await? {
                ServerMessage::Session { state, .. } => Ok(state),
                _ => Err(ClientError::Unexpected),
            }
        })
    }

    pub async fn relay(
        &self,
        block: Uuid,
        to: Option<ClientId>,
        payload: &[u8],
    ) -> Result<Answer, ClientError> {
        let key = self.sealing_key(block).await?;
        let sealed = self.commits.vault().seal(key, payload)?;
        self.connection().send(|request| ClientMessage::Relay {
            request,
            block,
            to,
            payload: sealed,
        })
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
