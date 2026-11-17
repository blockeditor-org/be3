use super::*;

#[test]
fn a_cancel_ends_only_the_gesture_it_names() {
    let mut desk = Desk::default();
    desk.begin(1, 1, GestureKind::Group);
    desk.set(1, Some(1), 1, 10);
    desk.begin(1, 2, GestureKind::Group);
    desk.set(1, Some(2), 2, 20);
    desk.set(1, Some(1), 1, 11);
    desk.cancel(1, 1);
    assert_eq!(
        desk.visible(),
        [(2, 20)],
        "the first finger's cancel takes back only its own edits"
    );
    desk.set(1, Some(2), 2, 21);
    desk.commit(1, 2);
    assert_eq!(desk.undo_all(), [vec![]], "the second finger's gesture is one step");
}
