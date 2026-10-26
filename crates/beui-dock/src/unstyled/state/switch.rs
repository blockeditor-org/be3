use super::{DockState, TabId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockSwitch {
    pub tabs: Vec<TabId>,
    pub chosen: TabId,
}

impl DockState {
    pub fn switch(&self) -> Option<DockSwitch> {
        let held = self.switch.as_ref()?;
        let tabs: Vec<TabId> = held
            .tabs
            .iter()
            .copied()
            .filter(|tab| self.contains(*tab))
            .collect();
        let chosen = match self.contains(held.chosen) {
            true => held.chosen,
            false => {
                let at = held.tabs.iter().position(|tab| *tab == held.chosen)?;
                held.tabs[at..]
                    .iter()
                    .chain(&held.tabs[..at])
                    .copied()
                    .find(|tab| self.contains(*tab))?
            }
        };
        Some(DockSwitch { tabs, chosen })
    }

    pub fn begin_switch(&mut self, backwards: bool) {
        let tabs = self.recent_tabs();
        let Some(first) = tabs.first().copied() else {
            self.switch = None;
            return;
        };
        self.switch = Some(DockSwitch {
            tabs,
            chosen: first,
        });
        self.step_switch(backwards);
    }

    pub fn step_switch(&mut self, backwards: bool) {
        let Some(DockSwitch { tabs, chosen }) = self.switch() else {
            self.begin_switch(backwards);
            return;
        };
        let count = tabs.len();
        let at = tabs.iter().position(|tab| *tab == chosen).unwrap_or(0);
        let next = match backwards {
            true => (at + count - 1) % count,
            false => (at + 1) % count,
        };
        let chosen = tabs[next];
        self.switch = Some(DockSwitch { tabs, chosen });
    }

    pub fn choose_switch(&mut self, tab: TabId) {
        if let Some(switch) = &mut self.switch
            && switch.tabs.contains(&tab)
        {
            switch.chosen = tab;
        }
    }

    pub fn commit_switch(&mut self) {
        let chosen = self.switch().map(|switch| switch.chosen);
        self.switch = None;
        if let Some(chosen) = chosen {
            self.show(chosen);
        }
    }

    pub fn cancel_switch(&mut self) {
        self.switch = None;
    }
}
