use super::*;

#[test]
fn text_operations_rebase_onto_concurrent_edits() {
    let base = "hello world";

    let theirs = TextOp::insert(0, ">> ");
    let ours = TextContent::rebase(TextOp::insert(5, ","), std::slice::from_ref(&theirs)).unwrap();
    assert_eq!(applied(base, &[theirs, ours]), ">> hello, world");

    let theirs = TextOp::delete(0, 6);
    let ours = TextContent::rebase(TextOp::insert(11, "!"), std::slice::from_ref(&theirs)).unwrap();
    assert_eq!(applied(base, &[theirs, ours]), "world!");

    let theirs = TextOp::insert(5, " there");
    let ours = TextContent::rebase(TextOp::delete(6, 5), std::slice::from_ref(&theirs)).unwrap();
    assert_eq!(applied(base, &[theirs, ours]), "hello there ");

    let theirs = TextOp::delete(0, 6);
    let ours = TextContent::rebase(TextOp::delete(6, 5), std::slice::from_ref(&theirs)).unwrap();
    assert_eq!(applied(base, &[theirs, ours]), "");

    let theirs = TextOp::delete(3, 5);
    let ours = TextContent::rebase(TextOp::delete(5, 5), std::slice::from_ref(&theirs)).unwrap();
    assert_eq!(
        applied(base, &[theirs, ours]),
        "held",
        "two overlapping deletes should remove their union exactly once"
    );

    let theirs = TextOp::delete(2, 7);
    assert_eq!(
        TextContent::rebase(TextOp::delete(3, 4), &[theirs]),
        None,
        "an operation whose whole range was deleted should vanish"
    );

    let language = TextOp::SetLanguage(TextLanguage::Rust);
    assert_eq!(
        TextContent::rebase(language.clone(), &[TextOp::delete(0, 5)]),
        Some(language)
    );

    let chain = [TextOp::insert(0, "a"), TextOp::insert(0, "b")];
    let ours = TextContent::rebase(TextOp::insert(0, "c"), &chain).unwrap();
    assert_eq!(
        applied("", &[chain[0].clone(), chain[1].clone(), ours]),
        "bac"
    );
}
