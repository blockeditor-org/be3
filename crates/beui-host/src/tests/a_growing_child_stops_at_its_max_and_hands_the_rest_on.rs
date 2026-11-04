use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_growing_child_stops_at_its_max_and_hands_the_rest_on() {
    let _root = crate::reactive::RootScope::new();
    let (capped, open) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <ListItem grow=0.5 max=40.0>
                    <Frame @node_ref=&capped height=10.0 />
                </ListItem>
                <ListItem grow=0.5>
                    <Frame @node_ref=&open height=10.0 />
                </ListItem>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 50.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(capped.get()).width(), 40.0);
    assert_eq!(harness.rect(open.get()).width(), 160.0);
}
