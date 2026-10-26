use super::*;

#[test]
fn an_event_opens_in_a_form_wide_enough_for_its_fields_and_actions() {
    let mut calendar = Harness::new();

    calendar.editor.click("calendar.add-event");
    calendar.run();
    calendar.editor.click("calendar.form.title");
    calendar.run();
    calendar.editor.type_text("Stand up");
    calendar.run();
    calendar.editor.click("calendar.form.confirm");
    calendar.run();
    let id = calendar
        .content()
        .root()
        .events
        .last_id()
        .expect("the event was added");
    calendar.editor.click(format!("calendar.event.{id}").as_str());
    calendar.run();

    assert!(calendar.editor.shown("calendar.form.delete"));
    calendar
        .editor
        .snapshot("an_event_opens_in_a_form_wide_enough_for_its_fields_and_actions");
}
