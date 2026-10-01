use super::*;
use crate::reactive::NodeRef;

#[test]
fn a_released_sheet_springs_to_its_stop_or_closed() {
    let sheet = NodeRef::new();
    let closed = Rc::new(Cell::new(0));
    let document = sheet_of_rows(0.5, &sheet, &closed);
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
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
        1,
        "a quick flick down closes the sheet, though it was let go high up"
    );
    harness.settle();
    assert_eq!(
        height(&harness),
        300.0,
        "a sheet its owner keeps open springs back to where it rests"
    );
}
