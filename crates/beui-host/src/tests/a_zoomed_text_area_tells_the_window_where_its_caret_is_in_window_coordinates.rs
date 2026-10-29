use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

fn caret_at(origin: Pos2, scale: f32) -> (Rect, Rect) {
    let area = NodeRef::new();
    let document = build_node(move || {
        let text = Arc::new(TextBuffer::new(b"Hello there")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(text);
        view! {
            <Canvas view={Some(CanvasView::new(origin, scale))}>
                <CanvasItem x=10.0 y=10.0 width=160.0 height=40.0 scales=true>
                    <TextArea
                        colors={area_colors()}
                        look={area_look()}
                        font_size=AREA_FONT_SIZE
                        padding=AREA_PADDING
                        @node_ref=&area
                        state
                        line_numbers=false
                    />
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::sized(document, vec2(600.0, 300.0));
    harness.frame(Vec::new());
    let rect = harness.rect(area.get());
    harness.click(rect.min + vec2(40.0, 16.0) * scale);
    harness.frame(Vec::new());
    let ime = harness
        .document()
        .focused_ime_area()
        .expect("a focused text area asks for the keyboard");
    (rect, ime.cursor)
}

#[test]
fn a_zoomed_text_area_tells_the_window_where_its_caret_is_in_window_coordinates() {
    let _root = crate::reactive::RootScope::new();
    let (area, caret) = caret_at(pos2(0.0, 0.0), 1.0);
    let (zoomed_area, zoomed) = caret_at(pos2(25.0, 15.0), 2.5);
    let expected = Rect::from_min_size(
        zoomed_area.min + (caret.min - area.min) * 2.5,
        caret.size() * 2.5,
    );
    assert!(
        (zoomed.min - expected.min).length() < 0.5 && (zoomed.max - expected.max).length() < 0.5,
        "the caret {zoomed:?} is where the zoom shows it, {expected:?}"
    );
}
