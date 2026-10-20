use super::*;

#[test]
fn rich_text_a_style_toggled_at_the_caret_applies_to_what_is_typed_next() {
    let mut tester = EditorTester::with_language(b"plain ", TextLanguage::RichText);
    tester.set_cursor(tester.pos(6));
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Bold));
    assert_eq!(
        formatted(&tester),
        "plain ",
        "toggling at a caret changes no text"
    );
    assert!(tester.editor.selection_format().unwrap().bold);

    tester.execute(EditorCommand::InsertText(b"bo"));
    tester.execute(EditorCommand::InsertText(b"ld"));
    assert_eq!(formatted(&tester), "plain {b}bold");

    tester.execute(EditorCommand::ToggleStyle(RichStyle::Bold));
    tester.execute(EditorCommand::MoveCursorLeftRight {
        mode: MoveMode::Move,
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    assert!(
        tester.editor.selection_format().unwrap().bold,
        "moving the caret forgets a style toggled at it"
    );
}
