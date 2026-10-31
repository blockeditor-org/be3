use super::*;

#[test]
fn a_status_bar_of_the_whole_dock_runs_under_every_pane() {
    let mut test = alone(WIDE, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let band = test.rect_of("dock.bar.band.bottom");
    let stage = test.rect_of("bars.stage");
    assert!(
        band.width() > band.height() && band.top() >= stage.bottom(),
        "the dock's status bars take a band under the stage"
    );
    assert!(
        band.right() > stage.right() + stage.width() / 4.0,
        "and it runs on under the side pane: {band:?}, {stage:?}"
    );
    let status = test.rect_of("bars.status");
    let pages = test.rect_of("bars.pages");
    let zoom = test.rect_of("bars.zoom");
    for bar in [status, pages, zoom] {
        assert!(
            band.contains_rect(bar),
            "the three bars share the one band: {bar:?} in {band:?}"
        );
    }
    assert!(
        (status.left() - band.left()).abs() < 1.0,
        "the status bar sits at the band's start"
    );
    assert!(
        (pages.center().x - band.center().x).abs() < 1.0,
        "the page bar sits at its centre"
    );
    assert!(
        (band.right() - zoom.right()).abs() < 1.0,
        "and the zoom bar at its end"
    );
    test.snapshot("a_status_bar_runs_along_the_bottom_of_the_whole_dock");
}
