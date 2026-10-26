use super::*;

#[test]
fn markdown_checkbox_toggle() {
    let mut tester = EditorTester::with_language(b"- [ ] task", TextLanguage::Markdown);
    tester.execute(EditorCommand::Markdown(MarkdownCommand::ToggleCheckbox(
        tester.pos(0),
    )));
    tester.expect_content(b"- [x] task");

    tester.set_cursor(tester.pos(usize::MAX));
    tester.execute(EditorCommand::Newline);
    tester.expect_content(b"- [x] task\n- [ ] |");

    let mut quoted = EditorTester::with_language(b"> 1. [x] quoted", TextLanguage::Markdown);
    quoted.execute(EditorCommand::Markdown(MarkdownCommand::ToggleCheckbox(
        quoted.pos(0),
    )));
    quoted.expect_content(b"> 1. [ ] quoted");
}
