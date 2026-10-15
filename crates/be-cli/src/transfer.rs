use std::collections::VecDeque;

use be_block::BlockContent;
use be_block::database::{Database, DatabaseContent};
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
    pub renamed_prefix: Option<(String, String)>,
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
        let mut blocks = Vec::new();
        for (entry, parent) in order {
            blocks.push(
                peer.download_block(entry.id, entry.block_type, parent)
                    .await
                    .map_err(explain)?,
            );
        }
        Ok((top, blocks))
    }

    pub async fn upload(
        &self,
        blocks: Vec<Saved>,
        parent: BlockParent,
        prefix: Option<&str>,
    ) -> Result<Uploaded, String> {
        let mut blocks = renumber(blocks);
        let mut taken = Vec::new();
        for (position, block) in blocks.iter().enumerate() {
            if block.block_type == DatabaseContent::CONTENT_TYPE
                && let Some(bytes) = &block.content
            {
                let database = DatabaseContent::decode(bytes)
                    .map_err(|error| format!("a database in the file is unreadable: {error}"))?;
                let held = database.root().prefix.trim().to_owned();
                if !held.is_empty() && self.database_with_prefix(&held).await?.is_some() {
                    taken.push((position, held, database));
                }
            }
        }
        let renamed_prefix = match (taken.as_mut_slice(), prefix) {
            ([], _) => None,
            ([(position, held, database)], Some(wanted)) => {
                let wanted = wanted.trim().to_ascii_uppercase();
                if !wanted.is_empty() && self.database_with_prefix(&wanted).await?.is_some() {
                    return Err(format!("another database already uses the prefix {wanted}"));
                }
                database.apply(&Database::set_prefix(wanted.clone()));
                let block = &mut blocks[*position];
                block.metadata.insert(&be_block::ROW_PREFIX, &wanted);
                block.content = Some(database.encode());
                Some((held.clone(), wanted))
            }
            ([(_, held, _)], None) => {
                return Err(format!(
                    "another database already uses the row id prefix {held}; give the copy its own with --prefix"
                ));
            }
            (several, _) => {
                return Err(format!(
                    "other databases already use the row id prefixes {}",
                    several
                        .iter()
                        .map(|(_, held, _)| held.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        };
        let peer = self.peer();
        for block in &blocks {
            let placed = match block.parent {
                Some(position) => BlockParent::Block(blocks[position].id),
                None => parent,
            };
            peer.create_block(block.id, block.block_type, placed, &block.metadata)
                .await
                .map_err(explain)?;
            if let Some(saved) = peer.publish_content(block).await.map_err(explain)?
                && saved.published().is_none()
            {
                return Err(format!(
                    "the server would not take the content of {}",
                    block.id
                ));
            }
        }
        self.invalidate().await;
        Ok(Uploaded {
            top: blocks[0].id,
            blocks: blocks.len(),
            renamed_prefix,
        })
    }
}
