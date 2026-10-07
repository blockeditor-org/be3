use be_model::{Document, Edit, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;

pub const MIN_REPEAT_DELAY: u32 = 150;
pub const MAX_REPEAT_DELAY: u32 = 2000;
pub const MIN_REPEAT_RATE: u32 = 1;
pub const MAX_REPEAT_RATE: u32 = 100;
pub const MIN_POINTER_SPEED: f32 = -1.0;
pub const MAX_POINTER_SPEED: f32 = 1.0;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepeatDelay(u32);

impl Default for RepeatDelay {
    fn default() -> Self {
        Self(600)
    }
}

impl RepeatDelay {
    pub fn new(milliseconds: u32) -> Self {
        Self(milliseconds.clamp(MIN_REPEAT_DELAY, MAX_REPEAT_DELAY))
    }

    pub fn milliseconds(self) -> u32 {
        Self::new(self.0).0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepeatRate(u32);

impl Default for RepeatRate {
    fn default() -> Self {
        Self(25)
    }
}

impl RepeatRate {
    pub fn new(per_second: u32) -> Self {
        Self(per_second.clamp(MIN_REPEAT_RATE, MAX_REPEAT_RATE))
    }

    pub fn per_second(self) -> u32 {
        Self::new(self.0).0
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Default, Deserialize, Model, PartialEq, Serialize)]
pub struct InputSettings {
    pub keyboard_layout: String,
    pub keyboard_variant: String,
    pub keyboard_options: String,
    pub repeat_delay: RepeatDelay,
    pub repeat_rate: RepeatRate,
    pub pointer_speed: PointerSpeed,
    pub tap_to_click: Option<bool>,
    pub natural_scroll: bool,
}

impl InputSettings {
    pub fn set_keyboard_layout(layout: &str) -> Edit {
        Self::KEYBOARD_LAYOUT
            .set(ObjectId::ROOT, &layout.trim().to_owned())
            .into()
    }

    pub fn set_keyboard_variant(variant: &str) -> Edit {
        Self::KEYBOARD_VARIANT
            .set(ObjectId::ROOT, &variant.trim().to_owned())
            .into()
    }

    pub fn set_keyboard_options(options: &str) -> Edit {
        Self::KEYBOARD_OPTIONS
            .set(ObjectId::ROOT, &options.trim().to_owned())
            .into()
    }

    pub fn set_repeat_delay(milliseconds: u32) -> Edit {
        Self::REPEAT_DELAY
            .set(ObjectId::ROOT, &RepeatDelay::new(milliseconds))
            .into()
    }

    pub fn set_repeat_rate(per_second: u32) -> Edit {
        Self::REPEAT_RATE
            .set(ObjectId::ROOT, &RepeatRate::new(per_second))
            .into()
    }

    pub fn set_pointer_speed(speed: f32) -> Edit {
        Self::POINTER_SPEED
            .set(ObjectId::ROOT, &PointerSpeed::new(speed))
            .into()
    }

    pub fn set_tap_to_click(enabled: bool) -> Edit {
        Self::TAP_TO_CLICK
            .set(ObjectId::ROOT, &Some(enabled))
            .into()
    }

    pub fn reset_tap_to_click() -> Edit {
        Self::TAP_TO_CLICK.set(ObjectId::ROOT, &None).into()
    }

    pub fn set_natural_scroll(enabled: bool) -> Edit {
        Self::NATURAL_SCROLL.set(ObjectId::ROOT, &enabled).into()
    }
}

impl Root for InputSettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x696e_7075_742d_7365_7474_696e_6773_2d31);
}

pub type InputSettingsContent = Document<InputSettings>;
