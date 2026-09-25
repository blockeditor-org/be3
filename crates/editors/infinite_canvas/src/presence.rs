use block_editor_plugin::be_block::canvas::CanvasPoint;
use block_editor_plugin::be_block::presence::{PresenceColor, PresenceKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const CANVAS_CURSOR: Uuid = Uuid::from_u128(0x6361_6e76_6173_5f63_7572_736f_725f_5f5f);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasCursor {
    pub pointer: Option<CanvasPoint>,
    pub selection: Vec<Uuid>,
    pub color: PresenceColor,
}

impl PresenceKind for CanvasCursor {
    const ID: Uuid = CANVAS_CURSOR;
}
