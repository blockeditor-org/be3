use super::*;
use crate::reactive::{Align, Direction, Frame, Layers, List, NodeRef, view};

#[test]
fn layers_stack_their_children_in_one_box_sized_to_the_largest() {
    let (layers, wide, tall) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (layers, wide, tall) = (layers.clone(), wide.clone(), tall.clone());
        move || {
            view! {
                <List direction=Direction::Horizontal align=Align::Start spacing=0.0>
                    <Layers @node_ref=&layers>
                        <Frame @node_ref=&wide width=40.0 height=20.0 />
                        <Frame @node_ref=&tall width=10.0 height=30.0 />
                    </Layers>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(300.0, 200.0));
    harness.frame(Vec::new());
    let expected = Rect::from_min_size(Pos2::ZERO, vec2(40.0, 30.0));
    assert_eq!(harness.rect(layers.get()), expected);
    assert_eq!(harness.rect(wide.get()), expected);
    assert_eq!(harness.rect(tall.get()), expected);
}
