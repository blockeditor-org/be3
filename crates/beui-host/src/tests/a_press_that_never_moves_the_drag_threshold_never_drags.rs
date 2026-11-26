use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::PointerPress;

/// The core drags a press only once it has moved the drag threshold from where it went down:
/// a quick click, a press held still for frames and one that jitters short of the threshold
/// never drag.
#[test]
fn a_press_that_never_moves_the_drag_threshold_never_drags() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal::<Vec<Pos2>>(Vec::new());
    let document = build_node(move || {
        view! {
            <Interactive
                on_drag={move |press: PointerPress| drags.update(|drags| drags.push(press.local))}
            >
                <Frame width=300.0 height=300.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let at = pos2(40.0, 60.0);
    let button = |pressed| Input::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
    };

    harness.frame(vec![Input::PointerMoved(at), button(true), button(false)]);
    assert_eq!(drags.get_untracked(), [], "a quick click does not drag");

    harness.frame(vec![button(true)]);
    harness.frame(Vec::new());
    harness.frame(vec![Input::PointerMoved(at + vec2(3.0, 0.0))]);
    harness.frame(vec![button(false)]);
    assert_eq!(
        drags.get_untracked(),
        [],
        "nor does a press held still, or moved short of the threshold"
    );
}
