use super::*;

#[test]
fn rich_text_bold_toggles_over_a_selection() {
    let mut tester = EditorTester::with_language(b"hello world", TextLanguage::RichText);
    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(0),
        focus: tester.pos(5),
    });
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Bold));
    assert_eq!(formatted(&tester), "{b}hello{} world");
    tester.expect_content(b"[hello| world");
    assert!(tester.editor.selection_format().unwrap().bold);

    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(3),
        focus: tester.pos(8),
    });
    assert!(
        !tester.editor.selection_format().unwrap().bold,
        "a selection only part bold is not bold"
    );
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Bold));
    assert_eq!(formatted(&tester), "{b}hello wo{}rld");
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Bold));
    assert_eq!(formatted(&tester), "{b}hel{}lo world");
}
