use super::{DockState, TabId};

const HISTORY_LENGTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DockVisit {
    pub from: TabId,
    pub to: TabId,
    pub opened: bool,
}

impl DockState {
    pub fn history(&self) -> &[DockVisit] {
        &self.history
    }

    pub fn keep_visits(&mut self, recent: Vec<TabId>, history: Vec<DockVisit>) {
        self.recent = recent
            .into_iter()
            .filter(|tab| self.contains(*tab))
            .collect();
        self.history = history
            .into_iter()
            .filter(|visit| self.contains(visit.from) && self.contains(visit.to))
            .collect();
    }

    pub fn back(&self) -> Option<DockVisit> {
        self.back_index().map(|index| self.history[index])
    }

    fn back_index(&self) -> Option<usize> {
        let current = self.stacked_tab()?;
        self.history.iter().rposition(|visit| visit.to == current)
    }

    pub fn go_back(&mut self) -> Option<DockVisit> {
        let index = self.back_index()?;
        let visit = self.history[index];
        self.history.truncate(index);
        self.return_to(visit.from);
        Some(visit)
    }

    pub(super) fn return_to(&mut self, tab: TabId) {
        let kept = self.history.clone();
        self.show(tab);
        self.history = kept;
    }

    pub(super) fn record_visit(&mut self, to: TabId) {
        let Some(from) = self.recent.first().copied() else {
            return;
        };
        let opened = !self.recent.contains(&to);
        self.history.push(DockVisit { from, to, opened });
        if self.history.len() > HISTORY_LENGTH {
            self.history.remove(0);
        }
    }

    pub(super) fn forget_visits(&mut self, tab: TabId) {
        let opener = self
            .history
            .iter()
            .rev()
            .find(|visit| visit.to == tab)
            .map(|visit| visit.from);
        self.history.retain(|visit| visit.to != tab);
        self.history.retain_mut(|visit| {
            if visit.from != tab {
                return true;
            }
            match opener {
                Some(opener) if opener != visit.to => {
                    visit.from = opener;
                    true
                }
                _ => false,
            }
        });
    }
}
