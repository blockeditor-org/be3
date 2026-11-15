use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn a_long_word_wraps_at_the_edge_and_short_words_after_it_stay_inside() {
    let mut editor: BeuiTest<TextRoot> =
        BeuiTest::editing(Uuid::new_v4()).with_size(Vec2::new(360.0, 400.0));
    editor.hold(None, TextBlock::of(""));
    editor.run();
    editor.click("text.surface");
    editor.run();
    editor.type_text("abcd ");
    editor.run();
    editor.record();
    for _ in 0..8 {
        editor.type_text("eeeeeeeeee");
        editor.run();
        editor.record();
    }
    for _ in 0..4 {
        editor.type_text(" abcd abcd abcd");
        editor.run();
        editor.record();
    }
    editor.key(Key::Enter, Modifiers::NONE);
    editor.type_text("see /usr/local/share/applications/somewhere/deep_inside/the.tree:parts");
    editor.run();
    editor.record();

    editor.snapshot("a_long_word_wraps_at_the_edge_and_short_words_after_it_stay_inside");
}
