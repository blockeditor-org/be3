use super::*;
use crate::reactive::{Direction, Frame, ItemSize, List, NodeRef, view};

#[test]
fn a_percent_child_stops_at_its_max_and_hands_the_rest_on() {
    let (capped, open) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (capped, open) = (capped.clone(), open.clone());
        move || {
            view! {
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame
                        @sizing=ItemSize::Percent(50.0).max(40.0)
                        @node_ref=&capped
                        height=10.0
                    />
                    <Frame @sizing=ItemSize::Percent(50.0) @node_ref=&open height=10.0 />
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 50.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(capped.get()).width(), 40.0);
    assert_eq!(harness.rect(open.get()).width(), 160.0);
}
