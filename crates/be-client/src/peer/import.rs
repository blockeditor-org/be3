use std::{collections::HashMap, future::Future, pin::Pin};

use be_commit::{Commit, CommitKind, now_milliseconds};
use be_graph::BlockParent;
use be_protocol::{BlockSummary, ByteBuf, ClientMessage, KeySet, ServerMessage};
use be_store::{Hash, ObjectStore};
use uuid::Uuid;

use super::{Peer, deduplicated};
use crate::{
    ClientError,
    refs::{Creation, Op, Pending},
    transfer::Saved,
};

const BATCH_BLOCKS: usize = 64;
const OBJECT_BATCH_BYTES: usize = 2 * 1024 * 1024;
pub(super) const IN_FLIGHT: usize = 8;

pub type Imported = Result<BlockSummary, ClientError>;

struct Prepared {
    id: Uuid,
    keys: KeySet,
    creation: Creation,
    publish: Option<Pending>,
    objects: Vec<Hash>,
    queued: Vec<u64>,
}

pub struct Queued {
    prepared: Vec<Prepared>,
}

impl Queued {
    pub fn blocks(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.prepared.iter().map(|ready| ready.id)
    }
}

type Sent<'a> = Pin<Box<dyn Future<Output = Result<Vec<(Uuid, Imported)>, ClientError>> + 'a>>;

impl<S: ObjectStore> Peer<S> {
    pub async fn import_blocks(
        &self,
        blocks: &[(Saved, BlockParent)],
        mut done: impl FnMut(Uuid, Imported),
    ) {
        let queued = self.queue_import(blocks, &mut done).await;
        self.upload_import(queued, done).await;
    }

    pub async fn queue_import(
        &self,
        blocks: &[(Saved, BlockParent)],
        mut done: impl FnMut(Uuid, Imported),
    ) -> Queued {
        let mut scopes: HashMap<Uuid, Uuid> = HashMap::new();
        let mut prepared = Vec::with_capacity(blocks.len());
        for (block, parent) in blocks {
            match self.prepare(block, *parent, &mut scopes).await {
                Ok(ready) => prepared.push(ready),
                Err(error) => done(block.id, Err(error)),
            }
        }
        if let Err(error) = self.commits.vault().store().sync() {
            for ready in &prepared {
                done(ready.id, Err(ClientError::Disconnected(error.to_string())));
            }
            return Queued {
                prepared: Vec::new(),
            };
        }
        let mut kept = Vec::with_capacity(prepared.len());
        for mut ready in prepared {
            match self.keep(&mut ready) {
                Ok(()) => kept.push(ready),
                Err(error) => done(ready.id, Err(error)),
            }
        }
        Queued { prepared: kept }
    }

    fn keep(&self, ready: &mut Prepared) -> Result<(), ClientError> {
        self.record_creation(&ready.creation)?;
        let mut ops = vec![Op::Create(ready.creation.clone())];
        if let Some(pending) = &ready.publish {
            ops.push(Op::Publish(pending.clone()));
        }
        ready.queued = self.refs.push_all(ops)?;
        if let Some(pending) = &ready.publish {
            self.refs.set_head(ready.id, pending.commit)?;
        }
        Ok(())
    }

