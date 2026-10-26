use super::*;

#[test]
fn the_action_bar_sample_pins_what_fits_and_offers_the_rest() {
    let mut test = alone(WIDE, Page::Menus);
    test.frame(Vec::new());
    reveal(&mut test, "bar.narrow.menu");
    assert!(test.shows("bar.pin.toolbar.undo"));
    assert!(test.shows("bar.pin.toolbar.share"));
    assert!(test.shows("bar.narrow.pin.toolbar.undo"));
    assert!(
        !test.shows("bar.narrow.pin.toolbar.share"),
        "a bar with no room for labels leaves the labelled action to its menu"
    );
    test.click("bar.narrow.menu");
    test.frame(Vec::new());
    assert!(test.shows("bar.narrow.menu.toolbar.share"));
    test.snapshot("action_bar");

    test.click("bar.narrow.menu.toolbar.share");
    test.frame(Vec::new());
    let root = test.document().root().expect("the demo has a root");
    assert_eq!(showing(test.document(), root, "Ran Share"), 1);
}
