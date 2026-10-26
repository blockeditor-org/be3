use super::*;

#[test]
fn a_held_press_is_replayed_where_the_pointer_is_and_leaves_once_over_nothing() {
    let other = NodeId::from_key(7);
    let aim = |over: Option<Over>, pos: Pos2| match over {
        Some(over) if over.node == other => Aim::Other {
            node: other,
            at: Pos2::ZERO + over.local.to_vec2(),
        },
        _ if rect().contains(pos) => Aim::Own,
        _ => Aim::Away,
    };
    let over = |pos: Pos2, local: Pos2| Forwarded::Pointer {
        event: pointer(PointerId::Mouse, PointerPhase::Move, pos),
        over: Some(Over { node: other, local }),
    };
    let mut replay = Replay::default();
    let input = ForwardedInput {
        events: vec![
            mouse(PointerPhase::Down(PRIMARY), pos2(110.0, 60.0)),
            over(pos2(400.0, 60.0), pos2(30.0, 20.0)),
            mouse(PointerPhase::Move, pos2(500.0, 500.0)),
            mouse(PointerPhase::Move, pos2(520.0, 500.0)),
            mouse(PointerPhase::Move, pos2(120.0, 70.0)),
            mouse(PointerPhase::Up(PRIMARY), pos2(120.0, 70.0)),
        ],
        rect: rect(),
        hovered: true,
        focused: true,
        pointer: None,
        modifiers: Modifiers::NONE,
    };
    let replayed: Vec<(Input, Option<NodeId>)> = replay
        .replay(&input, 1.0, aim)
        .into_iter()
        .map(|replayed| (replayed.input, replayed.over))
        .collect();
    assert_eq!(
        replayed,
        [
            (button(pos2(10.0, 10.0), true), None),
            (Input::PointerMoved(pos2(30.0, 20.0)), Some(other)),
            (Input::PointerLeft, None),
            (Input::PointerMoved(pos2(400.0, 450.0)), None),
            (Input::PointerMoved(pos2(420.0, 450.0)), None),
            (Input::PointerMoved(pos2(20.0, 20.0)), None),
            (button(pos2(20.0, 20.0), false), None),
        ],
        "over another region the press is replayed in its coordinates and names it; over none \
         the pointer leaves once and goes on in the region's own coordinates"
    );
}
