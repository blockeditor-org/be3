use super::*;

#[test]
fn inserts_at_one_place_from_two_peers_keep_each_peers_text_whole() {
    let base = TextBlock::of("hello world");
    let mut theirs = base.clone();
    let there = TextBlock::insert(&theirs, 2, 5, b" there").expect("the insert lands");
    theirs.apply(&there);
    let comma = TextBlock::insert(&theirs, 2, 11, b",").expect("the insert lands");
    let again = TextBlock::insert(&base, 1, 5, b" again").expect("the insert lands");

    let text = TextBlock::text(&edited(&base, [there, comma, again]));

    assert_eq!(text, "hello again there, world");
}
