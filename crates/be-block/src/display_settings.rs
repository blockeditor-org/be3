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

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct MonitorSettings {
    pub mode: Option<DisplayMode>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct DisplaySettings {
    pub monitors: Map<String, MonitorSettings>,
}

impl DisplaySettings {
    pub fn monitor(&self, id: &str) -> MonitorSettings {
        self.monitors.get(id).copied().unwrap_or_default()
    }

    pub fn mode(&self, id: &str) -> Option<DisplayMode> {
        self.monitor(id).mode
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
