use super::*;

#[test]
fn utf16_offsets_in_a_text_node_map_to_the_utf8_bytes_of_the_whole_text() {
    let stops = stops_of(&[(1, "aé"), (2, "😀b")]);
    assert_eq!(byte_at(&stops, |node| *node == 1, 0), Some(0));
    assert_eq!(byte_at(&stops, |node| *node == 1, 1), Some(1));
    assert_eq!(byte_at(&stops, |node| *node == 1, 2), Some(3));
    assert_eq!(byte_at(&stops, |node| *node == 2, 0), Some(3));
    assert_eq!(byte_at(&stops, |node| *node == 2, 2), Some(7));
    assert_eq!(byte_at(&stops, |node| *node == 2, 3), Some(8));
    assert_eq!(byte_at(&stops, |node| *node == 3, 0), None);
}
