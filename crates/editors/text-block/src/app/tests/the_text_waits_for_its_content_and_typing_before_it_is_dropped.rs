use super::*;

#[test]
fn the_text_waits_for_its_content_and_typing_before_it_is_dropped() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut editor = BeuiTest::<TextApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    editor.run();

    assert!(editor.shown("editor.loading"));
    assert!(!editor.shown("text.surface"));
    editor.snapshot("the_text_waits_for_its_content");
    editor.type_text("typed too early");
    editor.run();

    editor.hold(None, TextBlock::of("kept"));
    editor.run();

    assert!(!editor.shown("editor.loading"));
    assert!(editor.shown("text.surface"));
    assert_eq!(text(&editor), "kept");
}
