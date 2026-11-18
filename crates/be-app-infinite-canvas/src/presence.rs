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
    /// The move the peer carries, held until it is dropped: how others see it as it goes.
    pub carried: Option<CanvasCarry>,
    pub color: PresenceColor,
}

/// A carried move of the peer's selection (what of it is unlocked): moved `by`, and to whole
/// units when `snapped`, as the drop would move it.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasCarry {
    pub by: CanvasPoint,
    pub snapped: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CanvasCaret {
    pub entity: Uuid,
    pub caret: SharedCaret,
}

impl PresenceKind for CanvasCursor {
    const ID: Uuid = CANVAS_CURSOR;
}
