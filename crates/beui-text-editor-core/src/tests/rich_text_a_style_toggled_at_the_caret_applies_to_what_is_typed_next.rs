use super::*;

#[test]
fn rich_text_a_style_toggled_at_the_caret_applies_to_what_is_typed_next() {
    let mut tester = EditorTester::rich(b"plain ");
    tester.set_cursor(tester.pos(6));
    tool(&mut tester, "bold");
    assert_eq!(
        formatted(&tester),
        "plain ",
        "toggling at a caret changes no text"
    );
    assert!(active(&tester, "bold"));

    tester.execute(EditorCommand::InsertText(b"bo"));
    tester.execute(EditorCommand::InsertText(b"ld"));
    assert_eq!(formatted(&tester), "plain {bold}bold");

    tool(&mut tester, "bold");
    tester.execute(EditorCommand::MoveCursorLeftRight {
        mode: MoveMode::Move,
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    assert!(
        active(&tester, "bold"),
        "moving the caret forgets a style toggled at it"
    );
}
