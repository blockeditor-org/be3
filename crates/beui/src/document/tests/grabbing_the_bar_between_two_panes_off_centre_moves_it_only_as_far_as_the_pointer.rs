use super::*;

#[test]
fn grabbing_the_bar_between_two_panes_off_centre_moves_it_only_as_far_as_the_pointer() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    harness.drag(tab, pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y / 2.0));
    harness.frame(Vec::new());
    let before = harness.rect(harness.find("content.1"));
    let edge = pos2(before.right() + 5.5, WIDE_VIEWPORT.y / 2.0);

    harness.click(edge);
    harness.frame(Vec::new());
    let clicked = harness.rect(harness.find("content.1"));
    assert_eq!(
        clicked.width(),
        before.width(),
        "pressing the far side of the bar leaves it where it was"
    );

    harness.drag(edge, pos2(edge.x + 40.0, edge.y));
    harness.frame(Vec::new());
    let after = harness.rect(harness.find("content.1"));
    assert!(
        (after.width() - before.width() - 40.0).abs() <= 1.0,
        "the bar follows the pointer by the distance it moved, \
         {} points wider instead of 40",
        after.width() - before.width()
    );
}
