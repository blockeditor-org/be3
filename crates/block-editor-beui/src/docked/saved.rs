use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use be_block::DisplaySettings;
use beui::dock::unstyled::{DockHostLink, DockRequest, DockState, TabId};
use beui::reactive::*;
use block_plugin_api as api;
use serde::{Deserialize, Serialize};

use super::{ProtocolLink, layout_to_plain};
use crate::{Editor, LayeredSettings};

#[derive(Serialize, Deserialize)]
struct SavedDock {
    layout: api::DockLayout,
    keys: Vec<(u64, String)>,
}

#[derive(Default)]
pub(super) struct Saving {
    seen: RefCell<Option<String>>,
    stale: RefCell<Option<Option<String>>>,
}

pub(super) fn dock_layout_key(block_type: uuid::Uuid) -> String {
    format!("{}/frame", block_type.simple())
}

pub(super) fn keep_layout(link: &Rc<ProtocolLink>, editor: Editor) {
    let Some(block_type) = link.host.block_type() else {
        return;
    };
    let setting = DisplaySettings::dock_layout_setting(&dock_layout_key(block_type));
    let settings = LayeredSettings::<DisplaySettings>::this_device(editor);
    let ready = settings.ready();
    let saved = settings.value(&setting);
    let declares = link.declares.read_only();
    let pushed = link.pushed;
    let declared = create_memo(move || declares.get() > 0);
    let following: Weak<ProtocolLink> = Rc::downgrade(link);
    create_effect(move || {
        if !ready.get() || !declared.get() {
            return;
        }
        let saved = saved.get();
        if let Some(link) = following.upgrade() {
            untrack(|| link.follow_saved(saved));
        }
    });
    let writing: Weak<ProtocolLink> = Rc::downgrade(link);
    create_effect(move || {
        pushed.get();
        declares.get();
        if !ready.get() {
            return;
        }
        let Some(link) = writing.upgrade() else {
            return;
        };
        if let Some(layout) = untrack(|| link.changed_layout()) {
            untrack(|| settings.set(&setting, Some(layout)));
        }
    });
}

impl ProtocolLink {
    fn keys(&self) -> Vec<(u64, String)> {
        let declared = self.declared.borrow();
        let Some(declaration) = declared.as_ref() else {
            return Vec::new();
        };
        let mut keys: Vec<(u64, String)> = declaration
            .tabs
            .iter()
            .map(|tab| (tab.tab.value(), tab.key.clone()))
            .chain(
                declaration
                    .bars
                    .iter()
                    .map(|bar| (bar.tab.value(), bar.key.clone())),
            )
            .collect();
        keys.sort();
        keys
    }

    fn encoded(&self) -> Option<String> {
        let hosted = self.host.dock_hosted(self.view)?;
        let mut layout = hosted.layout;
        layout.tree.focus = None;
        serde_json::to_string(&SavedDock {
            layout,
            keys: self.keys(),
        })
        .ok()
    }

    fn follow_saved(&self, saved: Option<String>) {
        if *self.saving.seen.borrow() == saved {
            return;
        }
        self.saving.seen.replace(saved.clone());
        let Some(saved) = saved else {
            return;
        };
        let Ok(saved) = serde_json::from_str::<SavedDock>(&saved) else {
            return;
        };
        let current: HashMap<String, TabId> = self
            .keys()
            .into_iter()
            .map(|(tab, key)| (key, TabId::new(tab)))
            .collect();
        let old: HashMap<u64, String> = saved.keys.into_iter().collect();
        let plain = layout_to_plain(&saved.layout, &mut |path| {
            let key = old.get(&path.tab).filter(|_| path.is_own())?;
            current.get(key).copied()
        });
        self.saving.stale.replace(Some(self.encoded()));
        self.request(DockRequest::Restore(DockState::from_plain(&plain)));
    }

    fn changed_layout(&self) -> Option<String> {
        let encoded = self.encoded()?;
        let stale = self.saving.stale.borrow().clone();
        match stale {
            Some(Some(stale)) if stale == encoded => return None,
            Some(None) => {
                self.saving.stale.replace(Some(Some(encoded)));
                return None;
            }
            Some(Some(_)) => {
                self.saving.stale.replace(None);
            }
            None => {}
        }
        if self.saving.seen.borrow().as_ref() == Some(&encoded) {
            return None;
        }
        self.saving.seen.replace(Some(encoded.clone()));
        Some(encoded)
    }
}
