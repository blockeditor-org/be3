use be_model::{Document, Edit, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;

pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 3.0;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Zoom(f32);

impl Default for Zoom {
    fn default() -> Self {
        Self(1.0)
    }
}

impl Zoom {
    pub fn new(zoom: f32) -> Self {
        match zoom.is_finite() {
            true => Self(zoom.clamp(MIN_ZOOM, MAX_ZOOM)),
            false => Self::default(),
        }
    }

    pub fn get(self) -> f32 {
        Self::new(self.0).0
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct UiSettings {
    pub zoom: Zoom,
}

impl UiSettings {
    pub fn zoom(&self) -> f32 {
        self.zoom.get()
    }

    pub fn set_zoom(zoom: f32) -> Edit {
        Self::ZOOM.set(ObjectId::ROOT, &Zoom::new(zoom)).into()
    }
}

impl Root for UiSettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7569_2d73_6574_7469_6e67_732d_626c_6b03);
}

pub type UiSettingsContent = Document<UiSettings>;
