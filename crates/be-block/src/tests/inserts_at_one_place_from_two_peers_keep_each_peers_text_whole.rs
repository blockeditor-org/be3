use super::*;

#[test]
fn inserts_at_one_place_from_two_peers_keep_each_peers_text_whole() {
    let base = "hello world";
    let theirs = [TextOp::insert(5, " there"), TextOp::insert(11, ",")];
    let ours = TextContent::rebase(TextOp::insert(5, " again"), &theirs).unwrap();

    let text = applied(base, &[theirs[0].clone(), theirs[1].clone(), ours]);

    assert!(
        text == "hello there, again world" || text == "hello again there, world",
        "each insert stays in one piece: {text}"
    );
}
