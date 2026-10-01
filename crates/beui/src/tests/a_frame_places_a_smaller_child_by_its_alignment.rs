use super::*;
use crate::reactive::{Align, Frame, NodeRef, view};

#[test]
fn a_frame_places_a_smaller_child_by_its_alignment() {
    assert_eq!(
        child_rect(Align::Center, Align::Center),
        Rect::from_min_size(pos2(40.0, 25.0), vec2(20.0, 10.0))
    );
    assert_eq!(
        child_rect(Align::End, Align::Start),
        Rect::from_min_size(pos2(80.0, 0.0), vec2(20.0, 10.0))
    );
    assert_eq!(
        child_rect(Align::Start, Align::Stretch),
        Rect::from_min_size(pos2(0.0, 0.0), vec2(20.0, 60.0))
    );
}

fn child_rect(horizontal: Align, vertical: Align) -> Rect {
    let child = NodeRef::new();
    let document = build({
        let child = child.clone();
        move || {
            view! {
                <Frame width=100.0 height=60.0 align_horizontal=horizontal align_vertical=vertical>
                    <Frame @node_ref=&child width=20.0 height=10.0 />
                </Frame>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(300.0, 300.0));
    harness.frame(Vec::new());
    harness.rect(child.get())
}
