use block_editor_beui::SharedCaret;
use block_editor_beui::be_block::presence::{PresenceColor, PresenceKind};
use infinite_canvas_types::CanvasPoint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const CANVAS_CURSOR: Uuid = Uuid::from_u128(0x6361_6e76_6173_5f63_7572_736f_725f_5f5f);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasCursor {
    pub pointer: Option<CanvasPoint>,
    pub selection: Vec<Uuid>,
    pub caret: Option<CanvasCaret>,
    pub drawing: Option<Uuid>,
    pub color: PresenceColor,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CanvasCaret {
    pub entity: Uuid,
    pub caret: SharedCaret,
}

impl PresenceKind for CanvasCursor {
    const ID: Uuid = CANVAS_CURSOR;
}
