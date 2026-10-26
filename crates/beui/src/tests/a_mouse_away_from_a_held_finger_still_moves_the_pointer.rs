use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_mouse_away_from_a_held_finger_still_moves_the_pointer() {
    let _root = crate::reactive::RootScope::new();
    let hovered = create_signal(false);
    let target = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame width=200.0 height=200.0 />
                <Interactive @node_ref=&target on_hover_change={hovered}>
                    <Frame width=200.0 height=200.0 />
                </Interactive>
            </List>
        }
    });
    let target = target.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let finger = pos2(100.0, 100.0);
    let id = TouchId {
        device: 1,
        finger: 1,
    };
    let touch = |phase: TouchPhase, pos: Pos2| Input::Touch {
        id,
        phase,
        pos,
        force: None,
    };

    harness.frame(vec![
        touch(TouchPhase::Start, finger),
        Input::PointerMoved(finger),
    ]);
    assert!(
        !hovered.get_untracked(),
        "the mouse event a platform makes from the finger is dropped"
    );

    harness.frame(vec![Input::PointerMoved(harness.center(target))]);
    assert!(
        hovered.get_untracked(),
        "while a real mouse elsewhere hovers what it is over"
    );
    harness.frame(vec![touch(TouchPhase::End, finger)]);
}
