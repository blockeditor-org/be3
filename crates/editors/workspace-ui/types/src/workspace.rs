use be_model::{Document, Edit, List, Model};
use uuid::Uuid;

use crate::folder::{self, FolderEntry};
use be_block::{ChildChange, Root};

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "workspace")]
pub struct Workspace {
    pub entries: List<FolderEntry>,
}

impl Workspace {
    pub fn blocks(&self) -> Vec<Uuid> {
        folder::blocks(&self.entries)
    }

    pub fn add(&self, block: Uuid) -> Edit {
        folder::add(&self.entries, Self::ENTRIES, block)
    }

    pub fn remove(&self, block: Uuid) -> Edit {
        folder::remove(&self.entries, block)
    }

    pub fn reorder(&self, block: Uuid, after: Option<Uuid>) -> Edit {
        folder::reorder(&self.entries, Self::ENTRIES, block, after)
    }
}

impl Root for Workspace {
    const CONTENT_TYPE: Uuid = be_block::WORKSPACE_TYPE;
    const FROZEN: bool = true;

    fn references(&self) -> Vec<Uuid> {
        self.blocks()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        Some(folder::child_edit(&self.entries, Self::ENTRIES, change))
    }
}

pub type WorkspaceContent = Document<Workspace>;
