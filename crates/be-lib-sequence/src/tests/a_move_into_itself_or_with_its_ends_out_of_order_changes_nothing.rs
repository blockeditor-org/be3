use super::*;

#[test]
fn a_move_into_itself_or_with_its_ends_out_of_order_changes_nothing() {
    let mut sequence = loaded("abcdef");
    let at = |offset| Pos {
        client: LOADED,
        offset,
    };
    let stale = sequence
        .move_range(1..4, 6)
        .expect("the range moves somewhere else");

    for refused in [
        SeqOp::Move {
            first: at(1),
            last: at(3),
            after: Some(at(2)),
        },
        SeqOp::Move {
            first: at(3),
            last: at(1),
            after: None,
        },
        SeqOp::Move {
            first: at(1),
            last: at(3),
            after: Some(at(0)),
        },
    ] {
        assert_eq!(sequence.apply(&refused), None);
    }
    assert_eq!(sequence.move_range(1..4, 2), None);

    let first_to_the_end = sequence
        .move_range(1..2, 6)
        .expect("the range moves somewhere else");
    applied(&mut sequence, &first_to_the_end);
    assert_eq!(text(&sequence), "acdefb");
    assert_eq!(sequence.apply(&stale), None);
    assert_eq!(text(&sequence), "acdefb");
}
