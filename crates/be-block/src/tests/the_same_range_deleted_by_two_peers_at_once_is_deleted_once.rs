use super::*;

#[test]
fn the_same_range_deleted_by_two_peers_at_once_is_deleted_once() {
    let base = "hello brave new world";
    let theirs = TextOp::delete(6, 6);

    assert_eq!(
        TextContent::rebase(TextOp::delete(6, 6), std::slice::from_ref(&theirs)),
        None
    );
    let ours = TextContent::rebase(TextOp::delete(6, 10), std::slice::from_ref(&theirs))
        .expect("part of the delete is left");
    assert_eq!(applied(base, &[theirs, ours]), "hello world");
}
