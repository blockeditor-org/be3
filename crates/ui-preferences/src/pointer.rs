pub use crate::types::{InputDevice, PointerSettings};
use serde::{Deserialize, Serialize};

pub const MIN_POINTER_SPEED: f32 = -1.0;
pub const MAX_POINTER_SPEED: f32 = 1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PointerSpeed(f32);

impl PointerSpeed {
    pub fn new(speed: f32) -> Self {
        match speed.is_finite() {
            true => Self(speed.clamp(MIN_POINTER_SPEED, MAX_POINTER_SPEED)),
            false => Self::default(),
        }
    }

    pub fn get(self) -> f32 {
        Self::new(self.0).0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerSetting {
    Speed(Option<PointerSpeed>),
    TapToClick(Option<bool>),
    NaturalScroll(Option<bool>),
}

impl PointerSetting {
    pub fn cleared(self) -> Self {
        match self {
            Self::Speed(_) => Self::Speed(None),
            Self::TapToClick(_) => Self::TapToClick(None),
            Self::NaturalScroll(_) => Self::NaturalScroll(None),
        }
    }
}

impl PointerSettings {
    pub fn with(self, setting: PointerSetting) -> Self {
        match setting {
            PointerSetting::Speed(speed) => Self {
                speed: speed.map(|speed| PointerSpeed::new(speed.get())),
                ..self
            },
            PointerSetting::TapToClick(tap_to_click) => Self {
                tap_to_click,
                ..self
            },
            PointerSetting::NaturalScroll(natural_scroll) => Self {
                natural_scroll,
                ..self
            },
        }
    }

    pub fn or(self, fallback: Self) -> Self {
        Self {
            speed: self.speed.or(fallback.speed),
            tap_to_click: self.tap_to_click.or(fallback.tap_to_click),
            natural_scroll: self.natural_scroll.or(fallback.natural_scroll),
        }
    }
}
