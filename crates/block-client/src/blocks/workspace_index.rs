use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlockEntry {
    pub id: Uuid,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceIndex;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WorkspaceIndexOperation {}

impl Block for WorkspaceIndex {
    type Operation = WorkspaceIndexOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x626c_6f63_6b2d_6170_702d_696e_6465_7802);

    fn apply_operation(_block: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}
