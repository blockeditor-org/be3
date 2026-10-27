use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_first_fit_moves_on_when_the_child_it_shows_grows_too_wide() {
    let _root = crate::reactive::RootScope::new();
    let wide_width = create_signal(150.0f32);
    let (wide, narrow) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        let width = create_memo(move || Extent::px(wide_width.get()));
        view! {
            <Frame width=200.0>
                <unstyled::FirstFit>
                    <Frame @node_ref=&wide width height=20.0 fill=Color32::WHITE />
                    <Frame @node_ref=&narrow width=50.0 height=20.0 fill=Color32::WHITE />
                </unstyled::FirstFit>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (wide, narrow) = (wide.get(), narrow.get());
    assert!(harness.document().node_rect(wide).is_some());
    assert!(harness.document().node_rect(narrow).is_none());

    wide_width.set(250.0);
    harness.frame(Vec::new());
    assert!(
        harness.document().node_rect(wide).is_none(),
        "the first child no longer fits once it grows past the room it has"
    );
    assert_eq!(
        harness
            .document()
            .node_rect(narrow)
            .map(|rect| rect.width()),
        Some(200.0),
        "the child that fits is shown across the room"
    );
}
