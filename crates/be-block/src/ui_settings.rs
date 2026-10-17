use be_model::{Document, Edit, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;
use crate::settings::Layered;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Theme {
    #[default]
    Dark,
    OledDark,
    Light,
    EInk,
}

impl Theme {
    pub const ALL: [Self; 4] = [Self::Light, Self::Dark, Self::OledDark, Self::EInk];

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::OledDark => "OLED dark",
            Self::Light => "Light",
            Self::EInk => "E-ink",
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct UiSettings {
    pub theme: Option<Theme>,
    pub scale_percent: Option<u32>,
}

impl UiSettings {
    pub fn theme(&self) -> Theme {
        self.theme.unwrap_or_default()
    }

    pub fn set_theme(theme: Option<Theme>) -> Edit {
        Self::THEME.set(ObjectId::ROOT, &theme).into()
    }

    pub const SCALES: [u32; 9] = [75, 90, 100, 110, 125, 150, 175, 200, 250];
    pub const DEFAULT_SCALE: u32 = 100;

    pub fn scale_percent(&self) -> u32 {
        self.scale_percent
            .filter(|percent| *percent > 0)
            .unwrap_or(Self::DEFAULT_SCALE)
    }

    pub fn scale(&self) -> f32 {
        self.scale_percent() as f32 / 100.0
    }

    pub fn set_scale_percent(percent: Option<u32>) -> Edit {
        Self::SCALE_PERCENT.set(ObjectId::ROOT, &percent).into()
    }
}

impl Layered for UiSettings {
    fn over(&self, lower: &Self) -> Self {
        Self {
            theme: self.theme.or(lower.theme),
            scale_percent: self.scale_percent.or(lower.scale_percent),
        }
    }
}

impl Root for UiSettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7569_2d73_6574_7469_6e67_732d_626c_6b32);
}

pub type UiSettingsContent = Document<UiSettings>;
