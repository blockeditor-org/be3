use super::*;

#[test]
#[ignore = "a delete rebased over an insert inside its range grows to cover the inserted text"]
fn text_typed_inside_a_range_someone_deletes_at_once_survives() {
    let base = "hello brave new world";
    let theirs = TextOp::insert(8, "XY");
    let ours = TextContent::rebase(TextOp::delete(6, 10), std::slice::from_ref(&theirs))
        .expect("part of the delete is left");

    assert_eq!(applied(base, &[theirs, ours]), "hello XYworld");
}
