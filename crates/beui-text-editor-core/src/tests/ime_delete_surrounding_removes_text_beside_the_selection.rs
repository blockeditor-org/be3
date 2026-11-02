use super::*;

#[test]
fn ime_delete_surrounding_removes_text_beside_the_selection() {
    let mut tester = EditorTester::new("abcdéfg");
    tester.set_cursor(tester.pos(4));

    tester.execute(EditorCommand::Ime(ImeCommand::DeleteSurrounding {
        before: 2,
        after: 2,
    }));

    tester.expect_content("ab|fg");
}
