use super::*;
use beui::reactive::Get;

const SHOWN: &str = "one two\tthree `a b\tc`  end  \nnext\r line\n";

fn area(editor: &BeuiTest<TextApp>) -> beui::styled::TextAreaState {
    let document = editor.document();
    let area = document
        .find_test_id("text.surface")
        .expect("the text is drawn");
    let surface = document
        .api::<beui::styled::TextAreaSurfaceApi>(area)
        .surface();
    document.api::<beui::styled::TextAreaApi>(surface).state()
}

fn carets(editor: &BeuiTest<TextApp>) -> Vec<Option<beui::Rect>> {
    let layout = area(editor).layout().get_untracked();
    (0..=SHOWN.len())
        .map(|byte| layout.caret_rect(byte))
        .collect()
}

#[test]
fn selecting_text_does_not_move_it() {
    let mut editor = editor(SHOWN);
    editor.click("text.surface");
    let before = carets(&editor);
    assert!(before.iter().all(Option::is_some), "every byte has a caret");

    for range in [0..SHOWN.len(), 3..20, 25..33] {
        area(&editor).select(range.clone());
        editor.run();
        assert_eq!(
            carets(&editor),
            before,
            "selecting {range:?} moves no caret position"
        );
    }
}
