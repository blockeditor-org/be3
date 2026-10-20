use super::*;

#[test]
fn rich_text_backspace_at_a_block_start_lifts_its_props() {
    let mut tester = EditorTester::rich(b"intro\nitem");
    tester.set_cursor(tester.pos(8));
    tool(&mut tester, "bullet");
    assert_eq!(formatted(&tester), "intro\n[list=bullet]item");

    tester.set_cursor(tester.pos(6));
    let backspace = || EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    };
    tester.execute(backspace());
    assert_eq!(
        formatted(&tester),
        "intro\nitem",
        "the first backspace lifts the item"
    );
    tester.execute(backspace());
    assert_eq!(formatted(&tester), "introitem", "the next joins the lines");
}
