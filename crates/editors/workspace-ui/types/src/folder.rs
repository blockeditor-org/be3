use be_model::{Anchor, BlockRef, Change, Document, Edit, FieldRef, List, Model, ObjectId};
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
        blocks(&self.entries)
    }

    pub fn add(&self, block: Uuid) -> Edit {
        add(&self.entries, Self::ENTRIES, block)
    }

    pub fn remove(&self, block: Uuid) -> Edit {
        remove(&self.entries, block)
    }

    pub fn reorder(&self, block: Uuid, after: Option<Uuid>) -> Edit {
        reorder(&self.entries, Self::ENTRIES, block, after)
    }
}

impl Root for Folder {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0xd4e3_67ab_db73_4419_bffc_b913_1288_c2e7);

    const FROZEN: bool = true;

    fn references(&self) -> Vec<Uuid> {
        self.blocks()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        Some(child_edit(&self.entries, Self::ENTRIES, change))
    }
}

pub type FolderContent = Document<Folder>;

pub(crate) fn blocks(entries: &List<FolderEntry>) -> Vec<Uuid> {
    let mut seen = std::collections::HashSet::new();
    entries
        .iter()
        .map(|entry| entry.block.0)
        .filter(|block| seen.insert(*block))
        .collect()
}

pub(crate) fn add<M>(
    entries: &List<FolderEntry>,
    field: FieldRef<M, List<FolderEntry>>,
    block: Uuid,
) -> Edit {
    if entries.iter().any(|entry| entry.block.0 == block) {
        return Edit::default();
    }
    field
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

pub(crate) fn remove(entries: &List<FolderEntry>, block: Uuid) -> Edit {
    entries
        .iter()
        .filter(|entry| entry.block.0 == block)
        .map(|entry| Change::remove(entry.id))
        .collect()
}

pub(crate) fn reorder<M>(
    entries: &List<FolderEntry>,
    field: FieldRef<M, List<FolderEntry>>,
    block: Uuid,
    after: Option<Uuid>,
) -> Edit {
    let entry_of = |block: Uuid| entries.iter().find(|entry| entry.block.0 == block);
    let Some(moved) = entry_of(block) else {
        return Edit::default();
    };
    let anchor = match after {
        None => Anchor::Start,
        Some(after) if after == block => return Edit::default(),
        Some(after) => match entry_of(after) {
            Some(entry) => Anchor::After(entry.id),
            None => return Edit::default(),
        },
    };
    let position = entries.iter().position(|entry| entry.id == moved.id);
    let already = match anchor {
        Anchor::After(id) => position
            .and_then(|at| at.checked_sub(1))
            .is_some_and(|before| entries[before].id == id),
        _ => position == Some(0),
    };
    if already {
        return Edit::default();
    }
    field.move_into(ObjectId::ROOT, anchor, moved.id).into()
}

pub(crate) fn child_edit<M>(
    entries: &List<FolderEntry>,
    field: FieldRef<M, List<FolderEntry>>,
    change: ChildChange,
) -> Edit {
    match change {
        ChildChange::Add(block) => add(entries, field, block),
        ChildChange::Delete(block) => remove(entries, block),
        ChildChange::Replace { old, new } => {
            if entries.iter().any(|entry| entry.block.0 == new) {
                remove(entries, old)
            } else {
                entries
                    .iter()
                    .filter(|entry| entry.block.0 == old)
                    .map(|entry| FolderEntry::BLOCK.set(entry.id, &BlockRef(new)))
                    .collect()
            }
        }
        ChildChange::Reorder { block, after } => reorder(entries, field, block, after),
    }
}
