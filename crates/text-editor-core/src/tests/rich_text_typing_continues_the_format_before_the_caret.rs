use super::*;

#[test]
fn rich_text_typing_continues_the_format_before_the_caret() {
    let mut tester = EditorTester::with_language(b"one two\nthree", TextLanguage::RichText);
    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(4),
        focus: tester.pos(7),
    });
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Italic));
    assert_eq!(formatted(&tester), "one {i}two{}\nthree");

    tester.set_cursor(tester.pos(7));
    tester.execute(EditorCommand::InsertText(b"s"));
    assert_eq!(formatted(&tester), "one {i}twos{}\nthree");

    tester.set_cursor(tester.pos(4));
    tester.execute(EditorCommand::InsertText(b"x"));
    assert_eq!(
        formatted(&tester),
        "one x{i}twos{}\nthree",
        "typing before a format takes the format before the caret"
    );

    tester.execute(EditorCommand::SetSelection {
        anchor: tester.pos(10),
        focus: tester.pos(15),
    });
    tester.execute(EditorCommand::ToggleStyle(RichStyle::Underline));
    tester.set_cursor(tester.pos(10));
    tester.execute(EditorCommand::InsertText(b"a "));
    assert_eq!(
        formatted(&tester),
        "one x{i}twos{}\n{u}a three",
        "typing at the start of a paragraph takes the format after the caret"
    );
}
