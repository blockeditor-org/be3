use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use be_block::DisplaySettings;
use beui::dock::unstyled::{
    DockHostLink, DockKeeper, DockKept, DockOutside, DockOutsidePlace, DockPlain, DockPlainEntry,
    DockPlainNode, DockPlainWindow, DockRequest, DockState, SIDEBAR_WIDTH, TabId,
};
use beui::reactive::*;
use block_editor_plugin::EditorHost;
use block_plugin_api as api;
use serde::{Deserialize, Serialize};

use super::{ProtocolLink, layout_to_plain, own_path, plain_to_layout};
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

trait Kept {
    fn saving(&self) -> &Saving;
    fn layout(&self) -> Option<api::DockLayout>;
    fn keys(&self) -> Vec<(u64, String)>;
    fn restore(&self, plain: DockPlain);
}

pub(super) fn dock_layout_key(block_type: uuid::Uuid) -> String {
    format!("{}/frame", block_type.simple())
}

pub(super) fn keep_joined(link: &Rc<ProtocolLink>, editor: Editor) {
    let declares = link.declares.read_only();
    let declaring = link.declaring.read_only();
    let pushed = link.pushed;
    keep_layout(
        Rc::downgrade(link),
        &link.host,
        editor,
        move || {
            pushed.get();
            declares.get();
        },
        create_memo(move || declaring.get()),
    );
}

pub(super) fn keeper(host: EditorHost, editor: Editor) -> DockKeeper {
    DockKeeper::new(move |kept: DockKept| {
        let standalone = Rc::new(Standalone {
            kept,
            saving: Saving::default(),
        });
        let keys = kept.keys;
        let layout = kept.layout;
        keep_layout(
            Rc::downgrade(&standalone),
            &host,
            editor,
            move || {
                layout.with(|_| ());
                keys.with(|_| ());
            },
            create_memo(move || keys.with(|keys| !keys.is_empty())),
        );
        on_cleanup(move || drop(standalone));
    })
}

fn keep_layout<T: Kept + 'static>(
    kept: Weak<T>,
    host: &EditorHost,
    editor: Editor,
    changed: impl Fn() + 'static,
    declared: Memo<bool>,
) {
    let Some(block_type) = host.block_type() else {
        return;
    };
    let setting = DisplaySettings::dock_layout_setting(&dock_layout_key(block_type));
    let settings = LayeredSettings::<DisplaySettings>::this_device(editor);
    let ready = settings.ready();
    let saved = settings.value(&setting);
    let following = Weak::clone(&kept);
    create_effect(move || {
        if !ready.get() {
            return;
        }
        let Some(kept) = following.upgrade() else {
            return;
        };
        if !declared.get() {
            kept.saving().seen.replace(None);
            return;
        }
        let saved = saved.get();
        untrack(|| follow_saved(&*kept, saved));
    });
    create_effect(move || {
        changed();
        if !ready.get() || !declared.get() {
            return;
        }
        let Some(kept) = kept.upgrade() else {
            return;
        };
        if let Some(layout) = untrack(|| changed_layout(&*kept)) {
            untrack(|| settings.set(&setting, Some(layout)));
        }
    });
}

fn encoded(kept: &impl Kept) -> Option<String> {
    let mut layout = kept.layout()?;
    layout.tree.focus = None;
    serde_json::to_string(&SavedDock {
        layout,
        keys: kept.keys(),
    })
    .ok()
}

fn follow_saved(kept: &impl Kept, saved: Option<String>) {
    let saving = kept.saving();
    if *saving.seen.borrow() == saved {
        return;
    }
    saving.seen.replace(saved.clone());
    let Some(saved) = saved else {
        return;
    };
    let Ok(saved) = serde_json::from_str::<SavedDock>(&saved) else {
        return;
    };
    let current: HashMap<String, TabId> = kept
        .keys()
        .into_iter()
        .map(|(tab, key)| (key, TabId::new(tab)))
        .collect();
    let old: HashMap<u64, String> = saved.keys.into_iter().collect();
    let plain = layout_to_plain(&saved.layout, &mut |path| {
        let key = old.get(&path.tab).filter(|_| path.is_own())?;
        current.get(key).copied()
    });
    saving.stale.replace(Some(encoded(kept)));
    kept.restore(plain);
}

