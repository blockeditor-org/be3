use super::{DockState, SurfaceId, TabId};

impl DockState {
    pub fn is_hidden(&self, surface: SurfaceId) -> bool {
        self.hidden.contains(&surface)
    }

    pub fn is_tab_hidden(&self, tab: TabId) -> bool {
        self.find(tab)
            .is_some_and(|found| self.is_hidden(found.surface))
    }

    pub fn shown_windows(&self) -> Vec<SurfaceId> {
        self.windows()
            .into_iter()
            .filter(|surface| !self.is_hidden(*surface))
            .collect()
    }

    /// Keeps the window's place but places none of its tabs (`is_shown` is false), so its dialogs hide
    /// too; the switcher skips it and it is not saved. Showing or focusing any of its tabs shows it.
    pub fn hide_window(&mut self, surface: SurfaceId) {
        if self.window_rect(surface).is_none() || self.is_hidden(surface) {
            return;
        }
        self.hidden.push(surface);
        let tabs = self.surface_tabs(surface);
        if self
            .fullscreen()
            .is_some_and(|fullscreen| tabs.contains(&fullscreen.tab))
        {
            self.set_fullscreen(None);
        }
        if let Some(switch) = &mut self.switch {
            switch.tabs.retain(|tab| !tabs.contains(tab));
        }
        let focused = self.focus.and_then(|leaf| self.surface_of(leaf));
        if focused != Some(surface) {
            return;
        }
        let next = self
            .recent_tabs()
            .into_iter()
            .find(|tab| !self.is_tab_hidden(*tab));
        match next.and_then(|tab| self.find(tab)) {
            Some(found) => {
                self.set_active_index(found.leaf, found.index);
                self.focus(found.leaf);
            }
            None => {
                let main = self.main();
                self.focus = self.leaves(main).first().copied();
            }
        }
    }

    pub fn show_window(&mut self, surface: SurfaceId) {
        if !self.is_hidden(surface) {
            return;
        }
        self.unhide(surface);
        if let Some(leaf) = self.leaves(surface).first().copied() {
            let leaf = self
                .leaves(surface)
                .into_iter()
                .find(|leaf| self.active_tab(*leaf).is_some())
                .unwrap_or(leaf);
            self.focus(leaf);
        }
    }

    pub(super) fn unhide(&mut self, surface: SurfaceId) {
        self.hidden.retain(|hidden| *hidden != surface);
    }
}
