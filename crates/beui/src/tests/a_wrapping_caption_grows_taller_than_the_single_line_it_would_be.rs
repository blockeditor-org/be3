use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::Text;
use crate::styled::*;
use beui::Overflow;

const FRAME_WIDTH: f32 = 90.0;
const LONG_CAPTION: &str = "Scroll to pan, Shift+scroll sideways, Ctrl+scroll or pinch to zoom.";

#[test]
fn a_wrapping_caption_grows_taller_than_the_single_line_it_would_be() {
    let _root = crate::reactive::RootScope::new();
    assert!(caption_height(Overflow::default()) > caption_height(Overflow::Visible));
}

fn caption_height(overflow: Overflow) -> f32 {
    let frame = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @node_ref=&frame width=FRAME_WIDTH>
                    <Text style=TextStyle::CAPTION overflow>{LONG_CAPTION}</Text>
                </Frame>
            </List>
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
