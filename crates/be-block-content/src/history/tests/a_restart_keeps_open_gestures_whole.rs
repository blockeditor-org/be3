use super::*;

#[test]
fn a_restart_keeps_open_gestures_whole() {
    let mut desk = Desk::default();
    desk.set(1, None, 1, 1);
    desk.begin(1, 1, GestureKind::Group);
    desk.begin(1, 2, GestureKind::Tentative);
    desk.set(1, Some(1), 2, 20);
    desk.set(1, Some(2), 3, 30);
    desk.history.restart();
    assert!(desk.history.stopped());
    assert!(desk.history.holding(), "the drag is still held");
    desk.set(1, Some(1), 2, 21);
    desk.set(1, Some(2), 3, 31);
    desk.commit(1, 1);
    desk.commit(1, 2);
    assert_eq!(
        desk.undo_all(),
        [vec![(1, 1), (2, 21)], vec![(1, 1)]],
        "each gesture is still one step; the steps from before the restart are gone"
    );
}
