use super::*;

#[test]
fn ime_replace_text_swaps_a_typed_word_for_a_suggestion() {
    let mut tester = EditorTester::new("say helo there");
    tester.set_cursor(tester.pos(14));

    tester.execute(EditorCommand::Ime(ImeCommand::ReplaceText {
        range: 4..8,
        text: "hello",
    }));

    tester.expect_content("say hello| there");
}
