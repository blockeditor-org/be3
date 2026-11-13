use super::*;

#[test]
fn a_joined_docks_bar_moves_to_the_edge_of_another_of_its_panes() {
    let mut test = demo(WIDE);
    open(&mut test, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let stage = test.rect_of("bars.stage");
    let history = test.rect_of("bars.history");
    assert!(
        stage.contains(history.center()),
        "joined into the demo's dock, the page's own history bar floats over its first pane, the stage"
    );

    let side = test.rect_of("bars.note");
    let grip = test.rect_of("bars.history.grip").center();
    drop_bar(&mut test, grip, pos2(side.right() - 30.0, side.center().y));
    test.frame(Vec::new());
    test.settle();
    let side = test.rect_of("bars.note");
    let history = test.rect_of("bars.history");
    assert!(
        history.center().x > test.rect_of("bars.stage").right()
            && (history.right() - side.right()).abs() < side.width() / 2.0,
        "dropped by the side pane's right edge, the bar moves to that pane: {history:?} by {side:?}"
    );
    test.snapshot("a_joined_docks_bar_moved_to_the_edge_of_its_side_pane");
}
