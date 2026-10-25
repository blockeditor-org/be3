use std::{collections::HashMap, future::Future, pin::Pin};

use be_commit::{Commit, CommitKind, now_milliseconds};
use be_graph::BlockParent;
use be_protocol::{BlockSummary, ByteBuf, ClientMessage, KeySet, ServerMessage};
use be_store::{Hash, ObjectStore};
use uuid::Uuid;

use super::{Peer, deduplicated};
use crate::{ClientError, transfer::Saved};

const BATCH_BLOCKS: usize = 64;
const OBJECT_BATCH_BYTES: usize = 2 * 1024 * 1024;
pub(super) const IN_FLIGHT: usize = 8;

pub type Imported = Result<BlockSummary, ClientError>;

type Message = Box<dyn FnOnce(u64) -> ClientMessage + Send>;

struct Prepared {
    id: Uuid,
    keys: KeySet,
    create: Message,
    publish: Option<Message>,
    objects: Vec<Hash>,
}

type Sent<'a> = Pin<Box<dyn Future<Output = Result<Vec<(Uuid, Imported)>, ClientError>> + 'a>>;

impl<S: ObjectStore> Peer<S> {
    pub async fn import_blocks(
        &self,
        blocks: &[(Saved, BlockParent)],
        mut done: impl FnMut(Uuid, Imported),
    ) {
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
        let create: Message = Box::new({
            let metadata = vault.seal(key, &block.metadata.encode())?;
            let references = block.metadata.references();
            let (block, content_type) = (block.id, block.block_type);
            let keys = keys.clone();
            move |request| ClientMessage::CreateBlock {
                request,
                block,
                content_type,
                parent,
                metadata,
                references,
                key,
                keys,
            }
        });
        let Some(content) = &block.content else {
            return Ok(Prepared {
                id: block.id,
                keys,
                create,
                publish: None,
                objects: Vec::new(),
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
            create,
            publish: Some(Box::new({
                let (block, time) = (block.id, commit.time);
                let metadata = vault.seal(key, &encoded)?;
                move |request| ClientMessage::Publish {
                    request,
                    block,
                    commit: id,
                    expected: None,
                    parents: Vec::new(),
                    chunks,
                    time,
                    pinned: false,
                    references,
                    metadata,
                    objects: Vec::new(),
                }
            })),
            objects,
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
            let created = self.connection().send(ready.create);
            let published = ready.publish.map(|publish| self.connection().send(publish));
            sent.push((ready.id, ready.keys, created, published));
        }
        Box::pin(async move {
            let mut results = Vec::with_capacity(sent.len());
            for (id, keys, created, published) in sent {
                let created = answered(created).await?;
                let published = match published {
                    Some(published) => Some(answered(published).await?),
                    None => None,
                };
                results.push((id, self.imported(id, &keys, Some(created), published)));
            }
            Ok(results)
        })
    }

    fn imported(
        &self,
        block: Uuid,
        keys: &KeySet,
        created: Option<ServerMessage>,
        published: Option<ServerMessage>,
    ) -> Imported {
        let summary = match created {
            Some(ServerMessage::Block { block, .. }) => block,
            Some(ServerMessage::Failed { code, message, .. }) => {
                return Err(ClientError::Refused(code, message));
            }
            _ => return Err(ClientError::Unexpected),
        };
        self.recorded(keys);
        self.note(&summary);
        match published {
            None | Some(ServerMessage::Published { .. }) => {}
            Some(ServerMessage::Rejected { .. }) => return Err(ClientError::Contended),
            Some(ServerMessage::Failed { code, message, .. }) => {
                return Err(ClientError::Refused(code, message));
            }
            Some(_) => return Err(ClientError::Unexpected),
        }
        if let Some(ServerMessage::Published { head, .. }) = published {
            self.remember(block, head)?;
        }
        Ok(summary)
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
