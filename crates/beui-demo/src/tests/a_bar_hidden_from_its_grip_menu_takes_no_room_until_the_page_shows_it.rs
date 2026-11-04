use super::*;

fn hide(test: &mut DocumentTest, name: &str) {
    test.click(format!("bars.{name}.grip").as_str());
    test.frame(Vec::new());
    test.click("grip.dock.bar.hide");
    test.frame(Vec::new());
    test.settle();
}

#[test]
fn a_bar_hidden_from_its_grip_menu_takes_no_room_until_the_page_shows_it() {
    let mut test = alone(WIDE, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let band = test.rect_of("dock.bar.band.bottom");
    let stage = test.rect_of("bars.stage");

    hide(&mut test, "status");
    assert!(!test.shows("bars.status"), "the hidden bar draws nothing");
    let pages = test.rect_of("bars.pages");
    let zoom = test.rect_of("bars.zoom");
    assert!(
        (pages.center().x - band.center().x).abs() < 1.0
            && (band.right() - zoom.right()).abs() < 1.0,
        "the other bars keep their places in the band: {pages:?}, {zoom:?} in {band:?}"
    );
    assert!(test.shows("bars.show"), "the page offers to show it again");
    test.snapshot("a_status_bar_hidden_from_its_grip_menu");

    hide(&mut test, "pages");
    hide(&mut test, "zoom");
    assert!(
        !test.shows("dock.bar.band.bottom"),
        "with every bar beside it hidden, the band is gone"
    );
    let taller = test.rect_of("bars.stage");
    assert!(
        taller.bottom() > stage.bottom() + band.height() / 2.0,
        "and the stage takes its room: {taller:?}, was {stage:?}"
    );

    test.click("bars.show");
    test.frame(Vec::new());
    test.settle();
    for bar in ["bars.status", "bars.pages", "bars.zoom"] {
        assert!(test.shows(bar), "{bar} is shown again");
    }
    assert_eq!(test.rect_of("dock.bar.band.bottom"), band);
    assert!(!test.shows("bars.show"));
}
