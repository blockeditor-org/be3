use be_block_client::transfer::{SavedBlock, renumber};
use be_block_graph::BlockParent;
use be_block_store::ObjectStore;
use uuid::Uuid;

use crate::blocks::Entry;
use crate::session::explain;
use crate::tracker::Tracker;

pub struct Uploaded {
    pub top: Uuid,
    pub blocks: usize,
}

impl<S: ObjectStore> Tracker<S> {
    /// `entry` and everything inside it, for a block file, as be-host exports it
    /// (`be_block_type_host::transfer::collect`), with the entry it starts from.
    pub async fn download(&self, entry: &Entry) -> Result<(Entry, Vec<SavedBlock>), String> {
        let blocks = be_block_type_host::transfer::collect(
            self.peer(),
            entry.id,
            be_host_stack::discovery::exported_whole,
            &mut *self.types().await,
        )
        .await
        .map_err(explain)?;
        Ok((self.entry(blocks[0].id).await?, blocks))
    }

    pub async fn upload(
        &self,
        blocks: Vec<SavedBlock>,
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
        self.place(parent, blocks[0].id).await?;
        self.invalidate().await;
        self.register_uploaded(&blocks).await?;
        Ok(Uploaded {
            top: blocks[0].id,
            blocks: blocks.len(),
        })
    }
}
