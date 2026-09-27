use super::*;
use crate::unstyled::dock_state;

#[test]
fn a_window_larger_than_the_dock_is_drawn_no_larger_than_it() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let window = floated_window(&mut harness, dock);
    let stored = dock_state(harness.document(), dock)
        .window_rect(window)
        .expect("the window has a rect");
    let content = |harness: &Harness| harness.rect(harness.find("content.2"));
    let chrome = stored.size() - content(&harness).size();

    *harness.viewport_mut() = vec2(stored.width() - 100.0, stored.height() - 60.0);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let shrunk = harness.rect(dock);
    let drawn = content(&harness);
    assert!(
        drawn.width() + chrome.x <= shrunk.width() && drawn.height() + chrome.y <= shrunk.height(),
        "the window fits inside the smaller dock: {drawn:?} in {shrunk:?}"
    );
    assert_eq!(
        dock_state(harness.document(), dock).window_rect(window),
        Some(stored),
        "the window keeps the size it was given for when the dock grows again"
    );
}
