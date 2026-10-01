use super::*;
use crate::reactive::{Direction, Frame, ItemSize, List, NodeRef, view};

#[test]
fn a_gap_before_one_child_replaces_the_rows_spacing() {
    let (second, third) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (second, third) = (second.clone(), third.clone());
        move || {
            view! {
                <List direction=Direction::Horizontal spacing=4.0>
                    <Frame width=10.0 height=10.0 />
                    <Frame @node_ref=&second width=10.0 height=10.0 />
                    <Frame
                        @sizing=ItemSize::Intrinsic.gap(24.0)
                        @node_ref=&third
                        width=10.0
                        height=10.0
                    />
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 40.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(second.get()).left(), 14.0);
    assert_eq!(harness.rect(third.get()).left(), 48.0);
}
