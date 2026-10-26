use std::time::Duration;

use be_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;
use crate::setting::Setting;
use crate::settings::Layered;
pub use ui_types::{CanvasTouch, DisplayMode, Motion, PreferenceLayer, Rotation, ThemeKind};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ScreenOff {
    After { minutes: u32 },
    Never,
}

impl ScreenOff {
    pub const DEFAULT: Self = Self::After { minutes: 10 };

    pub fn after(self) -> Option<Duration> {
        match self {
            Self::After { minutes } => Some(Duration::from_secs(u64::from(minutes.max(1)) * 60)),
            Self::Never => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum LockAfter {
    WithScreens,
    After { minutes: u32 },
    Never,
}

impl LockAfter {
    pub const DEFAULT: Self = Self::WithScreens;

    pub fn after(self, screen_off: ScreenOff) -> Option<Duration> {
        match self {
            Self::WithScreens => screen_off.after(),
            Self::After { minutes } => ScreenOff::After { minutes }.after(),
            Self::Never => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ScreenSettings {
    pub mode: Option<DisplayMode>,
    pub rotation: Option<Rotation>,
}

impl ScreenSettings {
    fn over(self, lower: Self) -> Self {
        Self {
            mode: self.mode.or(lower.mode),
            rotation: self.rotation.or(lower.rotation),
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct DisplaySettings {
    pub theme: Option<ThemeKind>,
    pub scale_percent: Option<u32>,
    pub screens: Map<String, ScreenSettings>,
    pub rotate_in_bar: Option<bool>,
    pub screen_off: Option<ScreenOff>,
    pub lock_after: Option<LockAfter>,
    pub motion: Option<Motion>,
    pub canvas_touch: Option<CanvasTouch>,
    pub keyboard_percent: Option<u32>,
}

impl DisplaySettings {
    pub const SCALES: [u32; 9] = [75, 90, 100, 110, 125, 150, 175, 200, 250];
    pub const DEFAULT_SCALE: u32 = 100;
    pub const KEYBOARD_HEIGHTS: [u32; 7] = [20, 25, 30, 35, 40, 45, 50];
    pub const DEFAULT_KEYBOARD_HEIGHT: u32 = 35;

    pub fn theme(&self) -> ThemeKind {
        self.theme.unwrap_or_default()
    }

    pub fn preferences(&self) -> PreferenceLayer {
        PreferenceLayer {
            theme: self.theme,
            motion: self.motion,
            canvas_touch: self.canvas_touch,
        }
    }

    pub fn set_theme(theme: Option<ThemeKind>) -> Edit {
        Self::THEME.set(ObjectId::ROOT, &theme).into()
    }

    pub fn set_motion(motion: Option<Motion>) -> Edit {
        Self::MOTION.set(ObjectId::ROOT, &motion).into()
    }

    pub fn set_canvas_touch(canvas_touch: Option<CanvasTouch>) -> Edit {
        Self::CANVAS_TOUCH.set(ObjectId::ROOT, &canvas_touch).into()
    }

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

    pub fn keyboard_percent(&self) -> u32 {
        self.keyboard_percent
            .filter(|percent| *percent > 0)
            .unwrap_or(Self::DEFAULT_KEYBOARD_HEIGHT)
    }

    pub fn keyboard_share(&self) -> f32 {
        self.keyboard_percent() as f32 / 100.0
    }

    pub fn set_keyboard_percent(percent: Option<u32>) -> Edit {
        Self::KEYBOARD_PERCENT.set(ObjectId::ROOT, &percent).into()
    }

    pub fn screen(&self, id: &str) -> ScreenSettings {
        self.screens.get(id).copied().unwrap_or_default()
    }

    pub fn mode(&self, id: &str) -> Option<DisplayMode> {
        self.screen(id).mode
    }

    pub fn rotation(&self, id: &str) -> Rotation {
        self.screen(id).rotation.unwrap_or_default()
    }

    pub fn rotate_in_bar(&self) -> bool {
        self.rotate_in_bar.unwrap_or(false)
    }

    pub fn set_rotate_in_bar(shown: Option<bool>) -> Edit {
        Self::ROTATE_IN_BAR.set(ObjectId::ROOT, &shown).into()
    }

    pub fn screen_off(&self) -> ScreenOff {
        self.screen_off.unwrap_or(ScreenOff::DEFAULT)
    }

    pub fn lock_after(&self) -> LockAfter {
        self.lock_after.unwrap_or(LockAfter::DEFAULT)
    }

    pub fn lock_time(&self) -> Option<Duration> {
        self.lock_after().after(self.screen_off())
    }

    pub fn set_lock_after(lock_after: Option<LockAfter>) -> Edit {
        Self::LOCK_AFTER.set(ObjectId::ROOT, &lock_after).into()
    }

    pub fn set_screen_off(screen_off: Option<ScreenOff>) -> Edit {
        Self::SCREEN_OFF.set(ObjectId::ROOT, &screen_off).into()
    }

    pub fn set_mode(&self, id: &str, mode: Option<DisplayMode>) -> Edit {
        self.store_screen(
            id,
            ScreenSettings {
                mode,
                ..self.screen(id)
            },
        )
    }

    pub fn set_rotation(&self, id: &str, rotation: Option<Rotation>) -> Edit {
        self.store_screen(
            id,
            ScreenSettings {
                rotation,
                ..self.screen(id)
            },
        )
    }

    fn store_screen(&self, id: &str, screen: ScreenSettings) -> Edit {
        let stored = (screen != ScreenSettings::default()).then_some(&screen);
        Self::SCREENS
            .put(ObjectId::ROOT, &id.to_owned(), stored)
            .into()
    }
}

impl DisplaySettings {
    pub fn keyed_by_connector(id: &str) -> bool {
        !id.contains('|')
    }

    pub fn theme_setting() -> Setting<Self, ThemeKind> {
        Setting::new(
            "theme",
            "Theme",
            |settings: &Self| settings.theme,
            |_, theme| Self::set_theme(theme),
        )
        .with_default(Some(ThemeKind::default()))
    }

    pub fn motion_setting() -> Setting<Self, Motion> {
        Setting::new(
            "motion",
            "Motion",
            |settings: &Self| settings.motion,
            |_, motion| Self::set_motion(motion),
        )
        .with_default(Some(Motion::default()))
    }

    pub fn canvas_touch_setting() -> Setting<Self, CanvasTouch> {
        Setting::new(
            "canvas-touch",
            "Touch on canvases",
            |settings: &Self| settings.canvas_touch,
            |_, canvas_touch| Self::set_canvas_touch(canvas_touch),
        )
        .with_default(Some(CanvasTouch::default()))
        .only_on_this_device(true)
    }

    pub fn scale_setting() -> Setting<Self, u32> {
        Setting::new(
            "scale",
            "Scale",
            |settings: &Self| settings.scale_percent,
            |_, percent| Self::set_scale_percent(percent),
        )
        .with_default(Some(Self::DEFAULT_SCALE))
    }

    pub fn keyboard_setting() -> Setting<Self, u32> {
        Setting::new(
            "keyboard-height",
            "On-screen keyboard height",
            |settings: &Self| settings.keyboard_percent,
            |_, percent| Self::set_keyboard_percent(percent),
        )
        .with_default(Some(Self::DEFAULT_KEYBOARD_HEIGHT))
        .only_on_this_device(true)
    }

    pub fn rotate_in_bar_setting() -> Setting<Self, bool> {
        Setting::new(
            "rotate-in-bar",
            "The rotate button",
            |settings: &Self| settings.rotate_in_bar,
            |_, shown| Self::set_rotate_in_bar(shown),
        )
        .with_default(Some(false))
    }

    pub fn screen_off_setting() -> Setting<Self, ScreenOff> {
        Setting::new(
            "screen-off",
            "Screens off",
            |settings: &Self| settings.screen_off,
            |_, screen_off| Self::set_screen_off(screen_off),
        )
        .with_default(Some(ScreenOff::DEFAULT))
    }

    pub fn lock_after_setting() -> Setting<Self, LockAfter> {
        Setting::new(
            "lock-after",
            "Lock",
            |settings: &Self| settings.lock_after,
            |_, lock_after| Self::set_lock_after(lock_after),
        )
        .with_default(Some(LockAfter::DEFAULT))
    }

    pub fn mode_setting(id: &str) -> Setting<Self, DisplayMode> {
        let (read, write) = (id.to_owned(), id.to_owned());
        Setting::new(
            format!("screen.{id}.mode"),
            "Resolution",
            move |settings: &Self| settings.mode(&read),
            move |own: &Self, mode| own.set_mode(&write, mode),
        )
        .only_on_this_device(Self::keyed_by_connector(id))
    }

    pub fn rotation_setting(id: &str) -> Setting<Self, Rotation> {
        let (read, write) = (id.to_owned(), id.to_owned());
        Setting::new(
            format!("screen.{id}.rotation"),
            "Rotation",
            move |settings: &Self| settings.screen(&read).rotation,
            move |own: &Self, rotation| own.set_rotation(&write, rotation),
        )
        .with_default(Some(Rotation::Normal))
        .only_on_this_device(Self::keyed_by_connector(id))
    }
}

impl Layered for DisplaySettings {
    const NAME: &'static str = "Display";

    fn over(&self, lower: &Self) -> Self {
        let screens = lower
            .screens
            .iter()
            .chain(self.screens.iter())
            .map(|(id, _)| (id.clone(), self.screen(id).over(lower.screen(id))))
            .collect();
        Self {
            theme: self.theme.or(lower.theme),
            scale_percent: self.scale_percent.or(lower.scale_percent),
            screens,
            rotate_in_bar: self.rotate_in_bar.or(lower.rotate_in_bar),
            screen_off: self.screen_off.or(lower.screen_off),
            lock_after: self.lock_after.or(lower.lock_after),
            motion: self.motion.or(lower.motion),
            canvas_touch: self.canvas_touch.or(lower.canvas_touch),
            keyboard_percent: self.keyboard_percent.or(lower.keyboard_percent),
        }
    }
}

impl Root for DisplaySettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6469_7370_6c61_792d_7365_7474_696e_6773);
}

pub type DisplaySettingsContent = Document<DisplaySettings>;
