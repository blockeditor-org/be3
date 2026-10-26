use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::ops::Range;
use std::sync::Arc;
use text_editor_core::TextBuffer;

struct Zoomed {
    harness: Harness,
    area: NodeId,
    state: TextAreaState,
    scale: f32,
}

impl Zoomed {
    fn new(origin: Pos2, scale: f32) -> Self {
        let area = NodeRef::new();
        let held = Rc::new(RefCell::new(None));
        let document = build_node({
            let held = Rc::clone(&held);
            move || {
                let text = Arc::new(TextBuffer::new(b"Hello wide world"))
                    as Arc<dyn text_editor_core::Document>;
                let state = TextAreaState::new(text);
                *held.borrow_mut() = Some(state);
                view! {
                    <Canvas view={Some(CanvasView::new(origin, scale))}>
                        <CanvasItem x=10.0 y=10.0 width=200.0 height=60.0 scales=true>
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
            }
        });
        let mut harness = Harness::sized(document, Vec2::new(600.0, 400.0));
        harness.frame(Vec::new());
        let state = held.borrow_mut().take().expect("the text area was built");
        Self {
            harness,
            area: area.get(),
            state,
            scale,
        }
    }

    fn at(&self, local: Vec2) -> Pos2 {
        self.harness.rect(self.area).min + local * self.scale
    }

    fn caret_after_click(&mut self, local: Vec2) -> Vec<usize> {
        let at = self.at(local);
        self.harness.click(at);
        self.state.caret_indices()
    }

    fn selection_after_drag(&mut self, from: Vec2, to: Vec2) -> Vec<Range<usize>> {
        let (from, to) = (self.at(from), self.at(to));
        self.harness.drag(from, to);
        self.state.selection_ranges()
    }
}

#[test]
fn a_press_in_a_zoomed_text_area_places_the_caret_where_it_lands() {
    let _root = crate::reactive::RootScope::new();
    let press = vec2(60.0, 16.0);
    let shown = Zoomed::new(pos2(0.0, 0.0), 1.0).caret_after_click(press);
    assert!(
        shown.first().is_some_and(|at| *at > 0 && *at < 16),
        "the press lands inside the text: {shown:?}"
    );
    let mut zoomed = Zoomed::new(pos2(30.0, 20.0), 2.5);
    assert_eq!(
        zoomed.harness.rect(zoomed.area).width(),
        500.0,
        "the text area is shown at the zoom"
    );
    assert_eq!(
        zoomed.caret_after_click(press),
        shown,
        "a press on the zoomed text puts the caret at the same character"
    );
    let shrunk = Zoomed::new(pos2(-5.0, 4.0), 0.5).caret_after_click(press);
    assert_eq!(shrunk, shown, "and so does one zoomed out");

    let (from, to) = (vec2(30.0, 16.0), vec2(120.0, 16.0));
    let selected = Zoomed::new(pos2(0.0, 0.0), 1.0).selection_after_drag(from, to);
    assert!(
        selected.first().is_some_and(|range| !range.is_empty()),
        "a drag selects: {selected:?}"
    );
    assert_eq!(
        Zoomed::new(pos2(30.0, 20.0), 2.5).selection_after_drag(from, to),
        selected,
        "a drag across the zoomed text selects the same characters"
    );
}
