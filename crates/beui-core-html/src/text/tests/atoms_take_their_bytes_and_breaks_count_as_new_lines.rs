use super::*;

#[test]
fn atoms_take_their_bytes_and_breaks_count_as_new_lines() {
    let mut stops = stops_of(&[(1, "ab")]);
    stops.push(Stop::Atom {
        byte: 2,
        len: 0,
        node: 2,
        newline: true,
    });
    stops.push(Stop::Atom {
        byte: 2,
        len: 3,
        node: 3,
        newline: false,
    });
    assert!(stops[2].breaks());
    assert!(matches!(after(&stops, 2), Some(Stop::Atom { node: 3, .. })));
    assert!(matches!(
        before(&stops, 5),
        Some(Stop::Atom { node: 3, .. })
    ));
    assert!(matches!(
        before(&stops, 2),
        Some(Stop::Char { ch: 'b', .. })
    ));
}
