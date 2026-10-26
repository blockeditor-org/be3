use super::*;

const LINE: &str = "one two\tthree `a b\tc d` end  ";

#[test]
fn whitespace_markers_sit_over_real_spaces() {
    let mut editor = editor(&format!("{LINE}\n{LINE}\n"));
    editor.click("text.surface");
    editor.run();
    let document = editor.document();
    let area = document
        .find_test_id("text.surface")
        .expect("the text is drawn");
    let surface = document
        .api::<beui::styled::TextAreaSurfaceApi>(area)
        .surface();
    let state = document.api::<beui::styled::TextAreaApi>(surface).state();
    state.select(0..LINE.len() + 1);
    drop(document);
    editor.run();

    editor.snapshot("whitespace_markers_sit_over_real_spaces");
}
