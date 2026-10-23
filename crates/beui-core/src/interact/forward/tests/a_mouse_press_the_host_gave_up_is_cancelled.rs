use super::*;

#[test]
fn a_mouse_press_the_host_gave_up_is_cancelled() {
    let mut replay = Replay::default();
    let pos = pos2(100.0, 50.0);
    assert_eq!(
        replayed(
            &mut replay,
            vec![
                mouse(PointerPhase::Down(PRIMARY), pos),
                mouse(PointerPhase::Gone, pos),
            ],
            1.0
        ),
        [button(Pos2::ZERO, true), Input::PointersCancelled],
        "a press that ends without its release is over"
    );
    assert_eq!(
        replayed(
            &mut replay,
            vec![
                mouse(PointerPhase::Down(PRIMARY), pos),
                mouse(PointerPhase::Up(PRIMARY), pos),
                mouse(PointerPhase::Gone, pos),
                Forwarded::Left,
            ],
            1.0
        ),
        [
            button(Pos2::ZERO, true),
            button(Pos2::ZERO, false),
            Input::PointerLeft,
        ],
        "a mouse let go and then gone has only left"
    );
}
