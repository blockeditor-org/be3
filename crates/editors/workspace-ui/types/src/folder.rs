use be_model::{Anchor, BlockRef, Change, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

use be_block::{ChildChange, Root};

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "folder")]
pub struct Folder {
    pub entries: List<FolderEntry>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "folder.entry")]
pub struct FolderEntry {
    pub block: BlockRef,
}

impl Folder {
    pub fn blocks(&self) -> Vec<Uuid> {
        let mut seen = std::collections::HashSet::new();
        self.entries
            .iter()
            .map(|entry| entry.block.0)
            .filter(|block| seen.insert(*block))
            .collect()
    }

    pub fn add(&self, block: Uuid) -> Edit {
        if self.entries.iter().any(|entry| entry.block.0 == block) {
            return Edit::default();
        }
        Self::ENTRIES
            .insert(
                ObjectId::ROOT,
                Anchor::End,
                &FolderEntry {
                    block: BlockRef(block),
                },
            )
            .1
            .into()
    }

    pub fn remove(&self, block: Uuid) -> Edit {
        self.entries
            .iter()
            .filter(|entry| entry.block.0 == block)
            .map(|entry| Change::remove(entry.id))
            .collect()
    }
}

impl Root for Folder {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0xd4e3_67ab_db73_4419_bffc_b913_1288_c2e7);

    const FROZEN: bool = true;

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        Some(match change {
            ChildChange::Add(block) => self.add(block),
            ChildChange::Delete(block) => self.remove(block),
            ChildChange::Replace { old, new } => {
                if self.entries.iter().any(|entry| entry.block.0 == new) {
                    self.remove(old)
                } else {
                    self.entries
                        .iter()
                        .filter(|entry| entry.block.0 == old)
                        .map(|entry| FolderEntry::BLOCK.set(entry.id, &BlockRef(new)))
                        .collect()
                }
            }
        })
    }
}

pub type FolderContent = Document<Folder>;
