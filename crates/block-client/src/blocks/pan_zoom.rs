use block::{Block, NoHistory};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PanZoom {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PanZoomOperation {}

impl Block for PanZoom {
    type Operation = PanZoomOperation;
    type History = NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7061_6e5f_7a6f_6f6d_2d62_6c6f_636b_0001);

    fn apply_operation(_block: &mut Self, operation: &Self::Operation) {
        match *operation {}
    }
}

#[cfg(test)]
mod tests;
