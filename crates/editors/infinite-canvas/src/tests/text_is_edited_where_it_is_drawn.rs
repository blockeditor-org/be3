use super::*;
use block_editor_beui::beui::{Key, Modifiers, pos2};

#[test]
fn text_is_edited_where_it_is_drawn() {
    let mut label = entity(Uuid::from_u128(1));
    label.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(160.0, 40.0), 0.0);
    label.shape = CanvasShape::Text;
    label.text = "Hello".into();
    let mut editor = editor(std::slice::from_ref(&label));
    let field = format!("infinite-canvas.text.{}", label.id);
    let resting = editor.rect_of(&field);
    let node = editor.document().find_test_id(&field);

    editor.double_click(pos2(resting.right() - 2.0, resting.center().y));
    assert_eq!(
        editor.document().find_test_id(&field),
        node,
        "the text is edited in the very field that drew it"
    );
    assert_eq!(editor.rect_of(&field), resting, "nothing moves to edit");

    editor.type_text("!");
    editor.snapshot("editing");
    editor.key(Key::Escape, Modifiers::NONE);
    editor.type_text("?");

    assert_eq!(
        entities(&editor)[0].text.to_str_lossy(),
        "Hello!",
        "typing lands at the caret, and Escape stops it"
    );
}
