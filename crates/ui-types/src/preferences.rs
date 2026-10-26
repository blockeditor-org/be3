pub use crate::types::{CanvasTouch, Motion, PreferenceLayer, Preferences, ThemeKind};

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

impl CanvasTouch {
    pub const ALL: [Self; 2] = [Self::On, Self::Off];

    pub fn reaches(self) -> bool {
        self == Self::On
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::On => "On",
            Self::Off => "Off",
        }
    }
}

impl PreferenceLayer {
    pub const EMPTY: Self = Self {
        theme: None,
        motion: None,
        canvas_touch: None,
    };

    pub fn over(self, lower: Self) -> Self {
        Self {
            theme: self.theme.or(lower.theme),
            motion: self.motion.or(lower.motion),
            canvas_touch: self.canvas_touch.or(lower.canvas_touch),
        }
    }

    pub fn resolve(self) -> Preferences {
        Preferences {
            theme: self.theme.unwrap_or_default(),
            motion: self.motion.unwrap_or_default(),
            canvas_touch: self.canvas_touch.unwrap_or_default(),
        }
    }
}

impl From<Preferences> for PreferenceLayer {
    fn from(preferences: Preferences) -> Self {
        Self {
            theme: Some(preferences.theme),
            motion: Some(preferences.motion),
            canvas_touch: Some(preferences.canvas_touch),
        }
    }
}

#[cfg(test)]
mod tests;
