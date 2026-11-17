use super::*;

#[test]
fn undo_commits_open_undo_groups_first() {
    let mut desk = Desk::default();
    desk.begin(1, 0, GestureKind::Group);
    desk.begin(2, 0, GestureKind::Group);
    desk.set(1, Some(0), 1, 10);
    desk.set(2, Some(0), 2, 20);
    assert!(desk.undo());
    assert_eq!(
        desk.visible(),
        [(1, 10)],
        "the open groups commit, the one begun last on top, and undo takes it back"
    );
    assert!(!desk.history.gesture_open());
    desk.set(1, Some(0), 1, 11);
    assert_eq!(
        desk.undo_all(),
        [vec![(1, 10)], vec![]],
        "an edit after the group committed is its own step"
    );
}
