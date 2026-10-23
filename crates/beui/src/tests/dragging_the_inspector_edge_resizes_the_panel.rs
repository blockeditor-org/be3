use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn dragging_the_inspector_edge_resizes_the_panel() {
    let _root = crate::reactive::RootScope::new();
    let text = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Text @node_ref=&text color=Color32::WHITE>"Hello"</Text>
        }
    });
    let text = text.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);

    harness.toggle_inspector();
    let width = harness.inspector().width;
    let edge = WIDE_VIEWPORT.x - width;

    harness.drag(pos2(edge, 100.0), pos2(edge - 80.0, 100.0));

    assert_eq!(harness.inspector().width, width + 80.0);
    assert_eq!(
        harness.document().node_rect(text).map(|rect| rect.right()),
        Some(edge - 80.0)
    );
}
