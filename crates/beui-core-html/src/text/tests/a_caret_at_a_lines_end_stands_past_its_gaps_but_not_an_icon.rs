use super::*;

/// A caret at the end of a line stands after the last character and the gaps right after it,
/// as a rule's last share or a table's closing room, but before an icon, which holds no bytes
/// either.
#[test]
fn a_caret_at_a_lines_end_stands_past_its_gaps_but_not_an_icon() {
    let mut stops = stops_of(&[(1, "ab")]);
    stops.push(Stop::Room { byte: 2, node: 2 });
    stops.push(Stop::Room { byte: 2, node: 3 });
    stops.push(Stop::Atom {
        byte: 2,
        len: 0,
        node: 4,
        newline: false,
    });
    stops.push(Stop::Room { byte: 2, node: 5 });
    let nodes: Vec<u32> = ending_at(&stops, 2)
        .into_iter()
        .map(|stop| match stop {
            Stop::Char { node, .. } | Stop::Atom { node, .. } | Stop::Room { node, .. } => *node,
        })
        .collect();
    assert_eq!(nodes, [1, 2, 3]);
    assert!(ending_at(&stops, 1).first().is_some_and(|stop| stop.byte() == 0));
    assert!(ending_at(&stops, 3).is_empty());
}
