use super::*;
use crate::reactive::{Frame, NodeRef, build, create_signal, view, with_reactive_scope};
use crate::unstyled::{Edge, Floating};

#[test]
fn a_floating_overlay_follows_an_anchor_that_only_moves() {
    let (banner, set_banner) = create_signal(0.0_f32);
    let stage = NodeRef::new();
    let pill = NodeRef::new();
    let stage_ref = stage.clone();
    let pill_ref = pill.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame height={banner} />
                <Frame height=100.0 @node_ref={&stage_ref} />
                <Floating anchor={stage_ref.clone()} edge=Edge::TopEnd>
                    <Frame width=40.0 height=20.0 @node_ref={&pill_ref} />
                </Floating>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(pill.get()).top(),
        harness.rect(stage.get()).top()
    );

    with_reactive_scope(harness.document_mut(), move || set_banner.set(50.0));
    harness.frame(Vec::new());
    let stage = harness.rect(stage.get());
    assert_eq!(stage.top(), 50.0, "the banner pushes the stage down");
    assert_eq!(
        harness.rect(pill.get()).top(),
        stage.top(),
        "the pill stays on the stage's top edge"
    );
}
