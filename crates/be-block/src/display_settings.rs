use std::time::Duration;

use be_model::{Document, Edit, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Root;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_millihertz: u32,
}

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
pub struct MonitorSettings {
    pub mode: Option<DisplayMode>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct DisplaySettings {
    pub monitors: Map<String, MonitorSettings>,
    pub screen_off: Option<ScreenOff>,
    pub lock_after: Option<LockAfter>,
}

impl DisplaySettings {
    pub fn monitor(&self, id: &str) -> MonitorSettings {
        self.monitors.get(id).copied().unwrap_or_default()
    }

    pub fn mode(&self, id: &str) -> Option<DisplayMode> {
        self.monitor(id).mode
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

    pub fn set_mode(id: &str, mode: Option<DisplayMode>) -> Edit {
        let monitor = MonitorSettings { mode };
        let stored = (monitor != MonitorSettings::default()).then_some(&monitor);
        Self::MONITORS
            .put(ObjectId::ROOT, &id.to_owned(), stored)
            .into()
    }
}

impl Root for DisplaySettings {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6469_7370_6c61_792d_7365_7474_696e_6773);
}

pub type DisplaySettingsContent = Document<DisplaySettings>;
