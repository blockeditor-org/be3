use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeKind {
    #[default]
    Dark,
    OledDark,
    Light,
    EPaper,
}

impl ThemeKind {
    pub const ALL: [Self; 4] = [Self::Light, Self::Dark, Self::OledDark, Self::EPaper];

    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::OledDark => "OLED dark",
            Self::Light => "Light",
            Self::EPaper => "E-paper",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Motion {
    #[default]
    Animated,
    Instant,
}

impl Motion {
    pub const ALL: [Self; 2] = [Self::Animated, Self::Instant];

    pub fn animates(self) -> bool {
        self == Self::Animated
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Animated => "Animated",
            Self::Instant => "Reduced",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Preferences {
    pub theme: ThemeKind,
    pub motion: Motion,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PreferenceLayer {
    pub theme: Option<ThemeKind>,
    pub motion: Option<Motion>,
}

impl PreferenceLayer {
    pub const EMPTY: Self = Self {
        theme: None,
        motion: None,
    };

    pub fn over(self, lower: Self) -> Self {
        Self {
            theme: self.theme.or(lower.theme),
            motion: self.motion.or(lower.motion),
        }
    }

    pub fn resolve(self) -> Preferences {
        Preferences {
            theme: self.theme.unwrap_or_default(),
            motion: self.motion.unwrap_or_default(),
        }
    }
}

impl From<Preferences> for PreferenceLayer {
    fn from(preferences: Preferences) -> Self {
        Self {
            theme: Some(preferences.theme),
            motion: Some(preferences.motion),
        }
    }
}

#[cfg(test)]
mod tests;
