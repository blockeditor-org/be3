use be_sdk_beui::be_block_content::presence::{PresenceColor, PresenceKind};
use be_sdk_text::SharedCaret;
use be_type_infinite_canvas::CanvasPoint;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const CANVAS_CURSOR: Uuid = Uuid::from_u128(0x6361_6e76_6173_5f63_7572_736f_725f_5f5f);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasCursor {
    pub pointer: Option<CanvasPoint>,
    pub selection: Vec<Uuid>,
    pub caret: Option<CanvasCaret>,
    pub drawing: Option<Uuid>,
    /// What the peer carries mid-move: each entity's centre where a drop now would put it.
    /// The move is held until it is dropped, so this is how others see it as it goes.
    pub carried: Vec<(Uuid, CanvasPoint)>,
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
