use super::*;

const LINE: &str = "afdjnafndjsk fadsnjknjafkdsl nfjakdjnklafds njafdnjlkafds njkafdsnjlkafds njkafdnjlkfads jnkafldsnlj a";
const HEIGHT: f32 = 600.0;

fn state(editor: &BeuiTest<TextRoot>) -> beui::unstyled::TextAreaState {
    let document = editor.document();
    let area = document
        .find_test_id("text.surface")
        .expect("the text is drawn");
    let surface = document
        .api::<beui::styled::TextAreaSurfaceApi>(area)
        .surface();
    document.api::<beui::styled::TextAreaApi>(surface).state()
}

/// Where Down takes a caret at `from`.
fn down_from(editor: &mut BeuiTest<TextRoot>, from: usize) -> usize {
    state(editor).select(from..from);
    editor.run();
    caret_after(editor, Key::ArrowDown) as usize
}

/// Whether each line is one row at a view `width` wide: Down from a line's start leaves the
/// line, or for the last line, goes to the text's end rather than to a second row.
fn one_row(editor: &mut BeuiTest<TextRoot>, width: f32) -> (bool, bool) {
    let second = LINE.len() + 1;
    editor.set_size(Vec2::new(width, HEIGHT));
    editor.run();
    let first = down_from(editor, 0) >= second;
    let last = down_from(editor, second) == second + LINE.len();
    (first, last)
}

#[test]
fn a_line_just_wide_enough_keeps_its_last_word_and_its_newline() {
    // The first line ends in a newline, whose marker's room follows its last word; the second
    // ends the text and has none. At the least width the second fits on one row, so must the
    // first.
    let mut editor = editor(&format!("{LINE}\n{LINE}"));
    editor.presence_visible(true);
    editor.click("text.surface");
    let (mut narrow, mut wide) = (400.0, 1400.0);
    assert!(one_row(&mut editor, wide).1, "the line fits a wide view");
    assert!(!one_row(&mut editor, narrow).1, "the line wraps in a narrow one");
    while wide - narrow > 0.25 {
        let middle = (narrow + wide) / 2.0;
        match one_row(&mut editor, middle).1 {
            true => wide = middle,
            false => narrow = middle,
        }
    }
    assert_eq!(
        one_row(&mut editor, wide),
        (true, true),
        "at {wide}, the least width the last line fits, the first line fits too"
    );

    state(&editor).select(LINE.len()..LINE.len());
    editor.run();
    editor.record();
    state(&editor).select(LINE.len() - 1..LINE.len() + 1);
    editor.run();
    editor.record();
    editor.snapshot("a_line_just_wide_enough_keeps_its_last_word_and_its_newline");
}
