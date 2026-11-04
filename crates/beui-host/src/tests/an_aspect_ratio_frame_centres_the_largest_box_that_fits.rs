use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_aspect_ratio_frame_centres_the_largest_box_that_fits() {
    let _root = crate::reactive::RootScope::new();
    let stage = NodeRef::new();
    let slide = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Frame @node_ref={&stage} aspect_ratio=2.0>
                        <Frame @node_ref={&slide} />
                    </Frame>
                </ListItem>
            </List>
        }
    });

    let mut harness = Harness::sized(document, Vec2::new(400.0, 300.0));
    harness.frame(Vec::new());

    let stage = harness.document().node_rect(stage.get()).unwrap();
    let slide = harness.document().node_rect(slide.get()).unwrap();
    assert_eq!(stage.size(), Vec2::new(400.0, 300.0));
    assert_eq!(slide.size(), Vec2::new(400.0, 200.0));
    assert_eq!(slide.center(), stage.center());
}
