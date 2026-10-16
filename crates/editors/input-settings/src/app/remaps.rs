use std::cell::RefCell;
use std::sync::Arc;

use block_editor_beui::be_block::InputSettings;
use block_editor_beui::be_block::be_remap::text;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::TextAreaState;
use block_editor_beui::beui::{ItemSize, NodeId};
use block_editor_beui::{Editor, RemapRequest, RemapState, Remaps};
use text_editor_core::{Document, TextBuffer, TextLanguage};

use super::ui::Settings;

const ROW_SPACING: f32 = 8.0;
const TEXT_HEIGHT: f32 = 220.0;
const EXAMPLE: &str = r#"[[group]]
name = "Everywhere"
rules = [
  { on = "F20+WheelUp", send = "VolumeUp" },
  { on = "Pause", send = "PlayPause" },
]

[[group]]
name = "Celeste"
window = { exe = "Celeste.exe" }
rules = [
  { on = "MouseLeft", send = "J" },
  { on = "WheelUp", tap = "1", hold_ms = 20 },
]
"#;

#[component]
pub(super) fn RemapSettings(
    editor: Editor,
    settings: Settings,
    root: ReadSignal<InputSettings>,
    read_only: Memo<bool>,
) -> NodeId {
    let device = editor.host_value::<Remaps>();
    let offered = editor.host_offers::<Remaps>();
    let stored = create_memo(move || root.get().remaps.unwrap_or_default());
    let initial = stored.get_untracked();
    let buffer = TextBuffer::new(&initial);
    buffer.set_language(TextLanguage::Toml);
    let document = store(Arc::new(buffer));
    let loaded = store(RefCell::new(initial));
    let state =
        TextAreaState::new(document.with(|document| Arc::clone(document) as Arc<dyn Document>));
    let (error, set_error) = create_signal(None::<String>);
    let reload = move |text: String| {
        document.with(|document| replace(document, &text));
        state.external_edit();
        loaded.with(|loaded| *loaded.borrow_mut() = text);
        set_error.set(None);
    };
    create_effect(move || {
        let text = stored.get();
        untrack(|| {
            let unedited = loaded
                .with(|loaded| document.with(|document| text_of(document) == *loaded.borrow()));
            if unedited {
                reload(text);
            }
        });
    });
    let apply = move || {
        let typed = document.with(|document| text_of(document));
        match text::parse(&typed) {
            Ok(_) => {
                settings.operate(InputSettings::set_remaps(Some(&typed)));
                loaded.with(|loaded| *loaded.borrow_mut() = typed);
                set_error.set(None);
            }
            Err(problem) => set_error.set(Some(problem)),
        }
    };
    let revert = move || reload(stored.get_untracked());
    let failed = create_memo(move || error.get().is_some());
    let problem = create_memo(move || error.get().unwrap_or_default());
    let status = create_memo(move || {
        match device.get() {
            RemapState::Off => "No remaps run on this device.",
            RemapState::On => "These remaps run on this device.",
            RemapState::Waiting => {
                "These remaps wait for this device to turn them on, so they cannot press keys without its say."
            }
            RemapState::Stopped => {
                "The remaps on this device stopped after failing, and start again once they change."
            }
        }
        .to_owned()
    });
    let waiting = create_memo(move || device.get() == RemapState::Waiting);
    view! {
        <List spacing=ROW_SPACING>
            <Text style=TextStyle::HEADING string="Remaps" />
            <Text
                style=TextStyle::CAPTION
                string="Keys, buttons and wheel turns to change in a Linux session, everywhere or in the windows a group names. They are written as TOML, as in the example shown while this is empty, and kept as written."
                wrap=true
            />
            <TextArea
                @sizing=ItemSize::Fixed(TEXT_HEIGHT)
                @test_id={"input-settings.remaps"}
                state={state}
                placeholder={EXAMPLE.to_owned()}
                emoji=false
            />
            <Show condition={failed}>
                <Text
                    style={TextStyle::CAPTION.color(TextColor::Danger)}
                    string={problem}
                    wrap=true
                />
            </Show>
            <List direction=Direction::Horizontal spacing=ROW_SPACING>
                <Button
                    label="Apply"
                    variant=ButtonVariant::Primary
                    disabled={read_only}
                    @test_id={"input-settings.remaps.apply"}
                    on_click={apply}
                />
                <Button
                    label="Revert"
                    variant=ButtonVariant::Secondary
                    @test_id={"input-settings.remaps.revert"}
                    on_click={revert}
                />
            </List>
            <Show condition={offered}>
                <List spacing=ROW_SPACING>
                    <Text style=TextStyle::CAPTION string={status} wrap=true />
                    <Show condition={waiting}>
                        <List direction=Direction::Horizontal spacing=ROW_SPACING>
                            <Button
                                label="Review"
                                variant=ButtonVariant::Secondary
                                @test_id={"input-settings.remaps.review"}
                                on_click={move || editor.act(RemapRequest::Review)}
                            />
                        </List>
                    </Show>
                </List>
            </Show>
        </List>
    }
}

fn text_of(document: &TextBuffer) -> String {
    document
        .read()
        .map(|read| String::from_utf8_lossy(&read.slice(0..read.len())).into_owned())
        .unwrap_or_default()
}

fn replace(document: &TextBuffer, text: &str) {
    document.edit(Vec::new(), &mut |edit| {
        let length = edit.document().len();
        edit.replace(0, length, text.as_bytes());
    });
}
