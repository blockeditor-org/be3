use super::*;

#[test]
fn a_status_bar_of_the_whole_dock_runs_under_every_pane() {
    let mut test = alone(WIDE, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let status = test.rect_of("bars.status");
    let stage = test.rect_of("bars.stage");
    assert!(
        status.width() > status.height() && status.top() >= stage.bottom(),
        "the dock's status bar takes a band under the stage"
    );
    assert!(
        status.right() > stage.right() + stage.width() / 4.0,
        "and runs on under the side pane: {status:?}, {stage:?}"
    );
    test.snapshot("a_status_bar_runs_along_the_bottom_of_the_whole_dock");
}
