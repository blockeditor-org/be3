use accesskit::{Action, ActionData, ActionRequest, Point, Role, TreeId};

use super::*;
use crate::base::Direction;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_screen_reader_reads_and_scrolls_a_horizontal_scroll_sideways() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @test_id="strip" direction=Direction::Horizontal>
                    <ForEach keys={indices(40)}>|_: usize|
                        <Frame width=120.0 height=40.0 />
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    let output = harness.frame(Vec::new());
    let strip = harness.find("strip");
    let tree = output.accessibility_tree("Test", VIEWPORT);
    let (id, view) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::ScrollView)
        .expect("the scroll is absent from the accessibility tree");
    let id = *id;
    assert_eq!(view.scroll_x(), Some(0.0));
    assert_eq!(view.scroll_x_max(), Some(4500.0));
    assert_eq!(view.scroll_y(), None);
    assert!(view.supports_action(Action::ScrollRight));
    assert!(view.supports_action(Action::ScrollLeft));
    assert!(!view.supports_action(Action::ScrollDown));

    harness.context().accessibility_action(ActionRequest {
        action: Action::ScrollRight,
        target_tree: TreeId::ROOT,
        target_node: id,
        data: None,
    });
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 300.0);

    harness.context().accessibility_action(ActionRequest {
        action: Action::SetScrollOffset,
        target_tree: TreeId::ROOT,
        target_node: id,
        data: Some(ActionData::SetScrollOffset(Point::new(1000.0, 0.0))),
    });
    let output = harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(strip), 1000.0);
    let tree = output.accessibility_tree("Test", VIEWPORT);
    let view = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::ScrollView)
        .map(|(_, node)| node)
        .unwrap();
    assert_eq!(view.scroll_x(), Some(1000.0));
}
