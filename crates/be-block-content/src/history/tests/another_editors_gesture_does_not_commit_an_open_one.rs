use super::*;

#[test]
fn another_editors_gesture_does_not_commit_an_open_one() {
    let mut desk = Desk::default();
    desk.begin(1, 0, GestureKind::Group);
    desk.set(1, Some(0), 1, 10);
    desk.begin(2, 0, GestureKind::Group);
    desk.set(2, Some(0), 2, 20);
    desk.set(1, Some(0), 1, 11);
    desk.commit(2, 0);
    desk.set(1, Some(0), 1, 12);
    desk.commit(1, 0);
    assert_eq!(
        desk.undo_all(),
        [vec![(2, 20)], vec![]],
        "each window's drag is one step, the first's holding every edit it made after the \
         second began"
    );
}
