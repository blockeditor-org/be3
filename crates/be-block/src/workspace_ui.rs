use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

use crate::Root;

pub const MAX_RECENT: usize = 20;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct WorkspaceUi {
    pub recent: List<RecentBlock>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct RecentBlock {
    pub block: Uuid,
    pub block_type: Uuid,
}

impl WorkspaceUi {
    pub fn recent_blocks(&self) -> Vec<(Uuid, Uuid)> {
        self.recent
            .iter()
            .map(|entry| (entry.block, entry.block_type))
            .collect()
    }

    pub fn visit(&self, block: Uuid, block_type: Uuid) -> Option<Edit> {
        let first = self.recent.iter().next();
        if first.is_some_and(|entry| entry.block == block && entry.block_type == block_type) {
            return None;
        }
        let entry = RecentBlock { block, block_type };
        let (_, insert) = Self::RECENT.insert(ObjectId::ROOT, Anchor::Start, &entry);
        let kept = self
            .recent
            .iter()
            .filter(|entry| entry.block != block)
            .count()
            .min(MAX_RECENT - 1);
        let removed = self
            .recent
            .iter()
            .filter(|entry| entry.block == block)
            .chain(
                self.recent
                    .iter()
                    .filter(|entry| entry.block != block)
                    .skip(kept),
            )
            .map(|entry| Change::remove(entry.id));
        Some(std::iter::once(insert).chain(removed).collect())
    }

    pub fn forget(&self, block: Uuid) -> Edit {
        self.recent
            .iter()
            .filter(|entry| entry.block == block)
            .map(|entry| Change::remove(entry.id))
            .collect()
    }
}

impl Root for WorkspaceUi {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x776f_726b_7370_6163_652d_7569_2d30_3031);
}

pub type WorkspaceUiContent = Document<WorkspaceUi>;
