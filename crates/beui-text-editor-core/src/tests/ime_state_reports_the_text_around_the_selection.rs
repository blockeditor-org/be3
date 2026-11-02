use super::*;

#[test]
fn ime_state_reports_the_text_around_the_selection() {
    let mut tester = EditorTester::new("say ");
    tester.set_cursor(tester.pos(4));
    tester.execute(EditorCommand::Ime(ImeCommand::SetComposingText("helo")));
    tester.execute(EditorCommand::Ime(ImeCommand::SetSelection {
        anchor: 5,
        focus: 7,
    }));

    assert_eq!(
        tester.editor.ime_state(),
        Some(ImeState {
            start: 0,
            text: "say helo".to_owned(),
            selection: 5..7,
            composing: Some(4..8),
        })
    );
}
