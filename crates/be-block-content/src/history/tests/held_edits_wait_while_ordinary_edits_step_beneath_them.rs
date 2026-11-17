use super::*;

#[test]
fn held_edits_wait_while_ordinary_edits_step_beneath_them() {
    let mut desk = Desk::default();
    desk.begin(1, 1, GestureKind::Tentative);
    desk.set(1, Some(1), 1, 10);
    desk.now += 1_000;
    desk.set(2, None, 2, 20);
    desk.now += 1_000;
    desk.set(2, None, 2, 21);
    assert_eq!(desk.written(), [(2, 21)], "typing during the drag is written at once");
    desk.set(1, Some(1), 1, 11);
    desk.commit(1, 1);
    assert_eq!(desk.written(), [(1, 11), (2, 21)]);
    assert_eq!(
        desk.undo_all(),
        [vec![(2, 21)], vec![(2, 20)], vec![]],
        "the drag is one step, on top of the edits written while it was held"
    );
}