    pub async fn upload_import(&self, queued: Queued, mut done: impl FnMut(Uuid, Imported)) {
        let prepared = queued.prepared;
        if !self.is_attached() {
            for ready in prepared {
                done(
                    ready.id,
                    Err(ClientError::Disconnected(
                        "kept on this device until it connects".into(),
                    )),
                );
            }
            return;
        }
        let mut pending: std::collections::VecDeque<(Vec<Uuid>, Sent<'_>)> =
            std::collections::VecDeque::new();
        let mut finish =
            |ids: Vec<Uuid>, sent: Result<Vec<(Uuid, Imported)>, ClientError>| match sent {
                Err(_) if ids.is_empty() => {}
                Ok(results) => {
                    for (id, result) in results {
                        done(id, result);
                    }
                }
                Err(error) => {
                    for id in ids {
                        done(id, Err(ClientError::Disconnected(error.to_string())));
                    }
                }
            };
        let mut groups = prepared.into_iter().peekable();
        while groups.peek().is_some() {
            let group: Vec<Prepared> = groups.by_ref().take(BATCH_BLOCKS).collect();
            let ids: Vec<Uuid> = group.iter().map(|ready| ready.id).collect();
            let objects: Vec<Hash> = group
                .iter()
                .flat_map(|ready| ready.objects.iter().copied())
                .collect();
            let groups = match self.object_groups(&objects) {
                Ok(groups) => groups,
                Err(error) => {
                    finish(ids, Err(error));
                    continue;
                }
            };
            let mut failed = None;
            for objects in groups {
                while pending.len() >= IN_FLIGHT {
                    let (ids, sent) = pending.pop_front().expect("more than none are pending");
                    finish(ids, sent.await);
                }
                match self.send_objects(objects) {
                    Ok(sent) => pending.push_back((Vec::new(), sent)),
                    Err(error) => {
                        failed = Some(error);
                        break;
                    }
                }
            }
            if let Some(error) = failed {
                finish(ids, Err(error));
                continue;
            }
            while pending.len() >= IN_FLIGHT {
                let (ids, sent) = pending.pop_front().expect("more than none are pending");
                finish(ids, sent.await);
            }
            pending.push_back((ids, self.send_batch(group)));
        }
        while let Some((ids, sent)) = pending.pop_front() {
            finish(ids, sent.await);
        }
    }

    async fn prepare(
        &self,
        block: &Saved,
        parent: BlockParent,
        scopes: &mut HashMap<Uuid, Uuid>,
    ) -> Result<Prepared, ClientError> {
        let under = match parent {
            BlockParent::Block(parent) => match scopes.get(&parent) {
                Some(scope) => Some(*scope),
                None => {
                    let scope = self.scope_key_under(parent).await?;
                    scopes.insert(parent, scope);
                    Some(scope)
                }
            },
            BlockParent::Root | BlockParent::Detached => None,
        };
        let (key, keys, children) = match under {
            Some(under) => {
                let (key, keys) = self.keys.mint_data(block.id, under)?;
                (key, keys, under)
            }
            None => {
                let (scope, mut keys) = self.keys.mint_scope(block.id, None)?;
                let (key, more) = self.keys.mint_data(block.id, scope)?;
                keys.extend(more);
                (key, keys, scope)
            }
        };
        scopes.insert(block.id, children);
        let vault = self.commits.vault();
        let creation = Creation {
            block: block.id,
            content_type: block.block_type,
            parent,
            metadata: vault.seal(key, &block.metadata.encode())?,
            references: block.metadata.references(),
            key,
            keys: keys.clone(),
        };
        let Some(content) = &block.content else {
            return Ok(Prepared {
                id: block.id,
                keys,
                creation,
                publish: None,
                objects: Vec::new(),
                queued: Vec::new(),
            });
        };
        let manifest = vault.write(key, block.block_type, content)?;
        let mut references = block.references.clone();
        references.extend(block.metadata.references());
        let references = deduplicated(references);
        let encoded = block.metadata.encode();
        let commit = Commit::new(manifest.clone(), self.account, now_milliseconds())
            .with_parents(Vec::new())
            .with_kind(CommitKind::Autosave)
            .with_references(references.clone())
            .with_metadata(encoded.clone());
        let id = self.commits.put(key, &commit)?;
        let chunks = manifest.chunk_hashes();
        let mut objects = vec![id.hash()];
        objects.extend(chunks.iter().copied());
        Ok(Prepared {
            id: block.id,
            keys,
            creation,
            publish: Some(Pending {
                block: block.id,
                commit: id,
                expected: None,
                parents: Vec::new(),
                chunks,
                time: commit.time,
                pinned: false,
                references,
                metadata: vault.seal(key, &encoded)?,
            }),
            objects,
            queued: Vec::new(),
        })
    }

    pub(super) fn object_groups(&self, hashes: &[Hash]) -> Result<Vec<Vec<Hash>>, ClientError> {
        let store = self.commits.vault().store();
        let mut groups: Vec<Vec<Hash>> = Vec::new();
        let mut bytes = OBJECT_BATCH_BYTES;
        for hash in hashes {
            let length = store.len(*hash)?.ok_or(ClientError::MissingObject(*hash))?;
            if bytes + length > OBJECT_BATCH_BYTES {
                groups.push(Vec::new());
                bytes = 0;
            }
            bytes += length;
            groups.last_mut().expect("a group was started").push(*hash);
        }
        Ok(groups)
    }

    pub(super) fn send_objects(&self, group: Vec<Hash>) -> Result<Sent<'_>, ClientError> {
        let store = self.commits.vault().store();
        let objects = group
            .iter()
            .map(|hash| {
                store
                    .get(*hash)?
                    .map(ByteBuf::from)
                    .ok_or(ClientError::MissingObject(*hash))
            })
            .collect::<Result<Vec<ByteBuf>, ClientError>>()?;
        let sent = self
            .connection()
            .send(|request| ClientMessage::PutObjects { request, objects })?;
        Ok(Box::pin(async move {
            match sent.await? {
                ServerMessage::Stored { hashes, .. } if hashes == group => Ok(Vec::new()),
                _ => Err(ClientError::Unexpected),
            }
        }))
    }

