use be_model::{Change, Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;
use crate::setting::Setting;
use crate::settings::Layered;
pub use ui_types::pointer::{MAX_POINTER_SPEED, MIN_POINTER_SPEED};
pub use ui_types::{InputDevice, PointerSetting, PointerSettings, PointerSpeed};

pub const MIN_REPEAT_DELAY: u32 = 150;
pub const MAX_REPEAT_DELAY: u32 = 2000;
pub const DEFAULT_REPEAT_DELAY: u32 = 600;
pub const MIN_REPEAT_RATE: u32 = 1;
pub const MAX_REPEAT_RATE: u32 = 100;
pub const DEFAULT_REPEAT_RATE: u32 = 25;

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RemapperChoice {
    pub block: Uuid,
    pub editor: Uuid,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct InputSettings {
    pub keyboard_layout: Option<String>,
    pub keyboard_variant: Option<String>,
    pub keyboard_options: Option<String>,
    pub repeat_delay: Option<RepeatDelay>,
    pub repeat_rate: Option<RepeatRate>,
    pub every_pointer: PointerSettings,
    pub pointers: Map<InputDevice, PointerSettings>,
    pub middle_click_paste: Option<bool>,
    pub remapper: Option<RemapperChoice>,
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

    pub fn middle_click_paste(&self) -> bool {
        self.middle_click_paste.unwrap_or(false)
    }

    pub fn set_middle_click_paste(on: Option<bool>) -> Edit {
        Self::MIDDLE_CLICK_PASTE.set(ObjectId::ROOT, &on).into()
    }

    pub fn set_remapper(choice: Option<RemapperChoice>) -> Edit {
        Self::REMAPPER.set(ObjectId::ROOT, &choice).into()
    }

    pub fn device_pointer(&self, device: &InputDevice) -> PointerSettings {
        self.pointers.get(device).copied().unwrap_or_default()
    }

    pub fn pointer(&self, device: &InputDevice) -> PointerSettings {
        self.device_pointer(device).or(self.every_pointer)
    }

    pub fn set_keyboard_layout(layout: Option<&str>) -> Edit {
        Self::KEYBOARD_LAYOUT
            .set(ObjectId::ROOT, &keyboard_name(layout))
            .into()
    }

    pub fn set_keyboard_variant(variant: Option<&str>) -> Edit {
        Self::KEYBOARD_VARIANT
            .set(ObjectId::ROOT, &keyboard_name(variant))
            .into()
    }

    pub fn set_keyboard_options(options: Option<&str>) -> Edit {
        Self::KEYBOARD_OPTIONS
            .set(ObjectId::ROOT, &keyboard_name(options))
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

    pub fn set_every_pointer(&self, setting: PointerSetting) -> Edit {
        let every = self.every_pointer.with(setting);
        std::iter::once(Self::EVERY_POINTER.set(ObjectId::ROOT, &every))
            .chain(self.pointers.iter().filter_map(|(device, pointer)| {
                let cleared = pointer.with(setting.cleared());
                (cleared != *pointer).then(|| Self::store_pointer(device, cleared))
            }))
            .collect()
    }

    pub fn set_device_pointer(&self, device: &InputDevice, setting: PointerSetting) -> Edit {
        Self::store_pointer(device, self.device_pointer(device).with(setting)).into()
    }

    fn store_pointer(device: &InputDevice, pointer: PointerSettings) -> Change {
        let stored = (pointer != PointerSettings::default()).then_some(&pointer);
        Self::POINTERS.put(ObjectId::ROOT, device, stored)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerField {
    Speed,
    TapToClick,
    NaturalScroll,
}

impl PointerField {
    fn name(self) -> &'static str {
        match self {
            Self::Speed => "Pointer speed",
            Self::TapToClick => "Tap to click",
            Self::NaturalScroll => "Natural scrolling",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Speed => "speed",
            Self::TapToClick => "tap-to-click",
            Self::NaturalScroll => "natural-scroll",
        }
    }

    pub fn read(self, pointer: &PointerSettings) -> Option<PointerSetting> {
        match self {
            Self::Speed => pointer
                .speed
                .map(|speed| PointerSetting::Speed(Some(speed))),
            Self::TapToClick => pointer
                .tap_to_click
                .map(|on| PointerSetting::TapToClick(Some(on))),
            Self::NaturalScroll => pointer
                .natural_scroll
                .map(|on| PointerSetting::NaturalScroll(Some(on))),
        }
    }

    fn unset(self) -> PointerSetting {
        match self {
            Self::Speed => PointerSetting::Speed(None),
            Self::TapToClick => PointerSetting::TapToClick(None),
            Self::NaturalScroll => PointerSetting::NaturalScroll(None),
        }
    }
}

impl InputSettings {
    fn text_setting(
        key: &'static str,
        name: &'static str,
        get: fn(&Self) -> Option<String>,
        set: fn(Option<&str>) -> Edit,
    ) -> Setting<Self, String> {
        Setting::new(key, name, get, move |_, value: Option<String>| {
            set(value.as_deref())
        })
    }

    pub fn layout_setting() -> Setting<Self, String> {
        Self::text_setting(
            "keyboard-layout",
            "Keyboard layout",
            |settings| settings.keyboard_layout.clone(),
            Self::set_keyboard_layout,
        )
    }

    pub fn variant_setting() -> Setting<Self, String> {
        Self::text_setting(
            "keyboard-variant",
            "Keyboard variant",
            |settings| settings.keyboard_variant.clone(),
            Self::set_keyboard_variant,
        )
    }

    pub fn options_setting() -> Setting<Self, String> {
        Self::text_setting(
            "keyboard-options",
            "Keyboard options",
            |settings| settings.keyboard_options.clone(),
            Self::set_keyboard_options,
        )
    }

    pub fn repeat_delay_setting() -> Setting<Self, u32> {
        Setting::new(
            "repeat-delay",
            "Repeat delay",
            |settings: &Self| settings.repeat_delay.map(RepeatDelay::milliseconds),
            |_, delay| Self::set_repeat_delay(delay),
        )
        .with_default(Some(DEFAULT_REPEAT_DELAY))
    }

    pub fn repeat_rate_setting() -> Setting<Self, u32> {
        Setting::new(
            "repeat-rate",
            "Repeat rate",
            |settings: &Self| settings.repeat_rate.map(RepeatRate::per_second),
            |_, rate| Self::set_repeat_rate(rate),
        )
        .with_default(Some(DEFAULT_REPEAT_RATE))
    }

    pub fn middle_click_paste_setting() -> Setting<Self, bool> {
        Setting::new(
            "middle-click-paste",
            "Middle-click paste",
            |settings: &Self| settings.middle_click_paste,
            |_, on| Self::set_middle_click_paste(on),
        )
        .with_default(Some(false))
    }

    pub fn remapper_setting() -> Setting<Self, RemapperChoice> {
        Setting::new(
            "remapper",
            "Remaps",
            |settings: &Self| settings.remapper,
            |_, choice| Self::set_remapper(choice),
        )
    }

    pub fn pointer_setting(
        device: Option<InputDevice>,
        field: PointerField,
    ) -> Setting<Self, PointerSetting> {
        let key = match &device {
            Some(device) => format!(
                "pointer.{}.{}.{}.{}",
                device.name,
                device.vendor,
                device.product,
                field.key()
            ),
            None => format!("pointer.{}", field.key()),
        };
        let read = device.clone();
        Setting::new(
            key,
            field.name(),
            move |settings: &Self| match &read {
                Some(device) => field.read(&settings.pointer(device)),
                None => field.read(&settings.every_pointer),
            },
            move |own: &Self, value| {
                let setting = value.unwrap_or(field.unset());
                match &device {
                    Some(device) => own.set_device_pointer(device, setting),
                    None => own.set_every_pointer(setting),
                }
            },
        )
    }
}

fn keyboard_name(name: Option<&str>) -> Option<String> {
    name.map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

impl Layered for InputSettings {
    const NAME: &'static str = "Input";

    fn over(&self, lower: &Self) -> Self {
        let pointers = lower
            .pointers
            .iter()
            .map(|(device, pointer)| {
                let upper = self.device_pointer(device).or(self.every_pointer);
                (device.clone(), upper.or(*pointer))
            })
            .chain(
                self.pointers
                    .iter()
                    .filter(|(device, _)| !lower.pointers.contains_key(*device))
                    .map(|(device, pointer)| (device.clone(), *pointer)),
            )
            .collect();
        Self {
            keyboard_layout: self
                .keyboard_layout
                .clone()
                .or(lower.keyboard_layout.clone()),
            keyboard_variant: self
                .keyboard_variant
                .clone()
                .or(lower.keyboard_variant.clone()),
            keyboard_options: self
                .keyboard_options
                .clone()
                .or(lower.keyboard_options.clone()),
            repeat_delay: self.repeat_delay.or(lower.repeat_delay),
            repeat_rate: self.repeat_rate.or(lower.repeat_rate),
            every_pointer: self.every_pointer.or(lower.every_pointer),
            pointers,
            middle_click_paste: self.middle_click_paste.or(lower.middle_click_paste),
            remapper: self.remapper.or(lower.remapper),
        }
    }
}

impl Root for InputSettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x696e_7075_742d_7365_7474_696e_6773_2d31);

    fn references(&self) -> Vec<Uuid> {
        self.remapper.iter().map(|choice| choice.block).collect()
    }
}

pub type InputSettingsContent = Document<InputSettings>;
