use super::*;
use crate::unstyled::dock_state;

#[test]
fn without_a_drag_modifier_a_modified_drag_reaches_the_dock_content() {
    let SplitDock {
        document,
        dock,
        presses,
    } = split_dock(None);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let content = harness.rect(harness.find("content.1"));
    harness.click(content.center());
    harness.frame(Vec::new());
    presses.set(0);
    let before = dock_state(harness.document(), dock);
    let right = harness.center(harness.find("content.2"));

    drag_with(&mut harness, content.center(), right, Modifiers::LOGO);
    secondary_drag_with(
        &mut harness,
        content.center(),
        content.center() + vec2(120.0, 0.0),
        Modifiers::LOGO,
    );
    harness.frame(Vec::new());

    assert_eq!(
        dock_state(harness.document(), dock),
        before,
        "a dock with no drag modifier moves nothing"
    );
    assert_eq!(harness.rect(harness.find("content.1")), content);
    assert_eq!(
        presses.get(),
        2,
        "both presses reached the content they landed on"
    );
}
