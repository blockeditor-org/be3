use super::*;

#[test]
fn clicking_a_day_opens_the_new_event_form() {
    let mut calendar = Harness::new();

    assert!(!calendar.editor.shown("calendar.form.title"));
    calendar.editor.click("calendar.add-event");
    calendar.run();

    assert!(calendar.editor.shown("calendar.form.title"));
    calendar.editor.click("calendar.form.cancel");
    calendar.run();
    assert!(!calendar.editor.shown("calendar.form.title"));
}
