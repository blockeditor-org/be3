use super::*;

#[test]
fn rich_text_a_heading_covers_its_line_and_enter_at_its_end_starts_a_body_line() {
    let mut tester = EditorTester::with_language(b"Title\nbody", TextLanguage::RichText);
    tester.set_cursor(tester.pos(2));
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Heading(1)));
    assert_eq!(formatted(&tester), "{h1}Title\n{}body");
    assert_eq!(tester.editor.selection_format().unwrap().heading, 1);

    tester.set_cursor(tester.pos(5));
    tester.execute(EditorCommand::Newline);
    tester.execute(EditorCommand::InsertText(b"next"));
    assert_eq!(formatted(&tester), "{h1}Title\n{}next\nbody");

    tester.set_cursor(tester.pos(2));
    tester.execute(EditorCommand::Newline);
    assert_eq!(
        formatted(&tester),
        "{h1}Ti\ntle\n{}next\nbody",
        "splitting a heading keeps both halves headings"
    );

    tester.execute(EditorCommand::SelectAll);
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Heading(2)));
    assert_eq!(formatted(&tester), "{h2}Ti\ntle\nnext\nbody");
}
