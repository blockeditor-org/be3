use super::*;

#[test]
fn tentative_gestures_commit_as_one_step_when_the_last_ends() {
    let mut desk = Desk::default();
    desk.set(1, None, 1, 1);
    desk.set(1, None, 2, 2);
    desk.now += 1_000;
    desk.begin(1, 1, GestureKind::Tentative);
    desk.begin(2, 1, GestureKind::Tentative);
    desk.set(1, Some(1), 1, 10);
    desk.set(2, Some(1), 2, 20);
    desk.set(1, Some(1), 1, 11);
    assert!(desk.history.holding());
    assert_eq!(desk.visible(), [(1, 11), (2, 20)]);
    assert_eq!(desk.written(), [(1, 1), (2, 2)], "held edits are not written");
    desk.commit(1, 1);
    assert_eq!(
        desk.written(),
        [(1, 1), (2, 2)],
        "a committed gesture waits for the rest of the layer"
    );
    desk.set(2, Some(1), 2, 21);
    desk.commit(2, 1);
    assert!(!desk.history.holding());
    assert_eq!(desk.written(), [(1, 11), (2, 21)]);
    assert_eq!(
        desk.undo_all(),
        [vec![(1, 1), (2, 2)], vec![(1, 1)], vec![]],
        "both drags are one step"
    );
}
