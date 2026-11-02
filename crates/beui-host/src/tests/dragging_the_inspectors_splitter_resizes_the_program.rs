use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn dragging_the_inspectors_splitter_resizes_the_program() {
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
    let program = harness.program_rect();
    assert_eq!(program.left(), 0.0, "the program keeps the left of the window");
    assert!(
        (WIDE_VIEWPORT.x - program.right() - 340.0).abs() <= 1.0,
        "the tools start 340 points wide, the program has {program:?}"
    );
    assert_eq!(
        harness.document().node_rect(text).map(|rect| rect.right()),
        Some(program.right()),
        "the program is laid out in its pane"
    );

    let edge = program.right() + 0.5;
    harness.drag(pos2(edge, 300.0), pos2(edge - 80.0, 300.0));
    harness.settle();

    let moved = harness.program_rect();
    assert!(
        (moved.right() - (program.right() - 80.0)).abs() <= 1.0,
        "the program ends where the splitter was let go: {moved:?}"
    );
    assert_eq!(
        harness.document().node_rect(text).map(|rect| rect.right()),
        Some(moved.right())
    );
}
