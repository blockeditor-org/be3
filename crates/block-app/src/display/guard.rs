use be_block::DisplaySettings;
use be_block::be_model::{Document, Edit};
use be_block::display_settings::DisplayMode;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) struct Screen {
    pub(crate) id: String,
    pub(crate) modes: Vec<DisplayMode>,
    pub(crate) default: DisplayMode,
    pub(crate) preferred: DisplayMode,
}

impl Screen {
    fn showing(&self, settings: &DisplaySettings) -> DisplayMode {
        settings
            .mode(&self.id)
            .filter(|mode| self.modes.contains(mode))
            .unwrap_or(self.default)
    }

    fn untried(&self, settings: &DisplaySettings) -> bool {
        self.default != self.preferred
            && settings.mode(&self.id) != Some(self.default)
            && self.showing(settings) == self.default
    }
}

#[derive(Default)]
pub(crate) struct Guard {
    screens: Vec<Screen>,
    applied: DisplaySettings,
    previous: Option<DisplaySettings>,
}

impl Guard {
    pub(crate) fn applied(&self) -> &DisplaySettings {
        &self.applied
    }

    pub(crate) fn set_screens(&mut self, screens: Vec<Screen>) {
        self.screens = screens;
    }

    pub(crate) fn take(&mut self, settings: DisplaySettings) -> bool {
        let base = self.previous.take().unwrap_or_else(|| self.applied.clone());
        let changed = self
            .screens
            .iter()
            .any(|screen| screen.showing(&base) != screen.showing(&settings));
        self.applied = settings;
        if changed {
            self.previous = Some(base);
        }
        !changed
    }

    pub(crate) fn asking(&self) -> bool {
        self.previous.is_some() || self.untried().next().is_some()
    }

    pub(crate) fn keep(&mut self) -> Vec<Edit> {
        self.previous = None;
        let edits: Vec<Edit> = self
            .untried()
            .map(|screen| DisplaySettings::set_mode(&screen.id, Some(screen.default)))
            .collect();
        self.edit(&edits);
        edits
    }

    pub(crate) fn revert(&mut self) -> Vec<Edit> {
        let mut edits = Vec::new();
        if let Some(previous) = self.previous.take() {
            let restored: Vec<Edit> = self
                .screens
                .iter()
                .map(|screen| &screen.id)
                .filter(|id| previous.monitor(id) != self.applied.monitor(id))
                .map(|id| DisplaySettings::set_mode(id, previous.mode(id)))
                .collect();
            self.edit(&restored);
            edits.extend(restored);
        }
        let fallen_back: Vec<Edit> = self
            .untried()
            .map(|screen| DisplaySettings::set_mode(&screen.id, Some(screen.preferred)))
            .collect();
        self.edit(&fallen_back);
        edits.extend(fallen_back);
        edits
    }

    fn untried(&self) -> impl Iterator<Item = &Screen> {
        self.screens
            .iter()
            .filter(|screen| screen.untried(&self.applied))
    }

    fn edit(&mut self, edits: &[Edit]) {
        self.applied = edited(&self.applied, edits);
    }
}

pub(crate) fn edited(settings: &DisplaySettings, edits: &[Edit]) -> DisplaySettings {
    let mut document = Document::new(settings);
    for edit in edits {
        document.apply(edit);
    }
    document.root()
}

#[cfg(test)]
mod tests;
