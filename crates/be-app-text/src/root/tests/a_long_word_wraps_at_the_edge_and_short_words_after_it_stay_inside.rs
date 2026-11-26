use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn a_long_word_wraps_at_the_edge_and_short_words_after_it_stay_inside() {
    let mut editor: BeuiTest<TextRoot> =
        BeuiTest::editing(Uuid::new_v4()).with_size(Vec2::new(360.0, 400.0));
    editor.hold(None, TextBlock::of(""));
    editor.run();
    editor.click("text.surface");
    editor.type_text("abcd ");
    editor.record();
    for _ in 0..8 {
        editor.type_text("eeeeeeeeee");
        editor.record();
    }
    for _ in 0..4 {
        editor.type_text(" abcd abcd abcd");
        editor.record();
    }
    editor.key(Key::Enter, Modifiers::NONE);
    editor.type_text("see /usr/local/share/applications/somewhere/deep_inside/the.tree:parts");
    editor.record();

    editor.snapshot("a_long_word_wraps_at_the_edge_and_short_words_after_it_stay_inside");
}
