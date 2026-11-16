//! Where the keyboard goes when a pane's chrome is pressed or a tab is picked: back to the node the
//! tab last had focused, else its first focusable (`Document::focus_into`, which waits a layout for a
//! panel still being built). When a pane stops showing a tab whose panel has the focus, the focus
//! is remembered and the tab shown in its place gets the keyboard. Focus in a view of the tab's
//! content that this document cannot see (a child editor's) counts too, told by
//! `DockTabControl::content_focused`.

use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Pos2;

use super::{DockState, Entry, LeafId, State, TabId};

impl State {
    pub(super) fn chrome_pressed(&self, leaf: LeafId, pos: Option<Pos2>) {
        let (active, tab) = self
            .state
            .with_untracked(|state| (state.active_index(leaf), state.active_tab(leaf)));
        let switching_to = pos.and_then(|pos| {
            self.tab_rects(leaf)
                .iter()
                .enumerate()
                .find(|(index, rect)| *index != active && rect.contains(pos))
                .map(|(index, _)| index)
        });
        let Some(index) = switching_to else {
            if let Some(tab) = tab {
                self.give_keyboard(tab);
            }
            return;
        };
        // A tab pressed to switch to: a tab's own body hands the keyboard on as it switches
        // (`DockShownBody`), but a joined editor's group shows a body built anew, which hands on
        // nothing, so the group's shown tab is given it here.
        let joined = self.state.with_untracked(|state| {
            let entry = state.entries(leaf).get(index).copied()?;
            entry.group()?;
            super::entry_shown_tab(state, entry)
        });
        if let Some(tab) = joined {
            self.give_keyboard(tab);
        }
    }

    pub(super) fn bring_forward_when_shown(&self) {
        let dock = *self;
        let shown = create_memo(move || {
            dock.state
                .with(|state| (state.shows(), state.focused_tab()))
        });
        create_effect_on_change(
            move || shown.get(),
            move |&(_, tab), _| {
                let Some(tab) = tab else {
                    return;
                };
                if dock.state.with(|state| state.in_window(tab)) {
                    dock.forward.update(|(count, target)| {
                        *count += 1;
                        *target = Some(tab);
                    });
                }
            },
        );
    }

    /// A tab's content holds the keyboard only until something else takes it: this document, or
    /// an empty pane's view (`DockPaneControl::focus`, which clears it there). Without this, hiding
    /// that tab later would hand on a keyboard it no longer has.
    pub(super) fn forget_content_keyboard_when_taken(&self) {
        let keyboard = self.keyboard;
        let held = self.content_keyboard;
        create_effect(move || {
            if keyboard.get() {
                held.set(None);
            }
        });
    }

    /// Whether the keyboard is in a tab `entry` holds or stands for, remembering where for the
    /// tab that has it (`remember_focus`).
    pub(super) fn entry_holds_keyboard(&self, state: &DockState, entry: Entry) -> bool {
        let mut tabs = state.entry_tabs(entry);
        tabs.extend(state.standing_tabs(entry));
        tabs.into_iter().any(|tab| self.remember_focus(tab))
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
        self.given.update(|(count, target)| {
            *count += 1;
            *target = Some(given);
        });
    }

    /// Whether the keyboard is in `tab`'s panel, remembering where. That is the focus inside the
    /// panel, or a view of the tab's content that took it last (`DockTabControl::content_focused`)
    /// with nothing taking it since (`forget_content_keyboard_when_taken`).
    pub(super) fn remember_focus(&self, tab: TabId) -> bool {
        let Some(panel) = self.panel_node(tab) else {
            return false;
        };
        let focused = focused_node().filter(|_| panel.focus_is_within());
        let Some(focused) = focused else {
            return self.content_keyboard.get() == Some(tab) && !self.keyboard.get_untracked();
        };
        self.remembered.value().borrow_mut().insert(tab, focused);
        true
    }

    fn panel_node(&self, tab: TabId) -> Option<NodeId> {
        self.panels
            .with_untracked(|panels| panels.get(&tab).copied())
    }

    fn focus_panel(&self, tab: TabId) {
        let Some(panel) = self.panel_node(tab) else {
            return;
        };
        let held: Vec<TabId> = self
            .panels
            .with_untracked(|panels| panels.keys().copied().collect());
        for other in held.into_iter().filter(|other| *other != tab) {
            if self.remember_focus(other) {
                break;
            }
        }
        let remembered = self.remembered.value().borrow().get(&tab).copied();
        if panel.focus_is_within() {
            return;
        }
        let remembered = remembered.filter(|node| panel.focusables_within().contains(node));
        match remembered {
            Some(node) => set_focus(Some(node)),
            None => panel.focus_into(),
        }
    }
}
