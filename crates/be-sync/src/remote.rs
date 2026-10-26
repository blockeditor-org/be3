use std::collections::BTreeMap;

use be_block::{EXECUTABLE, Entries};
use be_client::ClientError;
use be_client::Peer;
use be_commit::CommitId;
use be_graph::BlockParent;
use be_protocol::BlockSummary;
use be_store::ObjectStore;
use be_types_host::{Plain, plain_of};
use uuid::Uuid;

use crate::explain;

const READ_GROUP: usize = 256;

#[derive(Clone, Debug)]
pub struct Block {
    pub block_type: Uuid,
    pub parent: Option<Uuid>,
    pub name: String,
    pub executable: bool,
    pub head: Option<CommitId>,
    pub defined: Entries,
    pub can_edit: bool,
}

pub async fn list<S: ObjectStore>(
    peer: &Peer<S>,
    root: Uuid,
) -> Result<BTreeMap<Uuid, Block>, String> {
    let top = peer.summary(root).await.map_err(explain)?;
    if !top.access.can_view() {
        return Err(format!("this account cannot see {root}"));
    }
    if plain_of(top.content_type) != Plain::Directory {
        return Err("be sync keeps a folder or a workspace in step with a directory".to_owned());
    }
    let mut blocks = BTreeMap::new();
    let mut level = vec![top];
    let mut first = true;
    while !level.is_empty() {
        peer.learn(&level).await.map_err(explain)?;
        let mut directories = Vec::new();
        for summary in level {
            if !summary.access.can_view() || (!first && !summary.held) {
                continue;
            }
            if plain_of(summary.content_type) == Plain::Directory {
                directories.push(summary.id);
            }
            let parent = match first {
                true => None,
                false => summary.parent.and_then(BlockParent::block),
            };
            blocks.insert(summary.id, block(peer, &summary, parent));
        }
        first = false;
        let below = futures_util::future::try_join_all(
            directories
                .iter()
                .map(|parent| peer.children(BlockParent::Block(*parent))),
        )
        .await
        .map_err(explain)?;
        level = below.into_iter().flatten().collect();
    }
    Ok(blocks)
}

fn block<S: ObjectStore>(peer: &Peer<S>, summary: &BlockSummary, parent: Option<Uuid>) -> Block {
    let metadata = peer.metadata(summary).unwrap_or_default();
    Block {
        block_type: summary.content_type,
        parent,
        name: metadata.name().unwrap_or_default(),
        executable: metadata.get(&EXECUTABLE).unwrap_or(false),
        head: summary.head,
        defined: metadata.defined,
        can_edit: summary.access.can_edit(),
    }
}

pub async fn read<S: ObjectStore>(
    peer: &Peer<S>,
    wanted: &[(Uuid, Option<CommitId>)],
) -> Result<Vec<(Uuid, Option<CommitId>, Option<Vec<u8>>)>, String> {
    let mut read = Vec::with_capacity(wanted.len());
    for group in wanted.chunks(READ_GROUP) {
        let contents = peer.read_contents(group).await.map_err(explain)?;
        for ((id, _), (summary, content)) in group.iter().zip(contents) {
            read.push((*id, summary.head, content));
        }
    }
    Ok(read)
}

pub async fn at_commit<S: ObjectStore>(
    peer: &Peer<S>,
    commit: CommitId,
) -> Result<Vec<u8>, ClientError> {
    let commit = peer.load_commit(commit).await?;
    peer.read_manifest(&commit.manifest).await
}
