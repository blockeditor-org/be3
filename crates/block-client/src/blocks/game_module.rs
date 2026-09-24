use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct GameModule {}

impl GameModule {
    pub const FILE_EXTENSIONS: &'static [&'static str] = &["wasm"];
    pub const MIME_TYPES: &'static [&'static str] = &["application/wasm"];

    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum GameModuleOperation {}

impl Block for GameModule {
    type Operation = GameModuleOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6761_6d65_2d6d_6f64_756c_652d_626c_0001);

    fn apply_operation(_game_module: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
