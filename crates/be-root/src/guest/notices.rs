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

/// Its host's notices as toasts at the top right of `area`, its whole surface, each gone at once from here
/// when dismissed, as its host lets go of it.
#[component]
pub(super) fn Notices(
    toasts: Signal<Vec<Toast>>,
    saving: Signal<SavingState>,
    area: Signal<Rect>,
) -> NodeId {
    let slow = slow_save(saving);
    let shown = create_memo(move || {
        let mut shown = toasts.get();
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
                toasts.update(|toasts| toasts.retain(|toast| toast.id != id));
                answer(id, None);
            }}
            on_action={move |(id, key): (u64, String)| answer(id, Some(key))}
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
