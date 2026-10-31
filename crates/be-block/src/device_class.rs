use std::rc::Rc;

use be_model::Edit;
use serde::{Deserialize, Serialize};

use crate::display_settings::{DisplaySettings, Motion, ThemeKind};
use crate::setting::Setting;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum DeviceClass {
    #[default]
    Desktop,
    Phone,
    Tablet,
    EpaperTablet,
}

impl DeviceClass {
    pub const ALL: [Self; 4] = [Self::Desktop, Self::Phone, Self::Tablet, Self::EpaperTablet];

    pub fn name(self) -> &'static str {
        match self {
            Self::Desktop => "Desktop",
            Self::Phone => "Phone",
            Self::Tablet => "Tablet",
            Self::EpaperTablet => "E-paper tablet",
        }
    }

    pub fn detect(epaper: bool, android: bool) -> Self {
        match (epaper, android) {
            (true, _) => Self::EpaperTablet,
            (false, true) => Self::Phone,
            (false, false) => Self::Desktop,
        }
    }

    pub fn display_defaults(self) -> Vec<ClassDefault<DisplaySettings>> {
        match self {
            Self::Desktop | Self::Phone => Vec::new(),
            Self::Tablet => bar_tools(),
            Self::EpaperTablet => [
                ClassDefault::new(
                    DisplaySettings::theme_setting(),
                    ThemeKind::EPaper,
                    |theme| theme.name().to_owned(),
                ),
                ClassDefault::new(
                    DisplaySettings::motion_setting(),
                    Motion::Instant,
                    |motion| motion.name().to_owned(),
                ),
                ClassDefault::new(DisplaySettings::scale_setting(), 150, |percent| {
                    format!("{percent}%")
                }),
            ]
            .into_iter()
            .chain(bar_tools())
            .collect(),
        }
    }
}

fn bar_tools() -> Vec<ClassDefault<DisplaySettings>> {
    vec![
        ClassDefault::new(DisplaySettings::rotate_in_bar_setting(), true, |shown| {
            on_off(*shown)
        }),
        ClassDefault::new(DisplaySettings::keyboard_in_bar_setting(), true, |shown| {
            on_off(*shown)
        }),
    ]
}

fn on_off(on: bool) -> String {
    match on {
        true => "On".to_owned(),
        false => "Off".to_owned(),
    }
}

trait Entry<R> {
    fn key(&self) -> &str;
    fn name(&self) -> &str;
    fn write(&self, own: &R) -> Edit;
    fn clear(&self, own: &R) -> Edit;
    fn holds(&self, own: &R) -> bool;
    fn kept(&self, own: &R) -> Option<String>;
    fn differs(&self, own: &R, every_device: &R) -> bool;
    fn every_device(&self, every_device: &R) -> String;
}

struct Typed<R, V> {
    setting: Setting<R, V>,
    value: V,
    describe: fn(&V) -> String,
}

impl<R, V: Clone + PartialEq> Typed<R, V> {
    fn everywhere(&self, every_device: &R) -> Option<V> {
        self.setting
            .get(every_device)
            .or(self.setting.default_value())
    }
}

impl<R, V: Clone + PartialEq> Entry<R> for Typed<R, V> {
    fn key(&self) -> &str {
        self.setting.key()
    }

    fn name(&self) -> &str {
        self.setting.name()
    }

    fn write(&self, own: &R) -> Edit {
        self.setting.set(own, Some(self.value.clone()))
    }

    fn clear(&self, own: &R) -> Edit {
        self.setting.set(own, None)
    }

    fn holds(&self, own: &R) -> bool {
        self.setting.get(own).as_ref() == Some(&self.value)
    }

    fn kept(&self, own: &R) -> Option<String> {
        self.setting.get(own).map(|value| (self.describe)(&value))
    }

    fn differs(&self, own: &R, every_device: &R) -> bool {
        self.setting
            .get(own)
            .is_some_and(|kept| Some(kept) != self.everywhere(every_device))
    }

    fn every_device(&self, every_device: &R) -> String {
        self.everywhere(every_device)
            .map(|value| (self.describe)(&value))
            .unwrap_or_default()
    }
}

pub struct ClassDefault<R>(Rc<dyn Entry<R>>);

impl<R> Clone for ClassDefault<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<R: 'static> ClassDefault<R> {
    pub fn new<V: Clone + PartialEq + 'static>(
        setting: Setting<R, V>,
        value: V,
        describe: fn(&V) -> String,
    ) -> Self {
        Self(Rc::new(Typed {
            setting,
            value,
            describe,
        }))
    }

    pub fn key(&self) -> &str {
        self.0.key()
    }

    pub fn name(&self) -> &str {
        self.0.name()
    }

    pub fn write(&self, own: &R) -> Edit {
        self.0.write(own)
    }

    pub fn clear(&self, own: &R) -> Edit {
        self.0.clear(own)
    }

    pub fn holds(&self, own: &R) -> bool {
        self.0.holds(own)
    }

    pub fn kept(&self, own: &R) -> Option<String> {
        self.0.kept(own)
    }

    pub fn differs(&self, own: &R, every_device: &R) -> bool {
        self.0.differs(own, every_device)
    }

    pub fn every_device(&self, every_device: &R) -> String {
        self.0.every_device(every_device)
    }
}

impl DeviceClass {
    pub fn display_settings(self) -> DisplaySettings {
        let mut content = crate::DisplaySettingsContent::new(&DisplaySettings::default());
        for default in self.display_defaults() {
            content.apply(&default.write(&content.root()));
        }
        content.root()
    }

    pub fn display_edits(self, previous: Option<Self>, own: &DisplaySettings) -> Vec<Edit> {
        let defaults = self.display_defaults();
        let cleared = previous
            .into_iter()
            .flat_map(Self::display_defaults)
            .filter(|old| {
                old.holds(own) && !defaults.iter().any(|default| default.key() == old.key())
            })
            .map(|old| old.clear(own));
        cleared
            .chain(defaults.iter().map(|default| default.write(own)))
            .collect()
    }
}
