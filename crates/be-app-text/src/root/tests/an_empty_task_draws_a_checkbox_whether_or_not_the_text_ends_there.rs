use super::editor;
use be_sdk_test::prelude::*;

/// A task with nothing after its box yet, as Enter leaves one, draws a checkbox, on the last
/// line or before others.
#[test]
fn an_empty_task_draws_a_checkbox_whether_or_not_the_text_ends_there() {
    let mut editor = editor("- [ ] \n\n- [x] ");
    editor.run();

    assert!(editor.shown("text.checkbox.0"));
    assert!(editor.shown("text.checkbox.2"));
    editor.snapshot("an_empty_task_draws_a_checkbox_whether_or_not_the_text_ends_there");
}
