use super::*;

#[test]
fn loaded_items_read_back_as_one_fragment() {
    let sequence = loaded("hello");

    assert_eq!(text(&sequence), "hello");
    assert_eq!(sequence.fragment_count(), 1);
    assert!(sequence.is_fresh());
    assert_eq!(
        sequence.pos(4),
        Some(Pos {
            client: LOADED,
            offset: 4
        })
    );
    assert_eq!(sequence.next_offset(LOADED), 5);
}
