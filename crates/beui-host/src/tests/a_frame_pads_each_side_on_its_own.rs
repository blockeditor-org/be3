use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_frame_pads_each_side_on_its_own() {
    let _root = crate::reactive::RootScope::new();
    let (outer, inner) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal align=Align::Start spacing=0.0>
                <Frame
                    @node_ref=&outer
                    padding={Sides::horizontal(5.0).top(1.0).right(3.0).bottom(4.0)}
                >
                    <Frame @node_ref=&inner width=20.0 height=10.0 />
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(300.0, 300.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(inner.get()).min, pos2(5.0, 1.0));
    assert_eq!(harness.rect(outer.get()).size(), vec2(28.0, 15.0));
}
