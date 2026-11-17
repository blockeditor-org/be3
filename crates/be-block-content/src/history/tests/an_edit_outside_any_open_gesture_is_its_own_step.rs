use super::*;

#[test]
fn an_edit_outside_any_open_gesture_is_its_own_step() {
    let mut desk = Desk::default();
    desk.begin(1, 1, GestureKind::Group);
    desk.set(1, Some(1), 1, 10);
    desk.now += 1_000;
    desk.set(1, None, 2, 20);
    desk.now += 1_000;
    desk.set(1, Some(7), 3, 30);
    desk.now += 1_000;
    desk.set(2, Some(1), 4, 40);
    desk.commit(1, 1);
    assert_eq!(
        desk.undo_all(),
        [
            vec![(2, 20), (3, 30), (4, 40)],
            vec![(2, 20), (3, 30)],
            vec![(2, 20)],
            vec![]
        ],
        "an edit outside the gesture, one naming a gesture not open, and another editor's \
         edit under the same id each step on their own"
    );
}
