use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn a_long_word_wraps_at_the_edge_and_short_words_after_it_stay_inside() {
    let mut editor = editor("");
    editor.resize(Vec2::new(360.0, 320.0));
    editor.click("text.surface");
    editor.run();
    editor.type_text("abcd ");
    editor.run();
    editor.record();
    for _ in 0..6 {
        editor.type_text("eeeeeeeeee");
        editor.run();
        editor.record();
    }
    for _ in 0..4 {
        editor.type_text(" abcd abcd abcd");
        editor.run();
        editor.record();
    }
    editor.type_text("\nsee /usr/local/share/applications/somewhere/deep_inside/the.tree:parts");
    editor.run();

    editor.snapshot("a_long_word_wraps_at_the_edge_and_short_words_after_it_stay_inside");
}
