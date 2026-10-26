use super::*;

fn note_text(test: &DocumentTest) -> String {
    let document = test.document();
    let area = document
        .find_test_id("demo.read-only-note")
        .expect("the note is drawn");
    let surface = document
        .api::<beui::styled::TextAreaSurfaceApi>(area)
        .surface();
    let state = document.api::<beui::styled::TextAreaApi>(surface).state();
    String::from_utf8_lossy(&state.bytes()).into_owned()
}

#[test]
fn a_read_only_text_area_keeps_its_text_when_typed_into() {
    let mut test = alone(WIDE, Page::Editor);
    reveal(&mut test, "demo.read-only-note");
    let before = note_text(&test);
    test.click("demo.read-only-note");
    test.frame(vec![Input::Text("typed ".to_owned())]);
    press_key(&mut test, beui::Key::Backspace);
    press_key(&mut test, beui::Key::Enter);
    assert_eq!(
        note_text(&test),
        before,
        "typing into read-only text leaves it as it was"
    );
    test.snapshot("a_read_only_text_area_keeps_its_text_when_typed_into");
}
