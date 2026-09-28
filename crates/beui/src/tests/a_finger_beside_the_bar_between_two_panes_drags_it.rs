use super::*;

#[test]
fn a_finger_beside_the_bar_between_two_panes_drags_it() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    harness.drag(tab, pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y / 2.0));
    harness.frame(Vec::new());
    let before = harness.rect(harness.find("content.1"));

    let beside = pos2(before.right() - 6.0, WIDE_VIEWPORT.y / 2.0);
    harness.finger_drag(&[
        beside,
        beside + vec2(-20.0, 0.0),
        beside + vec2(-200.0, 0.0),
    ]);
    harness.frame(Vec::new());

    let after = harness.rect(harness.find("content.1"));
    assert!(
        after.width() < before.width() - 150.0,
        "a finger just beside the bar reaches it and drags it, \
         leaving the pane on the left {} points instead of {}",
        after.width(),
        before.width()
    );
}
