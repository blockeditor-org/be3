use super::*;

#[test]
fn the_same_range_deleted_by_two_peers_at_once_is_deleted_once() {
    let base = TextBlock::of("hello brave new world");
    let theirs = TextBlock::delete(&base, 6..12).expect("there is something to delete");
    let ours = TextBlock::delete(&base, 6..16).expect("there is something to delete");

    assert_eq!(
        TextBlock::text(&edited(&base, [theirs.clone(), theirs.clone()])),
        "hello new world"
    );
    assert_eq!(
        TextBlock::text(&edited(&base, [theirs, ours])),
        "hello world"
    );
}
