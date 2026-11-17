use super::*;

#[test]
fn a_split_leaves_an_open_group_whole() {
    let mut desk = Desk::default();
    desk.begin(1, 1, GestureKind::Group);
    desk.set(1, Some(1), 1, 10);
    desk.history.split(Some(1));
    desk.set(1, Some(1), 1, 11);
    desk.commit(1, 1);
    assert_eq!(
        desk.undo_all(),
        [vec![]],
        "the group is one step whatever splits came while it was open"
    );
}
