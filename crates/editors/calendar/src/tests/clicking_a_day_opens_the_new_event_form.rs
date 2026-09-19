use super::*;

#[test]
fn clicking_a_day_opens_the_new_event_form() {
    let (mut editor, _) = editor();

    assert!(!editor.shown("calendar.form.title"));
    editor.click("calendar.add-event");
    editor.run();

    assert!(editor.shown("calendar.form.title"));
    editor.click("calendar.form.cancel");
    editor.run();
    assert!(!editor.shown("calendar.form.title"));
}
