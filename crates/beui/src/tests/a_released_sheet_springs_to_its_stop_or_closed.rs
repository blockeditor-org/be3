use super::*;
use crate::reactive::NodeRef;

#[test]
fn a_released_sheet_springs_to_its_stop_or_closed() {
    let sheet = NodeRef::new();
    let closed = Rc::new(Cell::new(0));
    let document = sheet_of_rows(0.5, &sheet, &closed);
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    let sheet = sheet.get();
    let height = |harness: &Harness| harness.rect(sheet).height();

    harness.finger_drag_and_hold(pos2(200.0, 450.0), pos2(200.0, 270.0));
    let let_go = height(&harness);
    harness.frame(Vec::new());
    let springing = height(&harness);
    assert!(
        springing > let_go && springing < 540.0,
        "released between stops, the sheet moves towards the nearest one \
         rather than jumping there, and is at {springing}"
    );
    harness.settle();
    assert_eq!(height(&harness), 540.0, "then comes to rest on it");

    let from = pos2(200.0, 200.0);
    harness.finger_drag(&[
        from,
        from + vec2(0.0, 40.0),
        from + vec2(0.0, 80.0),
        from + vec2(0.0, 120.0),
    ]);
    assert_eq!(
        closed.get(),
        0,
        "a quick flick down does not close the sheet the moment it is let go"
    );
    harness.frame(Vec::new());
    assert!(
        height(&harness) < 420.0,
        "it carries on down with the flick"
    );
    harness.settle();
    assert_eq!(
        closed.get(),
        1,
        "and closes once it reaches the bottom, though it was let go high up"
    );
}
