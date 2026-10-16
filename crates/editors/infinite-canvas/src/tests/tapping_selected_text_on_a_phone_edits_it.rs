use super::*;
use block_editor_beui::be_block::canvas::CanvasTextStyle;
use block_editor_beui::beui::TouchPhase;

#[test]
fn tapping_selected_text_on_a_phone_edits_it() {
    let mut label = card();
    label.kind = CanvasEntityKind::Text {
        text: "Hello".to_owned(),
        text_style: CanvasTextStyle::default(),
        placeholder: String::new(),
    };
    let mut editor = phone(std::slice::from_ref(&label));
    let at = editor
        .rect_of(&format!("infinite-canvas.entity.{}", label.id))
        .center();
    let tap = |editor: &mut BeuiTest<CanvasApp>| {
        editor.finger(1, TouchPhase::Start, at);
        editor.run();
        editor.finger(1, TouchPhase::End, at);
        editor.run();
    };

    let typed = |editor: &mut BeuiTest<CanvasApp>| {
        editor.text("!");
        editor.run();
        match &entities(editor)[0].kind {
            CanvasEntityKind::Text { text, .. } => text.clone(),
            _ => panic!("the entity is still text"),
        }
    };

    tap(&mut editor);
    assert_eq!(typed(&mut editor), "Hello", "the first tap selects");

    tap(&mut editor);
    assert_ne!(
        typed(&mut editor),
        "Hello",
        "a second tap opens the text to edit where it is drawn"
    );
}
