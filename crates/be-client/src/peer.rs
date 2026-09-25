use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use be_block::{BlockContent, BlockMetadata, Streamed, payload_start};
use be_commit::{
    Commit, CommitId, CommitStore, CommitSummary, RetentionPolicy, now_milliseconds,
    plan_retention, reference_delta,
};
use be_graph::{Access, BlockParent};
use be_protocol::{
    AccessEntry, BlockSummary, ClientId, ClientMessage, ErrorCode, HistoryEntry, ServerMessage,
    SessionState, WorkspaceRole,
};
use be_store::{ChunkerConfig, ContentKey, Hash, Manifest, ObjectStore, Vault};
use uuid::Uuid;

use crate::{ClientError, connection::Connection};

pub enum Credentials {
    Register {
        email: String,
        display_name: String,
        password: String,
    },
    Login {
        email: String,
        password: String,
    },
    Token(String),
}

impl Credentials {
    fn message(self, request: u64) -> ClientMessage {
        match self {
            Self::Register {
                email,
                display_name,
                password,
            } => ClientMessage::Register {
                request,
                email,
                display_name,
                password,
            },
            Self::Login { email, password } => ClientMessage::Login {
                request,
                email,
                password,
            },
            Self::Token(token) => ClientMessage::Authenticate { request, token },
        }
    }
}

pub struct PeerConfig {
    pub url: String,
    pub key: ContentKey,
    pub credentials: Credentials,
    pub workspace: Option<Uuid>,
    pub workspace_name: String,
    pub chunker: ChunkerConfig,
}

impl PeerConfig {
    pub fn new(url: impl Into<String>, key: ContentKey, credentials: Credentials) -> Self {
        Self {
            url: url.into(),
            key,
            credentials,
            workspace: None,
            workspace_name: "Workspace".into(),
            chunker: ChunkerConfig::DEFAULT,
        }
    }

