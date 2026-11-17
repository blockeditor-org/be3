use super::*;

#[test]
fn undo_while_held_cancels_the_layer_and_redo_is_refused() {
    let mut desk = Desk::default();
    desk.set(1, None, 1, 1);
    desk.now += 1_000;
    desk.set(1, None, 1, 2);
    assert!(desk.undo());
    assert!(desk.history.can_redo());
    desk.begin(1, 1, GestureKind::Tentative);
    desk.begin(2, 1, GestureKind::Tentative);
    desk.set(1, Some(1), 2, 10);
    desk.set(2, Some(1), 3, 20);
    desk.commit(2, 1);
    assert!(!desk.history.can_redo(), "redo is refused while the layer is held");
    assert!(!desk.redo());
    assert_eq!(desk.visible(), [(1, 1), (2, 10), (3, 20)]);

    assert!(desk.history.can_undo());
    assert_eq!(desk.history.tentative(), [gesture(1, 1)]);
    assert!(desk.undo(), "undo cancels the open tentative gestures");
    assert!(!desk.history.holding());
    assert_eq!(
        desk.visible(),
        [(1, 1)],
        "the whole layer goes, the committed gesture's part with it, and only it"
    );
    assert!(desk.redo(), "the redo held back is still there");
    assert_eq!(desk.visible(), [(1, 2)]);
    assert_eq!(desk.undo_all(), [vec![(1, 1)], vec![]]);
}
