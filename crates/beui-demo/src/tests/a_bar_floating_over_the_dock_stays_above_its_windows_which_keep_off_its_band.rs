use super::*;
use beui::PointerButton;
use beui_host::native::testing::Target;

#[test]
fn a_bar_floating_over_the_dock_stays_above_its_windows_which_keep_off_its_band() {
    let mut test = alone(WIDE, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    test.click("bars.pages.grip");
    test.frame(Vec::new());
    test.click("grip.dock.bar.over.ends");
    test.frame(Vec::new());
    test.click("grip.dock.bar.over.bottom.right");
    test.frame(Vec::new());
    test.settle();
    test.click_with(Target::text("Tab \"Layers\""), PointerButton::Secondary);
    test.frame(Vec::new());
    test.click("dock.tab.pop-out");
    test.frame(Vec::new());
    test.settle();

    let note = test.rect_of("bars.note");
    let held = pos2(note.left() + 12.0, note.top() - 18.0);
    let to = held + WIDE;
    test.drag(Target::point(held), Target::point(to));
    test.frame(Vec::new());
    test.settle();

    let band = test.rect_of("dock.bar.band.bottom");
    let note = test.rect_of("bars.note");
    let pages = test.rect_of("bars.pages");
    assert!(
        note.bottom() <= band.top() + 0.5,
        "a window dragged past the band stops above it, whole: {note:?} over {band:?}"
    );
    assert!(
        note.intersects(pages),
        "the window reaches over the floating bar: {note:?}, {pages:?}"
    );
    let layer = |id: &str| {
        let document = test.document();
        let layers = document.layers();
        let node = document.find_test_id(id).expect("it is laid out");
        document.layer_of(&layers, node).expect("it is in a layer")
    };
    assert!(
        layer("bars.pages") > layer("bars.note"),
        "the bar floating over the dock draws above its floating window"
    );
    test.snapshot("a_bar_floating_over_the_dock_stays_above_its_window");
}
