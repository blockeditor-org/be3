use super::*;

#[test]
fn adding_an_event_writes_it_to_the_block() {
    let (mut editor, block) = editor();

    editor.click("calendar.add-event");
    editor.run();
    editor.click("calendar.form.title");
    editor.run();
    editor.text("Stand up");
    editor.run();
    editor.click("calendar.form.save");
    editor.run();

    let titles: Vec<String> = block
        .read()
        .unwrap()
        .events()
        .iter()
        .map(|event: &CalendarEvent| event.title.clone())
        .collect();
    assert_eq!(titles, ["Stand up"]);
    assert!(!editor.shown("calendar.form.title"));
}
