use smithay::delegate_keyboard_shortcuts_inhibit;
use smithay::reexports::wayland_server::{DisplayHandle, Resource};
use smithay::wayland::keyboard_shortcuts_inhibit::{
    KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState, KeyboardShortcutsInhibitor,
};

use crate::state::{State, WindowId};

pub(crate) struct Shortcuts {
    state: KeyboardShortcutsInhibitState,
    inhibitors: Vec<KeyboardShortcutsInhibitor>,
    restored: Vec<KeyboardShortcutsInhibitor>,
}

impl Shortcuts {
    pub(crate) fn new(handle: &DisplayHandle) -> Self {
        Self {
            state: KeyboardShortcutsInhibitState::new::<State>(handle),
            inhibitors: Vec::new(),
            restored: Vec::new(),
        }
    }
}

impl State {
    fn inhibitor_window(&self, inhibitor: &KeyboardShortcutsInhibitor) -> Option<WindowId> {
        let surface = inhibitor.wl_surface();
        if !surface.is_alive() {
            return None;
        }
        self.window_id_of(surface)
    }

    pub fn inhibits_shortcuts(&self, id: WindowId) -> bool {
        self.shortcuts.inhibitors.iter().any(|inhibitor| {
            !self.shortcuts.restored.contains(inhibitor)
                && self.inhibitor_window(inhibitor) == Some(id)
        })
    }

    pub fn restore_shortcuts(&mut self, id: WindowId) {
        let held: Vec<KeyboardShortcutsInhibitor> = self
            .shortcuts
            .inhibitors
            .iter()
            .filter(|inhibitor| {
                !self.shortcuts.restored.contains(inhibitor)
                    && self.inhibitor_window(inhibitor) == Some(id)
            })
            .cloned()
            .collect();
        if held.is_empty() {
            return;
        }
        self.shortcuts.restored.extend(held);
        self.changed(id);
        self.settle_inhibitors();
    }

    pub(crate) fn settle_inhibitors(&mut self) {
        let focused = self.keyboard_window();
        let lapsed: Vec<KeyboardShortcutsInhibitor> = self
            .shortcuts
            .restored
            .iter()
            .filter(|inhibitor| focused.is_none() || self.inhibitor_window(inhibitor) != focused)
            .cloned()
            .collect();
        for inhibitor in &lapsed {
            if let Some(id) = self.inhibitor_window(inhibitor) {
                self.changed(id);
            }
        }
        self.shortcuts
            .restored
            .retain(|inhibitor| !lapsed.contains(inhibitor));
        for inhibitor in self.shortcuts.inhibitors.clone() {
            let window = self.inhibitor_window(&inhibitor);
            let active = window.is_some()
                && window == focused
                && !self.shortcuts.restored.contains(&inhibitor);
            match (active, inhibitor.is_active()) {
                (true, false) => inhibitor.activate(),
                (false, true) => inhibitor.inactivate(),
                _ => {}
            }
        }
    }
}

impl KeyboardShortcutsInhibitHandler for State {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.shortcuts.state
    }

    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        if let Some(id) = self.inhibitor_window(&inhibitor) {
            self.changed(id);
        }
        self.shortcuts.inhibitors.push(inhibitor);
        self.settle_inhibitors();
    }

    fn inhibitor_destroyed(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        if let Some(id) = self.inhibitor_window(&inhibitor) {
            self.changed(id);
        }
        self.shortcuts.inhibitors.retain(|held| *held != inhibitor);
        self.shortcuts.restored.retain(|held| *held != inhibitor);
    }
}

delegate_keyboard_shortcuts_inhibit!(State);
