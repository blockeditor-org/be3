use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Settings;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SettingsOperation {}

impl Block for Settings {
    type Operation = SettingsOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7365_7474_696e_6773_2d62_6c6f_636b_3031);

    fn apply_operation(_block: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}
