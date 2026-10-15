use beui_core::geometry::Pos2;
use beui_core::node::NodeId;
use beui_view::reactive::*;

use super::{DockTabControl, LeafId, State, TabId};

impl State {
    pub(super) fn chrome_pressed(&self, leaf: LeafId, pos: Option<Pos2>) {
        let (active, tab) = self
            .state
            .with_untracked(|state| (state.active_index(leaf), state.active_tab(leaf)));
        let switching = pos.is_some_and(|pos| {
            self.tab_rects(leaf)
                .iter()
                .enumerate()
                .any(|(index, rect)| index != active && rect.contains(pos))
        });
        if let Some(tab) = tab.filter(|_| !switching) {
            self.give_keyboard(tab);
        }
    }

    pub(super) fn give_keyboard(&self, tab: TabId) {
        let given = match self.dialog_focus.get_untracked() {
            Some(dialog) if self.dialog_root(dialog) == Some(tab) => {
                self.focus_into_dialog();
                dialog
            }
            _ => {
                self.focus_panel(tab);
                tab
            }
        };
        self.set_given.update(|(count, target)| {
            *count += 1;
            *target = Some(given);
        });
    }

    pub(super) fn remember_focus(&self, tab: TabId) -> bool {
        let Some(panel) = self.panel_node(tab) else {
            return false;
        };
        let focused = with_document(|document| {
            document
                .focused_node()
                .filter(|_| document.focus_is_within(panel))
        });
        let Some(focused) = focused else {
            return false;
        };
        self.remembered.get().borrow_mut().insert(tab, focused);
        true
    }

    fn panel_node(&self, tab: TabId) -> Option<NodeId> {
        self.panels
            .get()
            .borrow()
            .get(&tab)
            .map(|(panel, _)| *panel)
    }

    fn focus_panel(&self, tab: TabId) {
        let Some(panel) = self.panel_node(tab) else {
            return;
        };
        let held: Vec<TabId> = self.panels.get().borrow().keys().copied().collect();
        for other in held.into_iter().filter(|other| *other != tab) {
            if self.remember_focus(other) {
                break;
            }
        }
        let remembered = self.remembered.get().borrow().get(&tab).copied();
        with_document(|document| {
            if document.focus_is_within(panel) {
                return;
            }
            let remembered =
                remembered.filter(|node| document.focusables_within(panel).contains(node));
            match remembered {
                Some(node) => document.update_focus(Some(node)),
                None => document.focus_into(panel),
            }
        });
    }
}

impl DockTabControl {
    pub fn keyboard(&self) -> Option<u64> {
        let dock = self.dock()?;
        dock.given
            .with(|(count, target)| (*target == Some(self.tab)).then_some(*count))
    }
}
