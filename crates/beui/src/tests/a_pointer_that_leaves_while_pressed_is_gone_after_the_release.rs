use super::*;

#[test]
fn a_pointer_that_leaves_while_pressed_is_gone_after_the_release() {
    let mut intaken = Intaken::new();
    intaken
        .runner
        .pointer_button(pos2(5.0, 5.0), PointerButton::Primary, true);
    intaken.runner.push(Event::PointerGone);
    let seen = intaken.frame(1.0, []);
    assert!(
        !seen.contains(&Event::PointerGone),
        "a drag that leaves the window keeps its pointer: {seen:?}"
    );

    intaken
        .runner
        .pointer_button(pos2(5.0, 5.0), PointerButton::Primary, false);
    let seen = intaken.frame(1.0, []);
    assert_eq!(seen.last(), Some(&Event::PointerGone), "{seen:?}");
}
