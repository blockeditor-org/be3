use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{TextAreaApi, TextAreaState};
use beui_core::rich::HANDLE_RADIUS;
use std::sync::Arc;
use text_editor_core::TextBuffer;

const SCALE: f32 = 2.0;

#[test]
fn a_long_press_in_a_zoomed_text_area_selects_the_word_and_its_handles_and_toolbar_follow_the_zoom()
{
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let area = NodeRef::new();
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(b"Hello brave world")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <Canvas view={Some(CanvasView::new(pos2(20.0, 30.0), SCALE))}>
                <CanvasItem x=10.0 y=10.0 width=300.0 height=120.0 scales=true>
                    <TextArea @node_ref=&area state={state} />
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::sized(document, vec2(900.0, 600.0));
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    let root = area.get();
    let surface = harness.document().api::<TextAreaSurfaceApi>(root).surface();
    let canvas = harness.rect(state.canvas().get());
    let on_hello = canvas.min + vec2(12.0, 16.0) * SCALE;

    harness.touch(TouchPhase::Start, on_hello);
    harness.advance(Duration::from_secs(1));
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, on_hello);
    harness.frame(Vec::new());
    assert_eq!(
        state.selection_ranges(),
        vec![0..5],
        "the long press selects the word under the finger"
    );

    let handles = harness
        .document()
        .api::<TextAreaApi>(surface)
        .handles(harness.document());
    let [start, end] = handles[..] else {
        panic!("a word selected by touch shows two handles, not {handles:?}");
    };
    let copy = text_within(harness.document(), root, "Copy")
        .map(|copy| harness.rect(copy))
        .expect("the selection shows its toolbar");
    let below = start.y.max(end.y) + HANDLE_RADIUS * SCALE;
    assert!(
        copy.bottom() <= on_hello.y - 8.0 || copy.top() >= below,
        "the toolbar {copy:?} keeps clear of the selection and its handles, \
         which grow with the zoom down to {below}"
    );

    let further = pos2(end.x + 40.0 * SCALE, end.y);
    harness.touch(TouchPhase::Start, end);
    harness.touch(TouchPhase::Move, pos2(end.x + 20.0 * SCALE, end.y));
    harness.touch(TouchPhase::Move, further);
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, further);
    harness.frame(Vec::new());
    let range = state.selection_ranges()[0].clone();
    assert!(
        range.start == 0 && range.end > 5,
        "dragging the zoomed handle grew the selection to {range:?}"
    );
}
