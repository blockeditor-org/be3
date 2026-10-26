use super::*;
use block_editor_beui::beui::TouchPhase;

#[test]
fn tapping_selected_text_on_a_phone_edits_it() {
    let mut label = card();
    label.shape = CanvasShape::Text;
    label.text = "Hello".into();
    let mut editor = phone(std::slice::from_ref(&label));
    let at = editor
        .rect_of(&format!("infinite-canvas.entity.{}", label.id))
        .center();
    let tap = |editor: &mut BeuiTest<CanvasApp>| {
        editor.fingers(TouchPhase::Start, &[(1, at)]);
        editor.run();
        editor.fingers(TouchPhase::End, &[(1, at)]);
        editor.run();
    };

    let typed = |editor: &mut BeuiTest<CanvasApp>| {
        editor.type_text("!");
        editor.run();
        entities(editor)[0].text.to_str_lossy().into_owned()
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
