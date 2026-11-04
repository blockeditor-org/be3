use super::*;

#[test]
fn rich_text_enter_continues_a_list_and_lifts_out_of_an_empty_item() {
    let mut tester = EditorTester::rich(b"milk");
    tool(&mut tester, "check");
    assert_eq!(formatted(&tester), "[list=check]milk");
    tester.execute(EditorCommand::SetBlock {
        at: Some(tester.pos(0)),
        key: Name::new("checked"),
        value: Some(Value::Bool(true)),
    });
    assert_eq!(formatted(&tester), "[checked,list=check]milk");

    tester.set_cursor(tester.pos(4));
    tester.execute(EditorCommand::Newline);
    tester.execute(EditorCommand::InsertText(b"eggs"));
    assert_eq!(
        formatted(&tester),
        "[checked,list=check]milk\n[list=check]eggs",
        "a new item starts unchecked"
    );

    tester.execute(EditorCommand::Newline);
    tester.execute(EditorCommand::Newline);
    tester.execute(EditorCommand::InsertText(b"done"));
    assert_eq!(
        formatted(&tester),
        "[checked,list=check]milk\n[list=check]eggs\ndone",
        "Enter on an empty item lifts it out of the list"
    );
}
