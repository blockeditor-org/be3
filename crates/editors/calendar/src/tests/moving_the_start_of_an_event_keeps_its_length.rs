use block_editor_beui::beui::Key;

use super::*;

#[test]
fn moving_the_start_of_an_event_keeps_its_length() {
    let mut calendar = Harness::new();

    calendar.editor.click("calendar.add-event");
    calendar.run();
    calendar.editor.click("calendar.form.title");
    calendar.run();
    calendar.editor.text("Stand up");
    calendar.run();
    for _ in 0..4 {
        calendar.editor.key_press(Key::Tab);
        calendar.run();
    }
    calendar.editor.key_press(Key::ArrowUp);
    calendar.run();
    calendar.editor.click("calendar.form.save");
    calendar.run();

    let events: Vec<(i64, i64)> = calendar
        .content()
        .root()
        .events
        .iter()
        .map(|event| (event.start % 86_400, event.end - event.start))
        .collect();
    assert_eq!(events, [(10 * 3600, 3600)]);
}
