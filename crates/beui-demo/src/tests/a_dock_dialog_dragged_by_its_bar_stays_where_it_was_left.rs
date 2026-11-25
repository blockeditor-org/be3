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
    let by = Vec2::new(-60.0, 90.0);
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

    let margin = f32::from(beui::Placement::CARD.edge_margin());
    let drag_to = |test: &mut DocumentTest, to: Pos2| {
        let grab = test.rect_of(&header).center();
        test.drag(grab, to);
        test.settle();
        test.rect_of(&header)
    };
    let left = drag_to(&mut test, pos2(-200.0, 300.0));
    // The bar is inset in the dialog's frame by its border.
    let inset = left.left() - margin;
    assert!(
        (0.0..=4.0).contains(&inset),
        "dragged past the left edge, it stops a card's margin inside it: {left:?}"
    );
    let right = drag_to(&mut test, pos2(WIDE.x + 200.0, 300.0));
    assert_eq!(
        right.right(),
        WIDE.x - margin - inset,
        "and past the right edge, as far inside it"
    );
    drag_to(&mut test, pos2(300.0, WIDE.y + 200.0));
    let body = test.rect_of(&dialog);
    let below = WIDE.y - margin - body.bottom();
    assert!(
        (0.0..=4.0).contains(&below),
        "and past the bottom, its body just above the margin: {below} short of it"
    );
}
