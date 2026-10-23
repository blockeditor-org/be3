use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_frame_keeps_its_size_between_its_min_and_max() {
    let _root = crate::reactive::RootScope::new();
    let (widened, capped) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal align=Align::Start spacing=0.0>
                <Frame @node_ref=&widened width={Extent::at_least(80.0)}>
                    <Frame width=20.0 height=10.0 />
                </Frame>
                <Frame @node_ref=&capped width=30.0 height={Extent::at_most(25.0)} />
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(300.0, 200.0));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(widened.get()).size(), vec2(80.0, 10.0));
    assert_eq!(harness.rect(capped.get()).size(), vec2(30.0, 25.0));
}
