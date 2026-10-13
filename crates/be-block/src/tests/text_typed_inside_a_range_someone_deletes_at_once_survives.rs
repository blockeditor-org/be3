use super::*;

#[test]
fn text_typed_inside_a_range_someone_deletes_at_once_survives() {
    let base = TextBlock::of("hello brave new world");
    let typed = TextBlock::insert(&base, 2, 8, b"XY").expect("the insert lands");
    let deleted = TextBlock::delete(&base, 6..16).expect("there is something to delete");

    let text = TextBlock::text(&edited(&base, [typed, deleted]));

    assert_eq!(text, "hello XYworld");
}