fn changed_layout(kept: &impl Kept) -> Option<String> {
    let saving = kept.saving();
    let encoded = encoded(kept)?;
    let stale = saving.stale.borrow().clone();
    match stale {
        Some(Some(stale)) if stale == encoded => return None,
        Some(None) => {
            saving.stale.replace(Some(Some(encoded)));
            return None;
        }
        Some(Some(_)) => {
            saving.stale.replace(None);
        }
        None => {}
    }
    if saving.seen.borrow().as_ref() == Some(&encoded) {
        return None;
    }
    saving.seen.replace(Some(encoded.clone()));
    Some(encoded)
}

impl Kept for ProtocolLink {
    fn saving(&self) -> &Saving {
        &self.saving
    }

    fn layout(&self) -> Option<api::DockLayout> {
        Some(self.host.dock_hosted(self.view)?.layout)
    }

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

    fn restore(&self, plain: DockPlain) {
        let plain = windows_outside(plain);
        self.request(DockRequest::Restore(DockState::from_plain(&plain)));
    }
}

struct Standalone {
    kept: DockKept,
    saving: Saving,
}

impl Kept for Standalone {
    fn saving(&self) -> &Saving {
        &self.saving
    }

    fn layout(&self) -> Option<api::DockLayout> {
        let plain = self.kept.layout.with_untracked(DockState::plain);
        Some(plain_to_layout(&plain, &own_path))
    }

    fn keys(&self) -> Vec<(u64, String)> {
        self.kept.keys.with_untracked(|keys| {
            keys.iter()
                .map(|(tab, key)| (tab.value(), key.clone()))
                .collect()
        })
    }

    fn restore(&self, plain: DockPlain) {
        let plain = outside_windows(plain);
        self.kept.restore.call(DockState::from_plain(&plain));
    }
}

fn windows_outside(mut plain: DockPlain) -> DockPlain {
    for window in std::mem::take(&mut plain.windows) {
        let mut tabs = Vec::new();
        if let Some(root) = &window.root {
            node_tabs(root, &mut tabs);
        }
        plain
            .outside
            .extend(tabs.into_iter().map(|tab| DockOutside {
                tab,
                place: DockOutsidePlace::Window { rect: window.rect },
            }));
    }
    plain
}

fn outside_windows(mut plain: DockPlain) -> DockPlain {
    let (windows, kept): (Vec<DockOutside>, Vec<DockOutside>) = std::mem::take(&mut plain.outside)
        .into_iter()
        .partition(|outside| matches!(outside.place, DockOutsidePlace::Window { .. }));
    plain.outside = kept;
    for outside in windows {
        let DockOutsidePlace::Window { rect } = outside.place else {
            continue;
        };
        plain.windows.push(DockPlainWindow {
            key: String::new(),
            rect,
            root: Some(DockPlainNode::Pane {
                key: String::new(),
                active: 0,
                vertical: false,
                icons_only: false,
                sidebar: SIDEBAR_WIDTH,
                keep: false,
                area: None,
                bare: false,
                entries: vec![DockPlainEntry::Tab(outside.tab)],
            }),
        });
    }
    plain
}

fn node_tabs(node: &DockPlainNode, tabs: &mut Vec<TabId>) {
    match node {
        DockPlainNode::Split { first, second, .. } => {
            node_tabs(first, tabs);
            node_tabs(second, tabs);
        }
        DockPlainNode::Pane { entries, .. } => {
            for entry in entries {
                match entry {
                    DockPlainEntry::Tab(tab) => tabs.push(*tab),
                    DockPlainEntry::Group { root, .. } => node_tabs(root, tabs),
                }
            }
        }
    }
}
