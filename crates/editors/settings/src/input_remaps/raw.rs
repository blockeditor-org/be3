use std::cell::RefCell;
use std::sync::Arc;

use be_remap::text;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{ItemSize, NodeId};
use text_editor_core::{Document, TextBuffer, TextLanguage};

use super::ui::Shared;

const ROW_SPACING: Space = Space::Md;
const TEXT_HEIGHT: f32 = 360.0;
const SIDE_INSET: Space = Space::Xl2;
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
pub(super) fn TomlTab(shared: Shared) -> NodeId {
    let stored = shared.stored;
    let initial = stored.get_untracked();
    let buffer = TextBuffer::new(&initial);
    buffer.set_language(TextLanguage::Toml);
    let document = store(Arc::new(buffer));
    let loaded = store(RefCell::new(initial));
    let state =
        TextAreaState::new(document.with(|document| Arc::clone(document) as Arc<dyn Document>));
    let error = create_signal(None::<String>);
    let reload = move |text: String| {
        document.with(|document| replace(document, &text));
        state.external_edit();
        loaded.with(|loaded| *loaded.borrow_mut() = text);
        error.set(None);
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
                shared.store(&typed);
                loaded.with(|loaded| *loaded.borrow_mut() = typed);
                error.set(None);
            }
            Err(problem) => error.set(Some(problem)),
        }
    };
    let revert = move || reload(stored.get_untracked());
    let failed = create_memo(move || error.get().is_some());
    let problem = create_memo(move || error.get().unwrap_or_default());
    view! {
        <Frame padding={Padding::horizontal(SIDE_INSET)}>
            <List spacing=ROW_SPACING>
                <TextArea
                    @sizing=ItemSize::Fixed(TEXT_HEIGHT)
                    @test_id={"input-remaps.text"}
                    state={state}
                    placeholder={EXAMPLE.to_owned()}
                    emoji=false
                />
                <Show condition={failed}>
                    <Text style={TextStyle::CAPTION.color(TextColor::Danger)}>{problem}</Text>
                </Show>
                <List direction=Direction::Horizontal spacing=ROW_SPACING>
                    <Button
                        label="Apply"
                        variant=ButtonVariant::Primary
                        disabled={shared.read_only}
                        @test_id={"input-remaps.apply"}
                        on_click={apply}
                    />
                    <Button
                        label="Revert"
                        variant=ButtonVariant::Secondary
                        @test_id={"input-remaps.revert"}
                        on_click={revert}
                    />
                </List>
            </List>
        </Frame>
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
