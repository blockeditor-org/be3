use super::*;

const CATALOG_INSET: f32 = 4.0;
const NARROW_PANE: f32 = 400.0;

#[test]
fn a_narrow_pane_moves_its_labelled_pin_into_its_menu() {
    let mut test = demo(WIDE);
    assert!(
        test.shows("dock.pin.demo.reset"),
        "a wide pane's bar holds the action pinned with a label"
    );

    let row = test.rect_of("demo.catalog.Docking");
    let splitter = pos2(row.right() + CATALOG_INSET + 0.5, row.center().y);
    test.drag(splitter, pos2(WIDE.x - NARROW_PANE, splitter.y));
    test.frame(Vec::new());
    assert!(
        test.shows("dock.pin.demo.edit"),
        "a narrow pane's bar keeps the action pinned as an icon"
    );
    assert!(
        !test.shows("dock.pin.demo.reset"),
        "but leaves out the one pinned with a label"
    );
    test.snapshot("docking_narrow_pane");

    test.click("dock.menu");
    test.frame(Vec::new());
    assert!(
        test.shows("dock.menu.demo.reset"),
        "and its menu offers it instead"
    );
}
