use super::*;

#[test]
fn rich_text_enter_at_the_start_of_a_checked_item_keeps_it_checked() {
    let mut tester = EditorTester::rich(b"intro\ndone");
    tester.set_cursor(tester.pos(7));
    tool(&mut tester, "check");
    tester.execute(EditorCommand::SetBlock {
        at: None,
        key: Name::new("checked"),
        value: Some(Value::Bool(true)),
    });
    tester.set_cursor(tester.pos(6));
    tester.execute(EditorCommand::Newline);
    assert_eq!(
        formatted(&tester),
        "intro\n[list=check]\n[checked,list=check]done",
        "the new empty item above is unchecked and the moved item keeps its check"
    );
    tester.expect_content(b"intro\n\n|done");
}
