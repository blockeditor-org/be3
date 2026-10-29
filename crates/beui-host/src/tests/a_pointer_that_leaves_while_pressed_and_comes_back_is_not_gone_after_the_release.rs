use super::*;

#[test]
fn a_pointer_that_leaves_while_pressed_and_comes_back_is_not_gone_after_the_release() {
    let mut intaken = Intaken::new();
    intaken.runner.push(Input::PointerButton {
        pos: pos2(5.0, 5.0),
        button: PointerButton::Primary,
        pressed: true,
    });
    intaken.runner.push(Input::PointerLeft);
    intaken.runner.push(Input::PointerMoved(pos2(-30.0, 5.0)));
    intaken.frame(1.0, []);

    intaken.runner.push(Input::PointerMoved(pos2(8.0, 5.0)));
    intaken.runner.push(Input::PointerButton {
        pos: pos2(8.0, 5.0),
        button: PointerButton::Primary,
        pressed: false,
    });
    let seen = intaken.frame(1.0, []);
    assert!(
        !seen.contains(&Event::PointerGone),
        "a pointer released back inside the window is still there: {seen:?}"
    );
}
