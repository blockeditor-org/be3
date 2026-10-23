use super::*;

#[test]
fn a_move_carries_its_prediction_along() {
    let mut replay = Replay::default();
    let mut moved = pointer(PointerId::Mouse, PointerPhase::Move, pos2(120.0, 70.0));
    moved.predicted = Some(pos2(140.0, 90.0));
    let mut pressed = moved;
    pressed.phase = PointerPhase::Down(PRIMARY);
    assert_eq!(
        replayed(&mut replay, vec![sent(moved), sent(pressed)], 2.0),
        [
            Input::PointerMoved(pos2(10.0, 10.0)),
            Input::Predicted(pos2(20.0, 20.0)),
            button(pos2(10.0, 10.0), true),
        ],
        "only a move predicts where the pointer goes next"
    );
}
