use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

use crate::{ChildChange, Root};

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Folder {
    pub entries: List<FolderEntry>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct FolderEntry {
    pub block: Uuid,
}

impl Folder {
    pub fn blocks(&self) -> Vec<Uuid> {
        let mut seen = std::collections::HashSet::new();
        self.entries
            .iter()
            .map(|entry| entry.block)
            .filter(|block| seen.insert(*block))
            .collect()
    }

    pub fn add(&self, block: Uuid) -> Edit {
        if self.entries.iter().any(|entry| entry.block == block) {
            return Edit::default();
        }
        Self::ENTRIES
            .insert(ObjectId::ROOT, Anchor::End, &FolderEntry { block })
            .1
            .into()
    }

    pub fn remove(&self, block: Uuid) -> Edit {
        self.entries
            .iter()
            .filter(|entry| entry.block == block)
            .map(|entry| Change::remove(entry.id))
            .collect()
    }
}

impl Root for Folder {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x626c_6f63_6b2d_6170_702d_696e_6465_7802);

    fn references(&self) -> Vec<Uuid> {
        self.blocks()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        Some(match change {
            ChildChange::Add(block) => self.add(block),
            ChildChange::Delete(block) => self.remove(block),
            ChildChange::Replace { old, new } => {
                if self.entries.iter().any(|entry| entry.block == new) {
                    self.remove(old)
                } else {
                    self.entries
                        .iter()
                        .filter(|entry| entry.block == old)
                        .map(|entry| FolderEntry::BLOCK.set(entry.id, &new))
                        .collect()
                }
            }
        })
    }
}

pub type FolderContent = Document<Folder>;
