use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceUi {}

impl WorkspaceUi {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum WorkspaceUiOperation {}

impl Block for WorkspaceUi {
    type Operation = WorkspaceUiOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x776f_726b_7370_6163_652d_7569_2d30_3031);

    fn apply_operation(_ui: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
