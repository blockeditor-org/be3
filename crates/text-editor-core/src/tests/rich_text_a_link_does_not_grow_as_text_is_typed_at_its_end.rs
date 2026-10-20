use super::*;

#[test]
fn rich_text_a_link_does_not_grow_as_text_is_typed_at_its_end() {
    let mut tester = EditorTester::rich(b"see docs");
    select(&mut tester, 4..8);
    tester.execute(EditorCommand::SetMark(
        Name::new("link"),
        Some(Value::text("https://example.com")),
    ));
    tester.set_cursor(tester.pos(8));
    tester.execute(EditorCommand::InsertText(b" now"));
    tester.set_cursor(tester.pos(6));
    tester.execute(EditorCommand::InsertText(b"-"));
    assert_eq!(
        formatted(&tester),
        "see {link=https://example.com}do-cs{} now",
        "text typed inside a link joins it, text typed at its end does not"
    );
}
