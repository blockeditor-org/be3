use super::*;

/// A caret at the end of a line stands after the last character and the gaps right after it,
/// as a rule's last share or a table's closing room, but before an icon, which holds no bytes
/// either. The stops are made as a text makes them from its pieces.
#[test]
fn a_caret_at_a_lines_end_stands_past_its_gaps_but_not_an_icon() {
    let mut stops = stops_of(&[(1, "ab")]);
    let icon = Piece::Skip { newline: false };
    for (piece, node) in [
        (&Piece::Gap, 2),
        (&Piece::Gap, 3),
        (&icon, 4),
        (&Piece::Gap, 5),
    ] {
        stops.extend(empty_stop(piece, 2, node));
    }
    let (stop, rooms) = ending_at(&stops, 2).expect("b ends at 2");
    let nodes: Vec<u32> = std::iter::once(stop)
        .chain(rooms)
        .map(|stop| match stop {
            Stop::Char { node, .. } | Stop::Atom { node, .. } | Stop::Room { node, .. } => *node,
        })
        .collect();
    assert_eq!(nodes, [1, 2, 3]);
    assert!(ending_at(&stops, 1).is_some_and(|(stop, rooms)| stop.byte() == 0 && rooms.is_empty()));
    assert!(ending_at(&stops, 3).is_none());
}
