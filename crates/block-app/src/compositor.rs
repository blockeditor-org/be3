pub(crate) mod desktop;
mod editors;
mod intercept;
pub(crate) mod region;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use beui::reactive::*;

use crate::editors::EditorAction;

pub(crate) use desktop::{DesktopPlace, forget as forget_windows, request as request_windows};
pub(crate) use editors::{
    ConflictSurface, OwnedEditors, PresentingSurface, ShellDisplay, ShellSurface,
};
pub(crate) use region::{ChildView, PluginRegion, RegionEditor};

type Listeners = HashMap<String, Vec<(u64, Signal<u64>)>>;

thread_local! {
    static LISTENERS: RefCell<Listeners> = RefCell::new(HashMap::new());
    static NEXT_LISTENER: Cell<u64> = const { Cell::new(0) };
    static ACTIONS: RefCell<Vec<(Option<uuid::Uuid>, EditorAction)>> = const { RefCell::new(Vec::new()) };
    static ANY: RefCell<Option<Signal<u64>>> = const { RefCell::new(None) };
    static CHANGED: Cell<bool> = const { Cell::new(false) };
    static STALE: Cell<u64> = const { Cell::new(0) };
    static OWED: Cell<bool> = const { Cell::new(false) };
    static PENDING_SHELL: Cell<Option<Option<uuid::Uuid>>> = const { Cell::new(None) };
    static SHELL: RefCell<Option<Signal<Option<uuid::Uuid>>>> = const { RefCell::new(None) };
}

pub(crate) fn install() {
    desktop::install();
    let signal = create_signal(0u64);
    ANY.with(|any| *any.borrow_mut() = Some(signal));
    let shell = create_signal(None);
    SHELL.with(|held| *held.borrow_mut() = Some(shell));
}

pub(crate) fn shell() -> ReadSignal<Option<uuid::Uuid>> {
    SHELL.with(|shell| {
        shell
            .borrow()
            .as_ref()
            .map(|shell| shell.read_only())
            .expect("the compositor is installed while the document is built")
    })
}

pub(crate) fn set_shell(block: Option<uuid::Uuid>) {
    PENDING_SHELL.with(|pending| pending.set(Some(block)));
}

fn apply_shell() {
    let Some(block) = PENDING_SHELL.with(Cell::take) else {
        return;
    };
    let held = SHELL.with(|shell| *shell.borrow());
    if let Some(shell) = held
        && shell.get_untracked() != block
    {
        shell.set(block);
    }
}

pub(crate) fn any() -> ReadSignal<u64> {
    ANY.with(|any| {
        any.borrow()
            .as_ref()
            .map(|any| any.read_only())
            .expect("the compositor is installed while the document is built")
    })
}

pub(crate) fn changed() {
    CHANGED.with(|changed| changed.set(true));
    crate::host::request_repaint();
}

pub(crate) fn listen(plugin_id: &str) -> ReadSignal<u64> {
    let revision = create_signal(0u64);
    let id = NEXT_LISTENER.with(|next| {
        next.set(next.get() + 1);
        next.get()
    });
    LISTENERS.with(|listeners| {
        listeners
            .borrow_mut()
            .entry(plugin_id.to_owned())
            .or_default()
            .push((id, revision));
    });
    let plugin_id = plugin_id.to_owned();
    on_cleanup(move || {
        LISTENERS.with(|listeners| {
            if let Some(held) = listeners.borrow_mut().get_mut(&plugin_id) {
                held.retain(|(listener, _)| *listener != id);
            }
        });
    });
    revision.read_only()
}

pub(crate) fn release_revision_editors(ids: &[uuid::Uuid]) {
    editors::release_revisions(ids);
}

pub(crate) fn notify() {
    crate::be::revisions::collect_released();
    OWED.with(|owed| owed.set(false));
    apply_shell();
    desktop::apply();
    crate::surfaces::apply();
    notify_plugins();
}

pub(crate) fn notify_after_layout() {
    if notify_plugins() {
        OWED.with(|owed| owed.set(true));
        crate::host::request_repaint();
    }
}

pub(crate) fn owes_frame() -> bool {
    OWED.with(Cell::get)
}

fn notify_plugins() -> bool {
    let stale = crate::be::stale_revision();
    if STALE.with(|seen| seen.replace(stale)) != stale {
        changed();
    }
    let changed = crate::plugin_host::take_changed();
    let notified = !changed.is_empty() || CHANGED.with(|flag| flag.replace(false));
    if notified && let Some(any) = ANY.with(|any| *any.borrow()) {
        any.update(|revision| *revision += 1);
    }
    for plugin_id in changed {
        let writers: Vec<Signal<u64>> = LISTENERS.with(|listeners| {
            listeners
                .borrow()
                .get(&plugin_id)
                .map(|held| held.iter().map(|(_, listener)| *listener).collect())
                .unwrap_or_default()
        });
        for write in writers {
            write.update(|revision| *revision += 1);
        }
    }
    notified
}

pub(crate) fn act(from: Option<uuid::Uuid>, action: EditorAction) {
    ACTIONS.with(|actions| actions.borrow_mut().push((from, action)));
    crate::host::request_repaint();
}

pub(crate) fn take_actions() -> Vec<(Option<uuid::Uuid>, EditorAction)> {
    ACTIONS.with(|actions| std::mem::take(&mut *actions.borrow_mut()))
}
