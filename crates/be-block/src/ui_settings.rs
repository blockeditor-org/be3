use be_commit::MergeResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BlockContent, ContentError, LiveEdit, Merge};

pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiSettingsContent {
    zoom: f32,
}

impl Default for UiSettingsContent {
    fn default() -> Self {
        Self { zoom: 1.0 }
    }
}

impl UiSettingsContent {
    pub fn zoom(&self) -> f32 {
        self.zoom
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum UiSettingsOp {
    SetZoom { zoom: f32 },
}

impl BlockContent for UiSettingsContent {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7569_2d73_6574_7469_6e67_732d_626c_6b02);

    fn encode(&self) -> Vec<u8> {
        self.zoom.to_le_bytes().to_vec()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        let bytes: [u8; 4] = bytes
            .try_into()
            .map_err(|_| ContentError::Malformed("ui settings are four bytes"))?;
        let zoom = f32::from_le_bytes(bytes);
        if !zoom.is_finite() {
            return Err(ContentError::Malformed("a zoom is a finite number"));
        }
        Ok(Self {
            zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
        })
    }
}

impl LiveEdit for UiSettingsContent {
    type Op = UiSettingsOp;

    fn apply(&mut self, operation: &Self::Op) {
        match operation {
            UiSettingsOp::SetZoom { zoom } if zoom.is_finite() => {
                self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
            }
            UiSettingsOp::SetZoom { .. } => {}
        }
    }
}

impl Merge for UiSettingsContent {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        if ours == base {
            MergeResult::Clean(*theirs)
        } else if theirs == base || theirs == ours {
            MergeResult::Clean(*ours)
        } else {
            MergeResult::Conflicted {
                value: *ours,
                conflicts: 1,
            }
        }
    }
}
