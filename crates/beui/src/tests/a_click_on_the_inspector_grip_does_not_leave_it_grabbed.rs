use super::*;
use crate::reactive::*;

fn button(pos: Pos2, pressed: bool) -> Input {
    Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
    }
}

#[test]
fn a_click_on_the_inspector_grip_does_not_leave_it_grabbed() {
    let document = build_node(|| {
        view! {
            <Frame />
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.frame(Vec::new());
    let panel = |harness: &Harness| {
        harness
            .inspector()
            .panel_rect()
            .expect("the inspector shows its panel")
    };
    let shown = panel(&harness);
    let grip = pos2(shown.left(), shown.center().y);
    let left = pos2(grip.x - 100.0, grip.y);

    harness.frame(vec![
        Input::PointerMoved(grip),
        button(grip, true),
        button(grip, false),
    ]);
    harness.frame(vec![Input::PointerMoved(left)]);
    assert_eq!(
        panel(&harness),
        shown,
        "a press and release in one frame leaves the panel where it was"
    );

    harness.frame(vec![Input::PointerMoved(grip), button(grip, true)]);
    harness.frame(vec![Input::PointerMoved(left)]);
    harness.frame(vec![button(left, false)]);
    assert!(
        panel(&harness).width() > shown.width(),
        "while a drag of the grip widens it"
    );
}
