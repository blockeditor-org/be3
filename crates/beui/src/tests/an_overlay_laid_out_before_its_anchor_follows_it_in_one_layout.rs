use super::*;
use crate::reactive::{Frame, NodeRef, build, create_signal, view, with_reactive_scope};
use crate::unstyled::{Edge, Floating};

#[test]
fn an_overlay_laid_out_before_its_anchor_follows_it_in_one_layout() {
    let _root = crate::reactive::RootScope::new();
    let (banner, set_banner) = create_signal(0.0_f32);
    let stage = NodeRef::new();
    let pill = NodeRef::new();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Floating anchor={stage} edge=Edge::TopEnd>
                    <Frame width=40.0 height=20.0 @node_ref={&pill} />
                </Floating>
                <Frame height={banner} />
                <Frame height=100.0 @node_ref={&stage} />
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_reactive_scope(harness.document_mut(), move || set_banner.set(50.0));
    harness.frame(Vec::new());
    let stage = harness.rect(stage.get());
    assert_eq!(stage.top(), 50.0, "the banner pushes the stage down");
    assert_eq!(
        harness.rect(pill.get()).top(),
        stage.top(),
        "the pill stays on the stage's top edge"
    );
    assert_eq!(
        harness.document().performance().latest.layout_passes,
        1,
        "the overlay is placed after its anchor within the one layout"
    );
}
