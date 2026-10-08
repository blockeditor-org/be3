use super::{DockKey, DockState, DockingLayout};

impl<K: DockKey> DockingLayout<K> {
    pub fn switching(&self) -> bool {
        self.inner.state.with(|state| state.switch().is_some())
    }

    pub fn switch_choice(&self) -> Option<K> {
        let tab = self.inner.state.with(DockState::switch)?.chosen;
        self.key_of(tab)
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
