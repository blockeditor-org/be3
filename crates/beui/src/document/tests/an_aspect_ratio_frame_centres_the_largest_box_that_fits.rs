use super::*;
use crate::reactive::{ItemSize, List, build, view};

#[test]
fn an_aspect_ratio_frame_centres_the_largest_box_that_fits() {
    let stage = NodeRef::new();
    let slide = NodeRef::new();
    let stage_ref = stage.clone();
    let slide_ref = slide.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) @node_ref={&stage_ref} aspect_ratio=2.0>
                    <Spacer @node_ref={&slide_ref} />
                </Frame>
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
