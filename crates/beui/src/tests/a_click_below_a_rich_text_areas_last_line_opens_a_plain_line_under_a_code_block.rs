use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::schema::basic::{self, Basic, CODE_BLOCK};
use text_editor_core::{Document, TextBuffer, Value};

const TEXT: &str = "intro\nfn main() {}";

#[test]
fn a_click_below_a_rich_text_areas_last_line_opens_a_plain_line_under_a_code_block() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let seeds = [basic::block(6..TEXT.len(), CODE_BLOCK, Value::text("rust"))];
        let document = Arc::new(TextBuffer::rich(TEXT, &seeds)) as Arc<dyn Document>;
        let state = TextAreaState::with_schema(document, Arc::new(Basic));
        held.update(|held| *held = Some(state));
        view! {
            <List spacing=0.0>
                <TextArea
                    colors={area_colors()}
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
    let last = state
        .layout()
        .get_untracked()
        .caret_rect(TEXT.len())
        .expect("the last line was laid out")
        .translate(harness.rect(state.canvas().get()).min.to_vec2());

    harness.click(pos2(last.min.x - 40.0, last.max.y + 60.0));
    assert_eq!(
        state.caret_indices(),
        vec![TEXT.len() + 1],
        "the caret lands on a new line after the code block"
    );
    harness.type_text("after");
    assert_eq!(state.caret_indices(), vec![TEXT.len() + 6]);
    assert!(
        state.with_core(|core| core.selected().unwrap().block.is_empty()),
        "what is typed there is a plain line"
    );
}
