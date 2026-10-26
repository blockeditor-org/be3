use std::cell::Cell;
use std::rc::Rc;

use beui::reactive::*;
use beui::styled::*;
use beui::{Key, KeyAt, KeyPhase, NodeId};
use block_editor_plugin::{DialogAnswer, DialogFrame, EditorHost};

#[derive(Clone, Copy)]
pub(crate) struct DialogControl {
    narrow: Signal<bool>,
    inset: Signal<f32>,
    dismiss: Callback<()>,
}

impl DialogControl {
    pub(crate) fn frame(&self, frame: Option<DialogFrame>) {
        let frame = frame.unwrap_or_default();
        if self.narrow.get_untracked() != frame.narrow {
            self.narrow.set(frame.narrow);
        }
        if self.inset.get_untracked() != frame.bottom_inset {
            self.inset.set(frame.bottom_inset);
        }
    }

    pub(crate) fn dismiss(&self) {
        self.dismiss.call(());
    }
}

#[component]
pub(crate) fn PluginDialog(
    host: EditorHost,
    control: Rc<Cell<Option<DialogControl>>>,
    #[prop(children)] content: Render,
) -> NodeId {
    let narrow = create_signal(false);
    let inset = create_signal(0.0_f32);
    let wide_or_narrow = create_memo(move || narrow.get());
    provide_context(DialogLayout {
        narrow: wide_or_narrow,
        sheet: wide_or_narrow,
        bottom_inset: create_memo(move || inset.get()),
    });
    let dismissal = DialogDismissal::new();
    provide_context(dismissal);
    let handled = create_memo(move || dismissal.handled());
    let told = host.clone();
    create_effect(move || told.handles_dismiss(handled.get()));
    let dismiss = Callback::new(move |()| {
        if !dismissal.dismiss() {
            host.close_dialog(DialogAnswer::Dismissed);
        }
    });
    control.set(Some(DialogControl {
        narrow,
        inset,
        dismiss,
    }));
    on_key(KeyAt::Root, KeyPhase::Bubble, move |event| {
        let escape = event
            .press()
            .is_some_and(|press| press.key == Key::Escape && press.pressed && !press.repeat);
        if escape {
            dismiss.call(());
        }
        escape
    });
    let body = content.call(());
    view! {
        <BackHandler on_back={move || dismiss.call(())}>{body}</BackHandler>
    }
}
