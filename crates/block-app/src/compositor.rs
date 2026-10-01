mod editors;
mod region;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use beui::reactive::{ReadSignal, WriteSignal, create_signal, on_cleanup};

use crate::editors::EditorAction;

pub(crate) use editors::{Editors, HeadlessShell, PaneSurface, PresentingSurface, ShellSurface};

thread_local! {
    static LISTENERS: RefCell<HashMap<String, Vec<(u64, WriteSignal<u64>)>>> =
        RefCell::new(HashMap::new());
    static NEXT_LISTENER: Cell<u64> = const { Cell::new(0) };
    static ACTIONS: RefCell<Vec<EditorAction>> = const { RefCell::new(Vec::new()) };
    static ANY: RefCell<Option<(ReadSignal<u64>, WriteSignal<u64>)>> = const { RefCell::new(None) };
    static CHANGED: Cell<bool> = const { Cell::new(false) };
    static PENDING_SHELL: Cell<Option<Option<uuid::Uuid>>> = const { Cell::new(None) };
    static SHELL: RefCell<Option<(ReadSignal<Option<uuid::Uuid>>, WriteSignal<Option<uuid::Uuid>>)>> =
        const { RefCell::new(None) };
}

pub(crate) fn install() {
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
            .map(|(read, _)| read.clone())
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
    let held = SHELL.with(|shell| shell.borrow().clone());
    if let Some((read, write)) = held
        && read.get_untracked() != block
    {
        write.set(block);
    }
}

pub(crate) fn any() -> ReadSignal<u64> {
    ANY.with(|any| {
        any.borrow()
            .as_ref()
            .map(|(read, _)| read.clone())
            .expect("the compositor is installed while the document is built")
    })
}

pub(crate) fn changed() {
    CHANGED.with(|changed| changed.set(true));
    crate::host::request_repaint();
}

pub(crate) fn listen(plugin_id: &str) -> ReadSignal<u64> {
    let (read, write) = create_signal(0u64);
    let id = NEXT_LISTENER.with(|next| {
        next.set(next.get() + 1);
        next.get()
    });
    LISTENERS.with(|listeners| {
        listeners
            .borrow_mut()
            .entry(plugin_id.to_owned())
            .or_default()
            .push((id, write));
    });
    let plugin_id = plugin_id.to_owned();
    on_cleanup(move || {
        LISTENERS.with(|listeners| {
            if let Some(held) = listeners.borrow_mut().get_mut(&plugin_id) {
                held.retain(|(listener, _)| *listener != id);
            }
        });
    });
    read
}

pub(crate) fn notify() {
    apply_shell();
    let changed = crate::plugin_host::take_changed();
    if !changed.is_empty() || CHANGED.with(|flag| flag.replace(false)) {
        if let Some((_, write)) = ANY.with(|any| any.borrow().clone()) {
            write.update(|revision| *revision += 1);
        }
    }
    for plugin_id in changed {
        let writers: Vec<WriteSignal<u64>> = LISTENERS.with(|listeners| {
            listeners
                .borrow()
                .get(&plugin_id)
                .map(|held| held.iter().map(|(_, write)| write.clone()).collect())
                .unwrap_or_default()
        });
        for write in writers {
            write.update(|revision| *revision += 1);
        }
    }
}

pub(crate) fn act(action: EditorAction) {
    ACTIONS.with(|actions| actions.borrow_mut().push(action));
    crate::host::request_repaint();
}

pub(crate) fn take_actions() -> Vec<EditorAction> {
    ACTIONS.with(|actions| std::mem::take(&mut *actions.borrow_mut()))
}
