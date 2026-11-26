use super::*;

#[test]
fn a_gesture_named_again_commits_the_open_one_first() {
    let mut desk = Desk::default();
    desk.begin(1, 0, GestureKind::Group);
    desk.set(1, Some(0), 1, 10);
    desk.begin(1, 0, GestureKind::Group);
    desk.set(1, Some(0), 1, 11);
    desk.commit(1, 0);

    desk.begin(1, 0, GestureKind::Tentative);
    desk.set(1, Some(0), 1, 12);
    desk.begin(1, 0, GestureKind::Tentative);
    assert_eq!(
        desk.written(),
        [(1, 12)],
        "a tentative gesture begun again under its name commits what it held"
    );
    desk.set(1, Some(0), 1, 13);
    desk.commit(1, 0);
    assert_eq!(
        desk.undo_all(),
        [vec![(1, 12)], vec![(1, 11)], vec![(1, 10)], vec![]]
    );
}
