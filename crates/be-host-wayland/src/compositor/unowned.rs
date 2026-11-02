use std::time::Instant;

use beui::native::Context;

use super::Compositor;
use crate::state::{PING_TIMEOUT, WindowId};
use crate::windows::Command;

#[derive(Default)]
pub(super) struct Unowned {
    unowned: bool,
    pinged: Option<Instant>,
}

impl Compositor {
    pub fn set_owned(&mut self, owned: bool) {
        if self.unowned.unowned != owned {
            return;
        }
        self.unowned = Unowned {
            unowned: !owned,
            pinged: None,
        };
        if owned {
            return;
        }
        let state = &mut self.server.state;
        state.dismiss_popups();
        state.release_buttons();
        if state.pointer_window().is_some() {
            state.pointer_motion(None);
        }
        let had = state.keyboard_window();
        state.focus_keyboard(None);
        if let Some(id) = had {
            self.windows.push(Command::Configure(id));
        }
        self.server.flush();
    }

    pub fn owned(&self) -> bool {
        !self.unowned.unowned
    }

    pub(super) fn keyboard_for(&self, focused: Option<WindowId>) -> Option<WindowId> {
        focused.filter(|_| self.owned())
    }

    pub(super) fn ping_unowned(&mut self, context: &Context) {
        if self.owned() || self.server.state.windows().is_empty() {
            return;
        }
        let now = context.now();
        let due = |pinged: Option<Instant>| pinged.map_or(now, |pinged| pinged + PING_TIMEOUT);
        if due(self.unowned.pinged) <= now {
            self.unowned.pinged = Some(now);
            for id in self.server.state.windows() {
                self.server.state.ping(id, now);
            }
            self.server.flush();
        }
        context.request_repaint_after(due(self.unowned.pinged).saturating_duration_since(now));
    }
}
