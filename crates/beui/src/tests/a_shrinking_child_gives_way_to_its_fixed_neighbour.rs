use super::*;
use crate::reactive::{Direction, Frame, ItemSize, List, NodeRef, view};

#[test]
fn a_shrinking_child_gives_way_to_its_fixed_neighbour() {
    let (label, button) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (label, button) = (label.clone(), button.clone());
        move || {
            view! {
                <List direction=Direction::Horizontal spacing=10.0>
                    <Frame
                        @sizing=ItemSize::Intrinsic.shrink(1.0).min(50.0)
                        @node_ref=&label
                        width=300.0
                        height=20.0
                    />
                    <Frame @node_ref=&button width=60.0 height=20.0 />
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 50.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(label.get()).width(), 130.0);
    assert_eq!(harness.rect(button.get()).left(), 140.0);

    *harness.viewport_mut() = vec2(90.0, 50.0);
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(label.get()).width(),
        50.0,
        "a shrinking child stops at its min"
    );
}
