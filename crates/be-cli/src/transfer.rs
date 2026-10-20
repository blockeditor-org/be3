use std::collections::VecDeque;

use be_client::transfer::{Saved, renumber};
use be_graph::BlockParent;
use be_store::ObjectStore;
use uuid::Uuid;

use crate::blocks::Entry;
use crate::session::explain;
use crate::tracker::Tracker;

pub struct Uploaded {
    pub top: Uuid,
    pub blocks: usize,
}

impl<S: ObjectStore> Tracker<S> {
    pub async fn download(&self, entry: &Entry) -> Result<(Entry, Vec<Saved>), String> {
        let top = self.outermost(entry).await?;
        let index = self.index().await?;
        let mut order = vec![(top.clone(), None)];
        let mut queue = VecDeque::from([(top.id, 0)]);
        while let Some((parent, position)) = queue.pop_front() {
            for child in index.children(Some(parent)) {
                queue.push_back((child.id, order.len()));
                order.push((child.clone(), Some(position)));
            }
        }
        let peer = self.peer();
        let wanted: Vec<_> = order
            .iter()
            .map(|(entry, _)| (entry.id, entry.head))
            .collect();
        let read = peer.read_contents(&wanted).await.map_err(explain)?;
        let mut blocks = Vec::with_capacity(read.len());
        for ((entry, parent), (summary, content)) in order.into_iter().zip(read) {
            blocks.push(Saved {
                id: entry.id,
                block_type: entry.block_type,
                parent,
                metadata: peer.metadata_of_summary(&summary).await.map_err(explain)?,
                references: summary.references,
                content,
            });
        }
        Ok((top, blocks))
    }

    pub async fn upload(
        &self,
        blocks: Vec<Saved>,
        parent: BlockParent,
    ) -> Result<Uploaded, String> {
        let blocks = renumber(blocks);
        let placed: Vec<_> = blocks
            .iter()
            .map(|block| {
                let parent = match block.parent {
                    Some(position) => BlockParent::Block(blocks[position].id),
                    None => parent,
                };
                (block.clone(), parent)
            })
            .collect();
        let mut failed = None;
        self.peer()
            .import_blocks(&placed, |block, imported| {
                if let Err(error) = imported
                    && failed.is_none()
                {
                    failed = Some(format!("{block} was not uploaded: {}", explain(error)));
                }
            })
            .await;
        if let Some(error) = failed {
            return Err(error);
        }
        self.invalidate().await;
        Ok(Uploaded {
            top: blocks[0].id,
            blocks: blocks.len(),
        })
    }
}
