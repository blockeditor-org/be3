use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_release_that_moves_the_inspectors_edge_resizes_the_panel_to_it() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <Text color=Color32::WHITE>"Hello"</Text>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    let width = harness.inspector().width;
    let edge = pos2(WIDE_VIEWPORT.x - width, 100.0);
    let to = edge - vec2(80.0, 0.0);
    let button = |pos, pressed| Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
    };

    harness.frame(vec![Input::PointerMoved(edge)]);
    harness.frame(vec![button(edge, true)]);
    harness.frame(vec![Input::PointerMoved(edge - vec2(40.0, 0.0))]);
    harness.frame(vec![Input::PointerMoved(to), button(to, false)]);

    assert_eq!(
        harness.inspector().width,
        width + 80.0,
        "the panel ends where the edge was let go"
    );
}
