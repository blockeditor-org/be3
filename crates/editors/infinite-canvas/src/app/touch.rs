use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::beui::icons::ICON_POINTER_OFF;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{CanvasTouch, NodeId};
use block_editor_beui::{Editor, LayeredSettings};

#[derive(Clone, Copy)]
pub(crate) struct TouchChoice {
    write: Signal<Option<Callback<CanvasTouch>>>,
}

impl TouchChoice {
    pub(crate) fn new() -> Self {
        Self {
            write: create_signal(None),
        }
    }

    pub(crate) fn action(self) -> Action {
        let chosen = with_document(|document| document.watch_canvas_touch());
        let write = self.write;
        Action::new("canvas.ignore-touch", "Ignore touch", move || {
            let flipped = match chosen.get_untracked() {
                CanvasTouch::On => CanvasTouch::Off,
                CanvasTouch::Off => CanvasTouch::On,
            };
            if let Some(write) = write.get_untracked() {
                write.call(flipped);
            }
        })
        .glyph(ICON_POINTER_OFF)
        .checked(create_memo(move || chosen.get() == CanvasTouch::Off))
        .enabled(create_memo(move || write.with(Option::is_some)))
        .register()
    }
}

#[component]
pub(crate) fn TouchSetting(editor: Editor, choice: TouchChoice) -> NodeId {
    let fallback = move |_: Pending| {
        view! {
            <Frame />
        }
    };
    view! {
        <Layers>
            <Loading fallback>||
                <TouchWriter editor choice />
            </Loading>
        </Layers>
    }
}

#[component]
fn TouchWriter(editor: Editor, choice: TouchChoice) -> NodeId {
    let settings = LayeredSettings::<DisplaySettings>::this_device(editor);
    let ready = settings.ready();
    let set = settings.setter(&DisplaySettings::canvas_touch_setting());
    create_effect(move || {
        let write = ready
            .get()
            .then(|| Callback::new(move |chosen: CanvasTouch| set.call(Some(chosen))));
        choice.write.set_unconditionally(write);
    });
    view! {
        <Frame />
    }
}
