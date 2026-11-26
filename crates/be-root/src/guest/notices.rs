//! Its host's notices as toasts, and the slow save's toast.

use super::*;

/// A notice of its host's as a toast: one that says something failed is an alert, which stays
/// until it is answered.
pub(super) fn toast(notice: Notice) -> Toast {
    Toast {
        id: notice.id,
        message: notice.message,
        danger: notice.failure,
        actions: notice
            .actions
            .into_iter()
            .map(|action| ToastAction {
                key: action.key,
                label: action.label,
            })
            .collect(),
        ..Toast::default()
    }
}

/// Tells its host how the person answered its notice `id`: with the key of one of its
/// actions, or none to dismiss it.
pub(super) fn answer(id: u64, action: Option<String>) {
    be_wasm_ops::guest::spawn(async move {
        // A host that cannot hear it has stopped, which the root hears from its frames.
        let _ = host::answer_notice(id, action).await;
    });
}

/// The ids of the root's own toasts start here, past any its host numbers its notices with,
/// below `SLOW_SAVE_TOAST`.
const OWN_TOASTS: u64 = 1 << 63;

/// The key of a toast's action that copies its report.
const COPY_REPORT: &str = "copy-report";

/// A toast of the root's own saying the app `name` stopped for `why`, as be-host says one of
/// its own did: its first line, and Copy report, which copies all of it. Each is numbered
/// after those in `own`.
pub(super) fn stopped_toast(own: &[Toast], name: &str, why: &str) -> Toast {
    let first = why.lines().next().unwrap_or_default();
    let id = own.iter().map(|toast| toast.id + 1).max().unwrap_or(OWN_TOASTS);
    Toast {
        id,
        message: format!("{name} stopped: {first}"),
        danger: true,
        actions: vec![ToastAction {
            key: format!("{COPY_REPORT}:{why}"),
            label: "Copy report".to_owned(),
        }],
        ..Toast::default()
    }
}

/// Its host's notices as toasts at the top right of `area`, its whole surface, each gone at once from here
/// when dismissed, as its host lets go of it; and its `own`, which say an app it runs stopped,
/// gone when dismissed.
#[component]
pub(super) fn Notices(
    toasts: Signal<Vec<Toast>>,
    own: Signal<Vec<Toast>>,
    saving: Signal<SavingState>,
    area: Signal<Rect>,
) -> NodeId {
    let slow = slow_save(saving);
    let shown = create_memo(move || {
        let mut shown = toasts.get();
        shown.extend(own.get());
        if slow.shown.get() {
            shown.insert(
                0,
                Toast {
                    id: SLOW_SAVE_TOAST,
                    title: SLOW_SAVE_TITLE.to_owned(),
                    message:
                        "This is taking longer than usual. Keep be-host open until it finishes."
                            .to_owned(),
                    sticky: true,
                    ..Toast::default()
                },
            );
        }
        shown
    });
    view! {
        <Toasts
            anchor={move || OverlayAnchor::Rect(area.get())}
            edge=Edge::TopEnd
            toasts={shown}
            on_dismiss={move |id: u64| {
                if id == SLOW_SAVE_TOAST {
                    slow.dismissed.set(true);
                    return;
                }
                if id >= OWN_TOASTS {
                    own.update(|own| own.retain(|toast| toast.id != id));
                    return;
                }
                toasts.update(|toasts| toasts.retain(|toast| toast.id != id));
                answer(id, None);
            }}
            on_action={move |(id, key): (u64, String)| {
                if id < OWN_TOASTS {
                    answer(id, Some(key));
                    return;
                }
                if let Some(report) = key.strip_prefix(COPY_REPORT).and_then(|key| key.strip_prefix(':')) {
                    let item = be_lib_ui_types::ClipItem::text(report.to_owned());
                    be_wasm_ops::guest::spawn(async move {
                        // A host that cannot hear it has stopped, which the root hears from its
                        // frames.
                        let _ = host::copy(vec![be_wasm_ops::encode(&item)]).await;
                    });
                }
            }}
        />
    }
}

/// The slow save's toast: shown once changes have stayed unsaved for `SLOW_SAVE` by the host's
/// frame clock, a save finishing starting the wait afresh, until they are saved or the person
/// dismisses it.
#[derive(Clone, Copy)]
pub(super) struct SlowSave {
    shown: Memo<bool>,
    dismissed: Signal<bool>,
}

pub(super) fn slow_save(saving: Signal<SavingState>) -> SlowSave {
    let saved = create_memo(move || !saving.with(|saving| saving.unsaved));
    let saves = create_memo(move || saving.with(|saving| saving.saves));
    let slow = create_signal(false);
    let dismissed = create_signal(false);
    let waiting = create_timer(move || {
        slow.set(true);
        None
    });
    create_effect(move || {
        saves.get();
        match saved.get() {
            true => untrack(|| {
                waiting.stop();
                slow.set(false);
                dismissed.set(false);
            }),
            false => untrack(|| {
                if !slow.get() {
                    waiting.restart(SLOW_SAVE);
                }
            }),
        }
    });
    let shown = create_memo(move || slow.get() && !dismissed.get());
    SlowSave { shown, dismissed }
}
