pub(crate) mod anchored;
pub(crate) mod app_view;
pub(crate) mod desktop;
mod editors;
mod intercept;
mod palette;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use beui::reactive::*;

use be_host_runtime::EditorAction;

pub(crate) use anchored::AnchoredViews;
pub(crate) use app_view::{AppView, ChildView, ViewEditor};
pub(crate) use desktop::DesktopPlace;
pub(crate) use editors::{
    ConflictSurface, OwnedEditors, PanelReport, PresentingSurface, ShellDisplay, ShellSurface,
};
pub(crate) use palette::HostPalette;

type Listeners = HashMap<String, Vec<(u64, Signal<u64>)>>;

thread_local! {
    static LISTENERS: RefCell<Listeners> = RefCell::new(HashMap::new());
    static NEXT_LISTENER: Cell<u64> = const { Cell::new(0) };
    static ACTIONS: RefCell<Vec<(Option<uuid::Uuid>, EditorAction)>> = const { RefCell::new(Vec::new()) };
    static ANY: RefCell<Option<Signal<u64>>> = const { RefCell::new(None) };
    static HEARD: RefCell<Option<be_host_base::host::Mailbox>> = const { RefCell::new(None) };
    static STALE: Cell<u64> = const { Cell::new(0) };
    static SEARCHED: RefCell<Option<Signal<u64>>> = const { RefCell::new(None) };
    static OWED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
    static PENDING_SHELL: Cell<Option<Option<uuid::Uuid>>> = const { Cell::new(None) };
    static SHELL: RefCell<Option<Signal<Option<uuid::Uuid>>>> = const { RefCell::new(None) };
    static PALETTE_ASKS: RefCell<Option<Signal<(u64, Vec<be_host_runtime::PaletteAsk>)>>> =
        const { RefCell::new(None) };
}

pub(crate) fn install() {
    be_host_runtime::draw_palette(true);
    desktop::install();
    anchored::install();
    let signal = create_signal(0u64);
    ANY.with(|any| *any.borrow_mut() = Some(signal));
    let shell = create_signal(None);
    SHELL.with(|held| *held.borrow_mut() = Some(shell));
    let asks = create_signal((0, Vec::new()));
    PALETTE_ASKS.with(|held| *held.borrow_mut() = Some(asks));
    let searched = create_signal(be_host_stack::search_revision());
    SEARCHED.with(|held| *held.borrow_mut() = Some(searched));
    let (mailbox, post) = be_host_base::host::mailbox(heard);
    be_host_runtime::listen(post);
    HEARD.with(|held| *held.borrow_mut() = Some(mailbox));
}

/// Acts on what an instance asks of the host wherever it is shown.
fn heard(heard: be_host_runtime::Heard) {
    match heard.event {
        be_host_runtime::InstanceEvent::Action(action) => {
            act(
                editors::editors().key_of(&heard.app_id, heard.instance),
                action,
            );
        }
        be_host_runtime::InstanceEvent::BlockDrag(drag) => app_view::carry(drag),
        _ => {}
    }
}

/// Moves whenever the block store answers a search again (`be_host_stack::search_revision`).
pub(crate) fn searched() -> ReadSignal<u64> {
    SEARCHED.with(|held| {
        held.borrow()
            .as_ref()
            .map(|searched| searched.read_only())
            .expect("the compositor is installed while the document is built")
    })
}

/// What apps asked of the host's palette (`shell-message.open-palette`) since it last heard, with a
/// count that moves with each batch.
pub(crate) fn palette_asks() -> ReadSignal<(u64, Vec<be_host_runtime::PaletteAsk>)> {
    PALETTE_ASKS.with(|asks| {
        asks.borrow()
            .as_ref()
            .map(|asks| asks.read_only())
            .expect("the compositor is installed while the document is built")
    })
}

