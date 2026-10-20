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

pub(crate) type ModeChange = (String, Option<DisplayMode>);

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

    pub(crate) fn keep(&mut self) -> Vec<ModeChange> {
        self.previous = None;
        self.asked = None;
        let changes: Vec<ModeChange> = self
            .untried()
            .map(|screen| (screen.id.clone(), Some(screen.default)))
            .collect();
        self.change(&changes);
        changes
    }

    pub(crate) fn revert(&mut self) -> Vec<ModeChange> {
        self.asked = None;
        let mut changes = Vec::new();
        if let Some(previous) = self.previous.take() {
            let restored: Vec<ModeChange> = self
                .screens
                .iter()
                .map(|screen| &screen.id)
                .filter(|id| previous.mode(id) != self.applied.mode(id))
                .map(|id| (id.clone(), previous.mode(id)))
                .collect();
            self.change(&restored);
            changes.extend(restored);
        }
        let fallen_back: Vec<ModeChange> = self
            .untried()
            .map(|screen| (screen.id.clone(), Some(screen.preferred)))
            .collect();
        self.change(&fallen_back);
        changes.extend(fallen_back);
        changes
    }

    fn untried(&self) -> impl Iterator<Item = &Screen> {
        self.screens
            .iter()
            .filter(|screen| screen.untried(&self.applied))
    }

    fn change(&mut self, changes: &[ModeChange]) {
        for (id, mode) in changes {
            let edit = self.applied.set_mode(id, *mode);
            self.applied = edited(&self.applied, &[edit]);
        }
    }
}

pub(crate) fn written(top: &DisplaySettings, changes: &[ModeChange]) -> Vec<Edit> {
    let mut top = top.clone();
    changes
        .iter()
        .map(|(id, mode)| {
            let edit = top.set_mode(id, *mode);
            top = edited(&top, std::slice::from_ref(&edit));
            edit
        })
        .collect()
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
