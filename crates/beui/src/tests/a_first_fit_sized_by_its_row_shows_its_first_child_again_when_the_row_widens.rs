use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_first_fit_sized_by_its_row_shows_its_first_child_again_when_the_row_widens() {
    let _root = crate::reactive::RootScope::new();
    let (wide, narrow) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <unstyled::FirstFit @sizing={ItemSize::Intrinsic.shrink(1.0)}>
                    <Frame @node_ref=&wide width=300.0 height=20.0 fill=Color32::WHITE />
                    <Frame @node_ref=&narrow width=100.0 height=20.0 fill=Color32::WHITE />
                </unstyled::FirstFit>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 100.0));
    harness.frame(Vec::new());
    let (wide, narrow) = (wide.get(), narrow.get());
    let shown = |harness: &Harness| {
        let document = harness.document();
        (
            document.node_rect(wide).map(|rect| rect.width()),
            document.node_rect(narrow).map(|rect| rect.width()),
        )
    };
    assert_eq!(shown(&harness), (Some(300.0), None));

    harness.resize(vec2(200.0, 100.0));
    harness.frame(Vec::new());
    assert_eq!(
        shown(&harness),
        (None, Some(200.0)),
        "a row too narrow for the first child shows the second across what the row gives"
    );

    harness.resize(vec2(400.0, 100.0));
    harness.frame(Vec::new());
    assert_eq!(
        shown(&harness),
        (Some(300.0), None),
        "the row asks for the first child's width again, so it comes back once there is room"
    );
}
