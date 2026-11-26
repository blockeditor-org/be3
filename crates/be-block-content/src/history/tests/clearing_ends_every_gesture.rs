use super::*;

#[test]
fn clearing_ends_every_gesture() {
    let mut desk = Desk::default();
    desk.set(1, None, 1, 1);
    desk.begin(1, 1, GestureKind::Group);
    desk.begin(1, 2, GestureKind::Tentative);
    desk.set(1, Some(1), 2, 20);
    desk.set(1, Some(2), 3, 30);
    desk.history.clear();
    desk.layer.clear();
    assert!(!desk.history.gesture_open());
    assert!(!desk.history.holding());
    assert!(!desk.history.can_undo());
    desk.commit(1, 1);
    desk.commit(1, 2);
    assert!(
        !desk.history.can_undo(),
        "a commit after the clear keeps nothing"
    );
}
