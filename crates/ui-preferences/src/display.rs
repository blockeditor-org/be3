pub use crate::types::{DisplayMode, Rotation};

impl Rotation {
    pub const ALL: [Self; 4] = [
        Self::Normal,
        Self::Clockwise90,
        Self::Clockwise180,
        Self::Clockwise270,
    ];

    pub fn degrees(self) -> u32 {
        match self {
            Self::Normal => 0,
            Self::Clockwise90 => 90,
            Self::Clockwise180 => 180,
            Self::Clockwise270 => 270,
        }
    }

    pub fn sideways(self) -> bool {
        matches!(self, Self::Clockwise90 | Self::Clockwise270)
    }
}
