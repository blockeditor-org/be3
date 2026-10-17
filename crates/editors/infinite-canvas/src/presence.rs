use block_editor_beui::be_block::canvas::CanvasPoint;
use block_editor_beui::be_block::presence::{PresenceColor, PresenceKind};
use serde::{Deserialize, Serialize};
use text_editor_core::Position;
use uuid::Uuid;

const CANVAS_CURSOR: Uuid = Uuid::from_u128(0x6361_6e76_6173_5f63_7572_736f_725f_5f5f);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasCursor {
    pub pointer: Option<CanvasPoint>,
    pub selection: Vec<Uuid>,
    pub text: Option<TextCaret>,
    pub color: PresenceColor,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct TextCaret {
    pub entity: Uuid,
    pub anchor: Position,
    pub focus: Position,
}

impl PresenceKind for CanvasCursor {
    const ID: Uuid = CANVAS_CURSOR;
}
