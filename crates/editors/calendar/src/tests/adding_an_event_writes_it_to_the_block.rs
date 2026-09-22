use super::*;

#[test]
fn adding_an_event_writes_it_to_the_block() {
    let mut calendar = Harness::new();

    calendar.editor.click("calendar.add-event");
    calendar.run();
    calendar.editor.click("calendar.form.title");
    calendar.run();
    calendar.editor.text("Stand up");
    calendar.run();
    calendar.editor.click("calendar.form.save");
    calendar.run();

    let titles: Vec<&str> = calendar
        .content
        .events()
        .iter()
        .map(|event| event.title.as_str())
        .collect();
    assert_eq!(titles, ["Stand up"]);
    assert!(!calendar.editor.shown("calendar.form.title"));
}
