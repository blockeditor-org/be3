use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn dragging_the_inspectors_splitter_resizes_the_app() {
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
    harness.settle();
    let app = harness.app_rect();
    assert_eq!(app.left(), 0.0, "the app keeps the left of the window");
    assert!(
        (WIDE_VIEWPORT.x - app.right() - 340.0).abs() <= 1.0,
        "the tools start 340 points wide, the app has {app:?}"
    );
    assert_eq!(
        harness.document().node_rect(text).map(|rect| rect.right()),
        Some(app.right()),
        "the app is laid out in its pane"
    );

    let edge = app.right() + 0.5;
    harness.drag(pos2(edge, 300.0), pos2(edge - 80.0, 300.0));
    harness.settle();

    let moved = harness.app_rect();
    assert!(
        (moved.right() - (app.right() - 80.0)).abs() <= 1.0,
        "the app ends where the splitter was let go: {moved:?}"
    );
    assert_eq!(
        harness.document().node_rect(text).map(|rect| rect.right()),
        Some(moved.right())
    );
}
