use super::*;

#[test]
fn cancelling_one_tentative_gesture_keeps_the_rest_held() {
    let mut desk = Desk::default();
    desk.begin(1, 1, GestureKind::Tentative);
    desk.begin(1, 2, GestureKind::Tentative);
    desk.set(1, Some(1), 1, 10);
    desk.set(1, Some(2), 2, 20);
    desk.cancel(1, 1);
    assert_eq!(desk.visible(), [(2, 20)]);
    assert!(desk.history.holding(), "the other drag is still held");
    desk.commit(1, 2);
    assert_eq!(desk.written(), [(2, 20)]);
    assert_eq!(desk.undo_all(), [vec![]]);

    desk.begin(1, 1, GestureKind::Tentative);
    desk.begin(1, 2, GestureKind::Tentative);
    desk.set(1, Some(1), 1, 10);
    desk.set(1, Some(2), 2, 20);
    desk.commit(1, 1);
    desk.cancel(1, 2);
    assert_eq!(
        desk.written(),
        [(1, 10)],
        "the last gesture ending by a cancel still commits what the others committed"
    );
    assert_eq!(desk.undo_all(), [vec![]]);
}
