use be_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;

pub const MIN_REPEAT_DELAY: u32 = 150;
pub const MAX_REPEAT_DELAY: u32 = 2000;
pub const DEFAULT_REPEAT_DELAY: u32 = 600;
pub const MIN_REPEAT_RATE: u32 = 1;
pub const MAX_REPEAT_RATE: u32 = 100;
pub const DEFAULT_REPEAT_RATE: u32 = 25;
pub const MIN_POINTER_SPEED: f32 = -1.0;
pub const MAX_POINTER_SPEED: f32 = 1.0;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepeatDelay(u32);

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

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct InputDevice {
    pub name: String,
    pub vendor: u32,
    pub product: u32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PointerSettings {
    pub speed: Option<PointerSpeed>,
    pub tap_to_click: Option<bool>,
    pub natural_scroll: Option<bool>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct InputSettings {
    pub keyboard_layout: Option<String>,
    pub keyboard_variant: Option<String>,
    pub keyboard_options: Option<String>,
    pub repeat_delay: Option<RepeatDelay>,
    pub repeat_rate: Option<RepeatRate>,
    pub pointers: Map<InputDevice, PointerSettings>,
}

impl InputSettings {
    pub fn repeat_delay(&self) -> u32 {
        self.repeat_delay
            .map_or(DEFAULT_REPEAT_DELAY, RepeatDelay::milliseconds)
    }

    pub fn repeat_rate(&self) -> u32 {
        self.repeat_rate
            .map_or(DEFAULT_REPEAT_RATE, RepeatRate::per_second)
    }

    pub fn pointer(&self, device: &InputDevice) -> PointerSettings {
        self.pointers.get(device).copied().unwrap_or_default()
    }

    pub fn set_keyboard_layout(layout: Option<&str>) -> Edit {
        Self::KEYBOARD_LAYOUT
            .set(
                ObjectId::ROOT,
                &layout.map(|layout| layout.trim().to_owned()),
            )
            .into()
    }

    pub fn set_keyboard_variant(variant: Option<&str>) -> Edit {
        Self::KEYBOARD_VARIANT
            .set(
                ObjectId::ROOT,
                &variant.map(|variant| variant.trim().to_owned()),
            )
            .into()
    }

    pub fn set_keyboard_options(options: Option<&str>) -> Edit {
        Self::KEYBOARD_OPTIONS
            .set(
                ObjectId::ROOT,
                &options.map(|options| options.trim().to_owned()),
            )
            .into()
    }

    pub fn set_repeat_delay(milliseconds: Option<u32>) -> Edit {
        Self::REPEAT_DELAY
            .set(ObjectId::ROOT, &milliseconds.map(RepeatDelay::new))
            .into()
    }

    pub fn set_repeat_rate(per_second: Option<u32>) -> Edit {
        Self::REPEAT_RATE
            .set(ObjectId::ROOT, &per_second.map(RepeatRate::new))
            .into()
    }

    pub fn set_pointer(device: &InputDevice, settings: PointerSettings) -> Edit {
        let settings = PointerSettings {
            speed: settings.speed.map(|speed| PointerSpeed::new(speed.get())),
            ..settings
        };
        let stored = (settings != PointerSettings::default()).then_some(&settings);
        Self::POINTERS.put(ObjectId::ROOT, device, stored).into()
    }
}

impl Root for InputSettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x696e_7075_742d_7365_7474_696e_6773_2d31);
}

pub type InputSettingsContent = Document<InputSettings>;
