use super::*;

#[test]
fn the_characters_around_a_byte_are_found_across_text_nodes() {
    let stops = stops_of(&[(1, "ab"), (2, "é")]);
    let utf16 = |stop: Option<&Stop<u32>>| match stop {
        Some(Stop::Char { node, utf16, .. }) => Some((*node, *utf16)),
        _ => None,
    };
    assert_eq!(utf16(after(&stops, 0)), Some((1, 0)));
    assert_eq!(utf16(after(&stops, 2)), Some((2, 0)));
    assert_eq!(utf16(before(&stops, 2)), Some((1, 1)));
    assert_eq!(utf16(before(&stops, 4)), Some((2, 0)));
    assert_eq!(utf16(after(&stops, 4)), None);
    assert_eq!(utf16(after(&stops, 3)), None);
}
