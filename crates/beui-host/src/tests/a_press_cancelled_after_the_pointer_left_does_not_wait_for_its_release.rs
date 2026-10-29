use super::*;

#[test]
fn a_press_cancelled_after_the_pointer_left_does_not_wait_for_its_release() {
    let mut intaken = Intaken::new();
    let button = |pressed| Input::PointerButton {
        pos: pos2(5.0, 5.0),
        button: PointerButton::Primary,
        pressed,
    };
    intaken.runner.push(button(true));
    intaken.runner.push(Input::PointerLeft);
    intaken.frame(1.0, []);

    let seen = intaken.frame(1.0, [Input::PointersCancelled.into()]);
    assert_eq!(seen, [Event::PointersCancelled]);

    intaken.runner.push(Input::PointerMoved(pos2(8.0, 8.0)));
    intaken.runner.push(button(true));
    intaken.runner.push(button(false));
    let seen = intaken.frame(1.0, []);
    assert!(
        !seen.contains(&Event::PointerGone),
        "the next click inside the window keeps its pointer: {seen:?}"
    );
}
