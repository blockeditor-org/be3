use super::*;

#[test]
fn checkboxes_are_drawn_when_the_text_arrives_after_the_editor() {
    let mut editor = BeuiTest::<TextRoot>::editing(Uuid::new_v4());
    editor.run();
    editor.hold(
        None,
        TextBlock::of("# Tasks\n\n- [ ] buy milk\n- [x] water the plants\n"),
    );
    editor.run();

    assert!(editor.shown("text.checkbox.2"));
    assert!(editor.shown("text.checkbox.3"));
    editor.snapshot("checkboxes_are_drawn_when_the_text_arrives_after_the_editor");
}
