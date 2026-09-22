use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct UiSettings {}

impl UiSettings {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum UiSettingsOperation {}

impl Block for UiSettings {
    type Operation = UiSettingsOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7569_2d73_6574_7469_6e67_732d_626c_6b31);

    fn apply_operation(_settings: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