    pub fn workspace(mut self, workspace: Option<Uuid>) -> Self {
        self.workspace = workspace;
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

pub struct Peer<S: ObjectStore> {
    connection: Arc<Connection>,
    commits: CommitStore<S>,
    token: String,
    account: Uuid,
    workspace: Uuid,
    role: WorkspaceRole,
    heads: Mutex<HashMap<Uuid, CommitId>>,
}

impl<S: ObjectStore> Peer<S> {
    pub async fn connect(config: PeerConfig, store: S) -> Result<Self, ClientError> {
        let connection = Connection::connect(&config.url).await?;
        let response = connection
            .request(|request| config.credentials.message(request))
            .await?;
        let ServerMessage::Authenticated { account, token, .. } = response else {
            return Err(ClientError::Unexpected);
        };
        let workspace = match config.workspace {
            Some(workspace) => workspace,
            None => {
                let response = connection
                    .request(|request| ClientMessage::CreateWorkspace {
                        request,
                        name: config.workspace_name.clone(),
                    })
                    .await?;
                let ServerMessage::Workspaces { workspaces, .. } = response else {
                    return Err(ClientError::Unexpected);
                };
                workspaces.first().ok_or(ClientError::Unexpected)?.id
            }
        };
        let response = connection
            .request(|request| ClientMessage::OpenWorkspace { request, workspace })
            .await?;
        let ServerMessage::WorkspaceOpened { role, .. } = response else {
            return Err(ClientError::Unexpected);
        };
        Ok(Self {
            connection,
            commits: CommitStore::new(Vault::new(store, config.key).with_chunker(config.chunker)),
            token,
            account,
            workspace,
            role,
            heads: Mutex::new(HashMap::new()),
        })
    }

    pub fn account(&self) -> Uuid {
        self.account
    }

    pub fn workspace(&self) -> Uuid {
        self.workspace
    }

    pub fn role(&self) -> WorkspaceRole {
        self.role
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn commits(&self) -> &CommitStore<S> {
        &self.commits
    }

    pub fn connection(&self) -> &Arc<Connection> {
        &self.connection
    }

    pub fn local_head(&self, block: Uuid) -> Option<CommitId> {
        self.heads.lock().unwrap().get(&block).copied()
    }

    pub fn remember(&self, block: Uuid, head: CommitId) {
        self.heads.lock().unwrap().insert(block, head);
    }

    pub async fn create<C: BlockContent>(&self, parent: BlockParent) -> Result<Uuid, ClientError> {
        self.create_with_id::<C>(Uuid::new_v4(), parent).await
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
        let metadata = self.seal_metadata(metadata);
        let response = self
            .connection
            .request(|request| ClientMessage::CreateBlock {
                request,
                block,
                content_type,
                parent,
                metadata: metadata.clone(),
            })
            .await?;
        match response {
            ServerMessage::Block { block, .. } => Ok(block),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn list_blocks(&self) -> Result<Vec<BlockSummary>, ClientError> {
        let response = self
            .connection
            .request(|request| ClientMessage::ListBlocks { request })
            .await?;
        match response {
            ServerMessage::Blocks { blocks, .. } => Ok(blocks),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn set_metadata(
        &self,
        block: Uuid,
        metadata: &BlockMetadata,
    ) -> Result<BlockSummary, ClientError> {
        let metadata = self.seal_metadata(metadata);
        let response = self
            .connection
            .request(|request| ClientMessage::SetMetadata {
                request,
                block,
                metadata: metadata.clone(),
            })
            .await?;
        match response {
            ServerMessage::Block { block, .. } => Ok(block),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub fn metadata(&self, summary: &BlockSummary) -> BlockMetadata {
        if summary.metadata.is_empty() {
            return BlockMetadata::default();
        }
        self.unseal(&summary.metadata)
            .map(|plain| BlockMetadata::decode(&plain))
            .unwrap_or_default()
    }

    fn seal_metadata(&self, metadata: &BlockMetadata) -> Vec<u8> {
        self.commits.vault().seal(&metadata.encode())
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

    pub async fn summary(&self, block: Uuid) -> Result<BlockSummary, ClientError> {
        let response = self
            .connection
            .request(|request| ClientMessage::ReadBlock { request, block })
            .await?;
        match response {
            ServerMessage::Block { block, .. } => Ok(block),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn remote_head(&self, block: Uuid) -> Result<Option<CommitId>, ClientError> {
        Ok(self.summary(block).await?.head)
    }

    pub async fn fetch(&self, hashes: &[Hash]) -> Result<(), ClientError> {
        for hash in hashes {
            if self.commits.vault().store().has(*hash)? {
                continue;
            }
            let response = self
                .connection
                .request(|request| ClientMessage::GetObject {
                    request,
                    hash: *hash,
                })
                .await?;
            let ServerMessage::Object { bytes, .. } = response else {
                return Err(ClientError::Unexpected);
            };
            let bytes = bytes.ok_or(ClientError::MissingObject(*hash))?;
            if self.commits.vault().store().put(&bytes)? != *hash {
                return Err(ClientError::Tampered(*hash));
            }
        }
        Ok(())
    }

    pub async fn push(&self, hashes: &[Hash]) -> Result<(), ClientError> {
        let response = self
            .connection
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
        for hash in missing {
            let bytes = self
                .commits
                .vault()
                .store()
                .get(hash)?
                .ok_or(ClientError::MissingObject(hash))?;
            let response = self
                .connection
                .request(|request| ClientMessage::PutObject {
                    request,
                    bytes: bytes.clone(),
                })
                .await?;
            match response {
                ServerMessage::Stored { hash: stored, .. } if stored == hash => {}
                _ => return Err(ClientError::Unexpected),
            }
        }
        Ok(())
    }

    pub async fn try_fetch(&self, hash: Hash) -> Result<bool, ClientError> {
        if self.commits.vault().store().has(hash)? {
            return Ok(true);
        }
        let response = self
            .connection
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
        Ok(self.commits.get(commit)?)
    }

    pub async fn open<C: BlockContent>(&self, block: Uuid) -> Result<Option<C>, ClientError> {
        let Some(head) = self.remote_head(block).await? else {
            return Ok(None);
        };
        let commit = self.load_commit(head).await?;
        self.fetch(&commit.manifest.chunk_hashes()).await?;
        self.remember(block, head);
        Ok(Some(C::decode(
            &self.commits.vault().read(&commit.manifest)?,
        )?))
    }

    pub async fn open_commit<C: BlockContent>(&self, commit: CommitId) -> Result<C, ClientError> {
        let commit = self.load_commit(commit).await?;
        self.fetch(&commit.manifest.chunk_hashes()).await?;
        Ok(C::decode(&self.commits.vault().read(&commit.manifest)?)?)
    }

    pub async fn save<C: BlockContent>(
        &self,
        block: Uuid,
        content: &C,
        expected: Option<CommitId>,
    ) -> Result<Saved, ClientError> {
        self.save_at(
            block,
            content,
            expected,
            now_milliseconds(),
            false,
            Vec::new(),
        )
        .await
    }

    pub async fn bookmark<C: BlockContent>(
        &self,
        block: Uuid,
        content: &C,
        expected: Option<CommitId>,
        label: &str,
    ) -> Result<Saved, ClientError> {
        let _ = label;
        self.save_at(
            block,
            content,
            expected,
            now_milliseconds(),
            true,
            Vec::new(),
        )
        .await
    }

    pub async fn save_merge<C: BlockContent>(
        &self,
        block: Uuid,
        content: &C,
        expected: Option<CommitId>,
        parents: Vec<CommitId>,
    ) -> Result<Saved, ClientError> {
        self.save_at(block, content, expected, now_milliseconds(), false, parents)
            .await
    }

    async fn save_at<C: BlockContent>(
        &self,
        block: Uuid,
        content: &C,
        expected: Option<CommitId>,
        time: i64,
        pinned: bool,
        extra_parents: Vec<CommitId>,
    ) -> Result<Saved, ClientError> {
        let previous = match expected {
            Some(head) => Some(self.load_commit(head).await?),
            None => None,
        };
        let references = content.references_in(self.workspace());
        let manifest = self
            .commits
            .vault()
            .write(C::CONTENT_TYPE, &content.encode())?;
        if let Some(previous) = &previous
            && previous.manifest == manifest
            && previous.references == sorted(references.clone())
        {
            return Ok(Saved::Unchanged(expected.expect("a previous commit")));
        }
        let mut parents: Vec<_> = expected.into_iter().collect();
        parents.extend(extra_parents);
        let kind = if parents.len() > 1 {
            be_commit::CommitKind::Merge
        } else if pinned {
            be_commit::CommitKind::Bookmark(String::new())
        } else {
            be_commit::CommitKind::Autosave
        };
        let commit = Commit::new(manifest.clone(), self.account, time)
            .with_parents(parents)
            .with_kind(kind)
            .with_references(references.clone());
        let id = self.commits.put(&commit)?;

        let chunks = manifest.chunk_hashes();
        let mut objects = vec![id.hash()];
        objects.extend(chunks.iter().copied());
        self.push(&objects).await?;

        let before = previous
            .map(|previous| previous.references)
            .unwrap_or_default();
        let (added, removed) = reference_delta(&before, &references);
        let response = self
            .connection
            .request(|request| ClientMessage::Publish {
                request,
                block,
                commit: id,
                expected,
                chunks: chunks.clone(),
                time,
                pinned,
                references_added: added.clone(),
                references_removed: removed.clone(),
            })
            .await?;
        match response {
            ServerMessage::Published { head, .. } => {
                self.remember(block, head);
                Ok(Saved::Published(head))
            }
            ServerMessage::Rejected { head, .. } => Ok(Saved::Rejected { head }),
            _ => Err(ClientError::Unexpected),
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
        Ok(self.commits.vault().read_range(manifest, offset, length)?)
    }

    pub async fn history(&self, block: Uuid) -> Result<Vec<HistoryEntry>, ClientError> {
        let response = self
            .connection
            .request(|request| ClientMessage::ListHistory { request, block })
            .await?;
        match response {
            ServerMessage::History { entries, .. } => Ok(entries),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn prune(&self, block: Uuid, policy: &RetentionPolicy) -> Result<usize, ClientError> {
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
            .connection
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
        self.connection
            .request(|request| ClientMessage::SetParent {
                request,
                block,
                parent,
            })
            .await?;
        Ok(())
    }

    pub async fn children(&self, parent: BlockParent) -> Result<Vec<BlockSummary>, ClientError> {
        let response = self
            .connection
            .request(|request| ClientMessage::ListChildren { request, parent })
            .await?;
        match response {
            ServerMessage::Blocks { blocks, .. } => Ok(blocks),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn backrefs(&self, block: Uuid) -> Result<Vec<BlockSummary>, ClientError> {
        let response = self
            .connection
            .request(|request| ClientMessage::ListBackrefs { request, block })
            .await?;
        match response {
            ServerMessage::Blocks { blocks, .. } => Ok(blocks),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn delete(&self, block: Uuid) -> Result<(), ClientError> {
        self.connection
            .request(|request| ClientMessage::DeleteBlock { request, block })
            .await?;
        Ok(())
    }

    pub async fn collect_detached(&self) -> Result<Vec<Uuid>, ClientError> {
        let response = self
            .connection
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
        self.connection
            .request(|request| ClientMessage::SetAccess {
                request,
                block,
                account,
                access,
            })
            .await?;
        Ok(())
    }

    pub async fn list_access(&self, block: Uuid) -> Result<Vec<AccessEntry>, ClientError> {
        let response = self
            .connection
            .request(|request| ClientMessage::ListAccess { request, block })
            .await?;
        match response {
            ServerMessage::AccessList { entries, .. } => Ok(entries),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn join_session(&self, block: Uuid) -> Result<(ClientId, SessionState), ClientError> {
        let response = self
            .connection
            .request(|request| ClientMessage::JoinSession { request, block })
            .await?;
        match response {
            ServerMessage::Session { client, state, .. } => Ok((client, state)),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn leave_session(&self, block: Uuid) -> Result<(), ClientError> {
        self.connection
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
            .connection
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

    pub async fn heartbeat(
        &self,
        block: Uuid,
        generation: u64,
        clean_at: Option<CommitId>,
    ) -> Result<SessionState, ClientError> {
        let response = self
            .connection
            .request(|request| ClientMessage::Heartbeat {
                request,
                block,
                generation,
                clean_at,
            })
            .await?;
        match response {
            ServerMessage::Session { state, .. } => Ok(state),
            _ => Err(ClientError::Unexpected),
        }
    }

    pub async fn relay(
        &self,
        block: Uuid,
        to: Option<ClientId>,
        payload: &[u8],
    ) -> Result<(), ClientError> {
        let sealed = self.commits.vault().seal(payload);
        self.connection
            .request(|request| ClientMessage::Relay {
                request,
                block,
                to,
                payload: sealed.clone(),
            })
            .await?;
        Ok(())
    }

    pub fn unseal(&self, payload: &[u8]) -> Result<Vec<u8>, ClientError> {
        Ok(self.commits.vault().open(payload)?)
    }
}

fn sorted(mut references: Vec<Uuid>) -> Vec<Uuid> {
    references.sort_unstable();
    references.dedup();
    references
}
