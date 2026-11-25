use super::*;

#[test]
fn a_dock_dialog_dragged_by_its_bar_stays_where_it_was_left() {
    let mut test = demo(WIDE);
    reveal(&mut test, "demo.dialog.open");
    test.click("demo.dialog.open");
    test.frame(Vec::new());
    let dialog = dialog_id(0);
    let header = format!("{dialog}.header");
    let before = test.rect_of(&dialog);

    let grab = test.rect_of(&header).center();
    let by = Vec2::new(-120.0, 90.0);
    test.drag(grab, grab + by);
    test.settle();
    let after = test.rect_of(&dialog);
    assert!(
        (after.min - (before.min + by)).length() < 1.0,
        "the dialog follows its bar: {after:?} from {before:?} by {by:?}"
    );
    test.snapshot("docking_dialog_moved");

    test.frame(Vec::new());
    assert_eq!(
        test.rect_of(&dialog),
        after,
        "and stays where it was let go"
    );
}