    fn send_batch(&self, group: Vec<Prepared>) -> Sent<'_> {
        let mut sent = Vec::with_capacity(group.len());
        for ready in group {
            let Creation {
                block,
                content_type,
                parent,
                metadata,
                references,
                key,
                keys,
            } = ready.creation.clone();
            let created = self
                .connection()
                .send(|request| ClientMessage::CreateBlock {
                    request,
                    block,
                    content_type,
                    parent,
                    metadata,
                    references,
                    key,
                    keys,
                });
            let published = ready.publish.clone().map(|pending| {
                self.connection().send(|request| ClientMessage::Publish {
                    request,
                    block: pending.block,
                    commit: pending.commit,
                    expected: None,
                    parents: Vec::new(),
                    chunks: pending.chunks,
                    time: pending.time,
                    pinned: false,
                    references: pending.references,
                    metadata: pending.metadata,
                    objects: Vec::new(),
                })
            });
            sent.push((ready, created, published));
        }
        Box::pin(async move {
            let mut results = Vec::with_capacity(sent.len());
            for (ready, created, published) in sent {
                let created = answered(created).await?;
                let published = match published {
                    Some(published) => Some(answered(published).await?),
                    None => None,
                };
                results.push((ready.id, self.imported(&ready, created, published)));
            }
            Ok(results)
        })
    }

    fn imported(
        &self,
        ready: &Prepared,
        created: ServerMessage,
        published: Option<ServerMessage>,
    ) -> Imported {
        let block = ready.id;
        let summary = match created {
            ServerMessage::Block { block, .. } => Some(block),
            ServerMessage::Failed {
                code: be_protocol::ErrorCode::BlockAlreadyExists,
                ..
            } => None,
            ServerMessage::Failed { code, message, .. } => {
                self.refs.remove(&ready.queued)?;
                return Err(ClientError::Refused(code, message));
            }
            _ => return Err(ClientError::Unexpected),
        };
        self.recorded(&ready.keys);
        if let Some(summary) = &summary {
            self.note(summary);
        }
        let commit = ready.publish.as_ref().map(|pending| pending.commit);
        let head = match published {
            None => None,
            Some(ServerMessage::Published { head, .. }) => Some(head),
            Some(ServerMessage::Rejected { head, .. }) if head.is_some() && head == commit => head,
            Some(ServerMessage::Rejected { .. }) => {
                self.refs.remove(&ready.queued)?;
                return Err(ClientError::Contended);
            }
            Some(ServerMessage::Failed { code, message, .. }) => {
                self.refs.remove(&ready.queued)?;
                return Err(ClientError::Refused(code, message));
            }
            Some(_) => return Err(ClientError::Unexpected),
        };
        self.refs.remove(&ready.queued)?;
        if let Some(head) = head {
            self.refs.set_remote(block, head)?;
            self.remember(block, head)?;
        }
        match summary {
            Some(summary) => Ok(summary),
            None => self.refs.summary(block).ok_or(ClientError::Unexpected),
        }
    }
}

async fn answered(
    sent: Result<crate::connection::Answer, ClientError>,
) -> Result<ServerMessage, ClientError> {
    match sent?.await {
        Err(ClientError::Refused(code, message)) => Ok(ServerMessage::Failed {
            request: 0,
            code,
            message,
        }),
        answer => answer,
    }
}
