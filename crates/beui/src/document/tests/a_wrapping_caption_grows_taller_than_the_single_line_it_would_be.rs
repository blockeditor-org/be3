use super::*;
use crate::reactive::{Frame, NodeRef, view};
use crate::styled::Caption;

const FRAME_WIDTH: f32 = 90.0;
const LONG_CAPTION: &str = "Scroll to pan, Shift+scroll sideways, Ctrl+scroll or pinch to zoom.";

#[test]
fn a_wrapping_caption_grows_taller_than_the_single_line_it_would_be() {
    assert!(caption_height(true) > caption_height(false));
}

fn caption_height(wrap: bool) -> f32 {
    let frame = NodeRef::new();
    let placed = frame.clone();
    let document = build(move || {
        view! {
            <Column spacing=0.0>
                <Frame @node_ref=&placed width=FRAME_WIDTH>
                    <Caption content=LONG_CAPTION wrap />
                </Frame>
            </Column>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness
        .document()
        .node_rect(frame.get())
        .expect("the caption was laid out")
        .height()
}
