use std::collections::HashMap;

use crate::{Handle, RawWindow};

const SHELL_CLASSES: [&str; 4] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

const DIALOG_CLASS: &str = "#32770";

/// The top-level windows by handle, as one read of the desktop gave them.
pub struct Windows<'a> {
    by_handle: HashMap<Handle, &'a RawWindow>,
}

impl<'a> Windows<'a> {
    pub fn new(windows: &'a [RawWindow]) -> Self {
        Self {
            by_handle: windows.iter().map(|window| (window.handle, window)).collect(),
        }
    }

    pub fn get(&self, handle: Handle) -> Option<&'a RawWindow> {
        self.by_handle.get(&handle).copied()
    }

    /// The window's owner, when the owner is a window someone can see. Many
    /// programs own their windows by a hidden window, which counts as none.
    pub fn visible_owner(&self, window: &RawWindow) -> Option<&'a RawWindow> {
        self.get(window.owner?)
            .filter(|owner| owner.visible && !owner.cloaked)
    }

    /// The owners of the window, nearest first, stopping at a loop.
    pub fn owners(&self, window: &RawWindow) -> Vec<&'a RawWindow> {
        let mut owners = Vec::new();
        let mut next = window.owner;
        while let Some(handle) = next {
            let Some(owner) = self.get(handle) else {
                break;
            };
            if owner.handle == window.handle || owners.iter().any(|seen: &&RawWindow| seen.handle == handle) {
                break;
            }
            owners.push(owner);
            next = owner.owner;
        }
        owners
    }
}

/// Whether someone looking at the screen sees the window at all.
pub fn on_screen(window: &RawWindow) -> bool {
    window.visible && !window.cloaked && !window.style.child && (window.minimized || !window.frame.is_empty())
}

pub fn is_dialog(window: &RawWindow) -> bool {
    window.class == DIALOG_CLASS || window.style.dialog_frame
}

/// Whether Alt-Tab would list the window: shown, not cloaked, not a tool
/// window unless it asks to be an app window, and owned by nobody visible
/// unless it is a dialog.
pub fn alt_tab(window: &RawWindow, windows: &Windows) -> bool {
    if !on_screen(window) || SHELL_CLASSES.contains(&window.class.as_str()) {
        return false;
    }
    if window.style.app_window {
        return true;
    }
    if window.style.tool_window || window.style.no_activate {
        return false;
    }
    windows.visible_owner(window).is_none() || is_dialog(window)
}
