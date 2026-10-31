use std::cell::Cell;
use std::rc::Rc;

use beui::reactive::*;
use beui::styled::*;
use beui::{BackGesture, Key, KeyAt, KeyPhase, NodeId};
use block_editor_plugin::{DialogAnswer, DialogFrame, EditorHost};

use crate::Artifacts;

#[derive(Clone, Copy)]
pub(crate) struct DialogControl {
    narrow: Signal<bool>,
    inset: Signal<f32>,
    modal: Signal<bool>,
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
        if self.modal.get_untracked() != frame.modal {
            self.modal.set(frame.modal);
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
    let modal = create_signal(false);
    provide_context(Modality(create_memo(move || modal.get())));
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
        modal,
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
    on_back(true, move |gesture| {
        if gesture == BackGesture::Invoked {
            dismiss.call(());
        }
    });
    body
}

#[component]
pub(crate) fn SettingsDialog(artifacts: Artifacts, #[prop(children)] content: Render) -> NodeId {
    let settings = artifacts.settings();
    let saved = artifacts.saved_settings();
    let unchanged =
        create_memo(move || settings.with(|settings| saved.with(|saved| settings == saved)));
    let cancel = artifacts.host().clone();
    let apply = artifacts.host().clone();
    let body = content.call(());
    view! {
        <DialogBody
            title="Dynamic artifact settings"
            id="artifact-settings"
            actions={vec![
                DialogAction::cancel("Cancel"),
                DialogAction::primary("Apply", move || apply.close_dialog(DialogAnswer::Accepted))
                    .disabled(unchanged),
            ]}
            on_dismiss={move || cancel.close_dialog(DialogAnswer::Dismissed)}
        >
            {body}
        </DialogBody>
    }
}
