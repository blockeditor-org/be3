use std::time::Duration;

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

pub(crate) const ANSWER_WITHIN: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prompt {
    Settled,
    Asking { round: u64, left: Duration },
    Expired,
}

#[derive(Clone, Copy)]
struct Asked {
    round: u64,
    due: Duration,
}

#[derive(Default)]
pub(crate) struct Guard {
    screens: Vec<Screen>,
    applied: DisplaySettings,
    previous: Option<DisplaySettings>,
    asked: Option<Asked>,
    rounds: u64,
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
            self.asked = None;
        }
        !changed
    }

    pub(crate) fn prompt(&mut self, now: Duration) -> Prompt {
        if !self.asking() {
            self.asked = None;
            return Prompt::Settled;
        }
        let asked = match self.asked {
            Some(asked) => asked,
            None => {
                self.rounds += 1;
                let asked = Asked {
                    round: self.rounds,
                    due: now + ANSWER_WITHIN,
                };
                self.asked = Some(asked);
                asked
            }
        };
        match asked.due.saturating_sub(now) {
            Duration::ZERO => Prompt::Expired,
            left => Prompt::Asking {
                round: asked.round,
                left,
            },
        }
    }

    pub(crate) fn asking(&self) -> bool {
        self.previous.is_some() || self.untried().next().is_some()
    }

    pub(crate) fn keep(&mut self) -> Vec<Edit> {
        self.previous = None;
        self.asked = None;
        let edits: Vec<Edit> = self
            .untried()
            .map(|screen| DisplaySettings::set_mode(&screen.id, Some(screen.default)))
            .collect();
        self.edit(&edits);
        edits
    }

    pub(crate) fn revert(&mut self) -> Vec<Edit> {
        self.asked = None;
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
