use super::*;

#[test]
fn ending_a_gesture_that_is_not_open_changes_nothing() {
    let mut desk = Desk::default();
    desk.begin(1, 1, GestureKind::Group);
    desk.begin(1, 2, GestureKind::Tentative);
    desk.set(1, Some(1), 1, 10);
    desk.set(1, Some(2), 2, 20);
    desk.cancel(1, 3);
    desk.cancel(2, 1);
    desk.commit(2, 2);
    assert!(desk.history.holding());
    assert_eq!(desk.history.tentative(), [gesture(1, 2)]);
    assert_eq!(
        desk.visible(),
        [(1, 10), (2, 20)],
        "a cancel or commit under a name no gesture is open under, even one sharing an editor \
         or an id with an open one, ends nothing"
    );
    desk.commit(1, 2);
    desk.commit(1, 1);
    assert_eq!(desk.undo_all(), [vec![(2, 20)], vec![]]);
}
