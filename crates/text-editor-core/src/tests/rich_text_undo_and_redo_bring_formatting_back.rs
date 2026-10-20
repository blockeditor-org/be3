use super::*;

#[test]
fn rich_text_undo_and_redo_bring_formatting_back() {
    let mut tester = EditorTester::with_language(b"abcdef", TextLanguage::RichText);
    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(0),
        focus: tester.pos(4),
    });
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Strikethrough));
    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(2),
        focus: tester.pos(6),
    });
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Code));
    assert_eq!(formatted(&tester), "{s}ab{sc}cd{c}ef");

    tester.execute(EditorCommand::Undo);
    assert_eq!(formatted(&tester), "{s}abcd{}ef");
    tester.execute(EditorCommand::Undo);
    assert_eq!(formatted(&tester), "abcdef");
    tester.execute(EditorCommand::Redo);
    assert_eq!(formatted(&tester), "{s}abcd{}ef");
    tester.execute(EditorCommand::Redo);
    assert_eq!(formatted(&tester), "{s}ab{sc}cd{c}ef");

    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(1),
        focus: tester.pos(3),
    });
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    assert_eq!(formatted(&tester), "{s}a{sc}d{c}ef");
    tester.execute(EditorCommand::Undo);
    assert_eq!(
        formatted(&tester),
        "{s}ab{sc}cd{c}ef",
        "deleted text comes back with its formatting"
    );
}