fn hand_over_palette_asks() {
    let asked = be_host_runtime::take_palette_asks();
    if asked.is_empty() {
        return;
    }
    if let Some(asks) = PALETTE_ASKS.with(|asks| *asks.borrow()) {
        asks.update(|(count, held)| {
            *count += 1;
            *held = asked;
        });
    }
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

pub(crate) fn listen(app_id: &str) -> ReadSignal<u64> {
    let revision = create_signal(0u64);
    let id = NEXT_LISTENER.with(|next| {
        next.set(next.get() + 1);
        next.get()
    });
    LISTENERS.with(|listeners| {
        listeners
            .borrow_mut()
            .entry(app_id.to_owned())
            .or_default()
            .push((id, revision));
    });
    let app_id = app_id.to_owned();
    on_cleanup(move || {
        LISTENERS.with(|listeners| {
            if let Some(held) = listeners.borrow_mut().get_mut(&app_id) {
                held.retain(|(listener, _)| *listener != id);
            }
        });
    });
    revision.read_only()
}

/// Hands `receive` what the instance `instance` of `app_id` does from now on, until the
/// calling scope is cleaned up.
pub(crate) fn hear(
    app_id: Stored<String>,
    instance: be_sdk_api::EditorInstanceId,
    receive: impl Fn(be_host_runtime::InstanceEvent) + 'static,
) {
    let (mailbox, post) = be_host_base::host::mailbox(move |heard: be_host_runtime::Heard| {
        if app_id.try_with(|app_id| heard.from(app_id, instance)) == Some(true) {
            receive(heard.event);
        }
    });
    be_host_runtime::listen(post);
    on_cleanup(move || drop(mailbox));
}

pub(crate) fn notify() {
    be_host_stack::revisions::collect_released(editors::release_revisions);
    OWED.with(|owed| owed.borrow_mut().take());
    apply_shell();
    desktop::apply();
    crate::surfaces::apply();
    hand_over_palette_asks();
    notify_apps();
}

pub(crate) fn notify_after_layout() {
    if let Some(changed) = notify_apps() {
        OWED.with(|owed| *owed.borrow_mut() = Some(changed));
        be_host_base::host::request_repaint();
    }
}

pub(crate) fn owes_frame() -> Option<String> {
    let changed = OWED.with(|owed| owed.borrow().clone())?;
    Some(match changed.is_empty() {
        true => "the compositor owes a frame for a change after layout".to_owned(),
        false => format!(
            "the compositor owes a frame for a change after layout from {}",
            changed.join(", ")
        ),
    })
}

fn notify_apps() -> Option<Vec<String>> {
    let searched = be_host_stack::search_revision();
    if let Some(signal) = SEARCHED.with(|held| *held.borrow())
        && signal.get_untracked() != searched
    {
        signal.set(searched);
    }
    let stale = be_host_stack::stale_revision();
    if STALE.with(|seen| seen.replace(stale)) != stale {
        be_host_base::host::changed();
    }
    let mut changed = be_host_runtime::take_changed();
    changed.sort();
    let notified = !changed.is_empty() || be_host_base::host::take_changed();
    if notified && let Some(any) = ANY.with(|any| *any.borrow()) {
        any.update(|revision| *revision += 1);
    }
    for app_id in &changed {
        let writers: Vec<Signal<u64>> = LISTENERS.with(|listeners| {
            listeners
                .borrow()
                .get(app_id)
                .map(|held| held.iter().map(|(_, listener)| *listener).collect())
                .unwrap_or_default()
        });
        for write in writers {
            write.update(|revision| *revision += 1);
        }
    }
    notified.then_some(changed)
}

pub(crate) fn act(from: Option<uuid::Uuid>, action: EditorAction) {
    ACTIONS.with(|actions| actions.borrow_mut().push((from, action)));
    be_host_base::host::request_repaint();
}

pub(crate) fn take_actions() -> Vec<(Option<uuid::Uuid>, EditorAction)> {
    ACTIONS.with(|actions| std::mem::take(&mut *actions.borrow_mut()))
}
