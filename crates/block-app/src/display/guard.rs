use std::collections::BTreeMap;
use std::time::Duration;

use be_block::be_model::{Document, Edit};
use be_block::display_settings::DisplayMode;
use be_block::{DisplaySettings, SettingsLayer};

use crate::local_settings::LayerContents;

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

    fn untried(&self, settings: &DisplaySettings, tried: &Tried) -> bool {
        let showing = self.showing(settings);
        showing != self.preferred && tried.get(&self.id) != Some(&showing)
    }
}

pub(crate) type Tried = BTreeMap<String, DisplayMode>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ModeChange {
    pub(crate) id: String,
    pub(crate) mode: Option<DisplayMode>,
    pub(crate) here: bool,
}

impl ModeChange {
    fn new(id: &str, mode: Option<DisplayMode>) -> Self {
        Self {
            id: id.to_owned(),
            mode,
            here: false,
        }
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
    tried: Tried,
}

impl Guard {
    pub(crate) fn applied(&self) -> &DisplaySettings {
        &self.applied
    }

    pub(crate) fn tried(&self) -> &Tried {
        &self.tried
    }

    pub(crate) fn remember(&mut self, tried: Tried) {
        self.tried = tried;
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
            .filter(|screen| self.applied.mode(&screen.id) != Some(screen.showing(&self.applied)))
            .map(|screen| ModeChange::new(&screen.id, Some(screen.showing(&self.applied))))
            .collect();
        self.change(&changes);
        self.try_showing();
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
                .map(|id| ModeChange::new(id, previous.mode(id)))
                .collect();
            self.change(&restored);
            changes.extend(restored);
            let shown: Vec<(String, DisplayMode)> = self
                .screens
                .iter()
                .map(|screen| (screen.id.clone(), screen.showing(&previous)))
                .collect();
            self.tried.extend(shown);
        }
        let fallen_back: Vec<ModeChange> = self
            .untried()
            .map(|screen| ModeChange {
                here: self.applied.mode(&screen.id).is_some(),
                ..ModeChange::new(&screen.id, Some(screen.preferred))
            })
            .collect();
        self.change(&fallen_back);
        changes.extend(fallen_back);
        self.try_showing();
        changes
    }

    fn untried(&self) -> impl Iterator<Item = &Screen> {
        self.screens
            .iter()
            .filter(|screen| screen.untried(&self.applied, &self.tried))
    }

    fn try_showing(&mut self) {
        let shown: Vec<(String, DisplayMode)> = self
            .screens
            .iter()
            .map(|screen| (screen.id.clone(), screen.showing(&self.applied)))
            .collect();
        self.tried.extend(shown);
    }

    fn change(&mut self, changes: &[ModeChange]) {
        for change in changes {
            let edit = self.applied.set_mode(&change.id, change.mode);
            self.applied = edited(&self.applied, &[edit]);
        }
    }
}

pub(crate) fn written(
    layers: &LayerContents<DisplaySettings>,
    changes: &[ModeChange],
) -> Vec<(SettingsLayer, Edit)> {
    let mut layers = layers.clone();
    changes
        .iter()
        .map(|change| {
            let setting = DisplaySettings::mode_setting(&change.id);
            let layer = match change.here {
                true => SettingsLayer::ThisDevice,
                false => setting.layer(&layers.own(SettingsLayer::ThisDevice)),
            };
            let own = layers.own(layer);
            let edit = setting.set(&own, change.mode);
            layers.set(layer, edited(&own, std::slice::from_ref(&edit)));
            (layer, edit)
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
