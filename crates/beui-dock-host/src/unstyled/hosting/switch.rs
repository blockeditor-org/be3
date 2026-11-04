use super::{DockHostLayout, DockState, TabId};
use beui_reactive::Get;

impl DockHostLayout {
    pub fn switching(&self) -> bool {
        self.state.with(|state| state.switch().is_some())
    }

    pub fn switch_choice(&self) -> Option<TabId> {
        Some(self.state.with(DockState::switch)?.chosen)
    }

    pub fn begin_switch(&self, backwards: bool) {
        self.edit(|state| state.begin_switch(backwards));
    }

    pub fn step_switch(&self, backwards: bool) {
        self.edit(|state| state.step_switch(backwards));
    }

    pub fn commit_switch(&self) {
        self.edit(DockState::commit_switch);
    }

    pub fn cancel_switch(&self) {
        self.edit(DockState::cancel_switch);
    }
}
