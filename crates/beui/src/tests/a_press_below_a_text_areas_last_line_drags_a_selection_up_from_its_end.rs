use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::schema::basic::Basic;
use text_editor_core::{Document, EditorCommand, TextBuffer, TextLanguage};

const TEXT: &str = "first line here\nsecond line here\nthird line here";

fn drag_up_from_below(make: fn() -> TextAreaState) {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let state = make();
        held.update(|held| *held = Some(state));
        view! {
            <List spacing=0.0>
                <TextArea
                    colors={area_colors()}
                    look={area_look()}
                    font_size=AREA_FONT_SIZE
                    padding=AREA_PADDING
                    @sizing=ItemSize::Fixed(300.0)
                    state
                />
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    let canvas = harness.rect(state.canvas().get()).min.to_vec2();
    let caret = |byte: usize| {
        state
            .layout()
            .get_untracked()
            .caret_rect(byte)
            .expect("the byte was laid out")
            .translate(canvas)
    };
    let second = TEXT.find("second").expect("the text has a second line");
    let last = caret(TEXT.len());
    let target = caret(second + 2);

    harness.drag(
        pos2(last.min.x + 40.0, last.max.y + 60.0),
        pos2(target.min.x + 0.25, target.center().y),
    );
    assert_eq!(
        state.selection_ranges(),
        vec![second + 2..TEXT.len()],
        "a drag from below the last line selects from the end up to where it is let go"
    );
}

#[test]
fn a_press_below_a_text_areas_last_line_drags_a_selection_up_from_its_end() {
    drag_up_from_below(|| {
        let document = Arc::new(TextBuffer::new(TEXT.as_bytes())) as Arc<dyn Document>;
        let state = TextAreaState::new(document);
        state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
        state
    });
    drag_up_from_below(|| {
        let document = Arc::new(TextBuffer::rich(TEXT, &[])) as Arc<dyn Document>;
        TextAreaState::with_schema(document, Arc::new(Basic))
    });
}
