use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PaintSnapshot {}

impl PaintSnapshot {
    pub const FILE_EXTENSION: &'static str = "paint";

    pub fn fingerprint(data: &[u8]) -> String {
        Sha256::digest(data)
            .iter()
            .fold(String::new(), |mut hash, byte| {
                use std::fmt::Write as _;
                let _ = write!(hash, "{byte:02x}");
                hash
            })
    }

    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PaintSnapshotOperation {}

impl Block for PaintSnapshot {
    type Operation = PaintSnapshotOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7061_696e_742d_736e_6170_7368_6f74_0001);

    fn apply_operation(_paint_snapshot: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
