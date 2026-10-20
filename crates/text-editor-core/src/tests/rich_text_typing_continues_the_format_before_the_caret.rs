use super::*;

#[test]
fn rich_text_typing_continues_the_format_before_the_caret() {
    let mut tester = EditorTester::rich(b"one two\nthree");
    select(&mut tester, 4..7);
    tool(&mut tester, "italic");
    assert_eq!(formatted(&tester), "one {italic}two{}\nthree");

    tester.set_cursor(tester.pos(7));
    tester.execute(EditorCommand::InsertText(b"s"));
    assert_eq!(formatted(&tester), "one {italic}twos{}\nthree");

    tester.set_cursor(tester.pos(4));
    tester.execute(EditorCommand::InsertText(b"x"));
    assert_eq!(
        formatted(&tester),
        "one x{italic}twos{}\nthree",
        "typing before a format takes the format before the caret"
    );

    select(&mut tester, 10..15);
    tool(&mut tester, "underline");
    tester.set_cursor(tester.pos(10));
    tester.execute(EditorCommand::InsertText(b"a "));
    assert_eq!(
        formatted(&tester),
        "one x{italic}twos{}\n{underline}a three",
        "typing at the start of a paragraph takes the format after the caret"
    );
}
