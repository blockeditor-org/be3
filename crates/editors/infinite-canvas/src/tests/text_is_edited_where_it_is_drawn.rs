use super::*;
use block_editor_beui::be_block::canvas::CanvasTextStyle;
use block_editor_beui::beui::{Key, pos2};

#[test]
fn text_is_edited_where_it_is_drawn() {
    let mut label = entity(Uuid::from_u128(1));
    label.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(160.0, 40.0), 0.0);
    label.kind = CanvasEntityKind::Text {
        text: "Hello".to_owned(),
        text_style: CanvasTextStyle::default(),
        placeholder: String::new(),
    };
    let mut editor = editor(std::slice::from_ref(&label));
    let field = format!("infinite-canvas.text.{}", label.id);
    let resting = editor.rect_of(&field);
    let node = editor.document().find_test_id(&field);

    editor.double_click_at(pos2(resting.right() - 2.0, resting.center().y));
    editor.run();
    assert_eq!(
        editor.document().find_test_id(&field),
        node,
        "the text is edited in the very field that drew it"
    );
    assert_eq!(editor.rect_of(&field), resting, "nothing moves to edit");

    editor.text("!");
    editor.run();
    editor.snapshot("editing");
    editor.key_press(Key::Escape);
    editor.run();
    editor.text("?");
    editor.run();

    let CanvasEntityKind::Text { text, .. } = &entities(&editor)[0].kind else {
        panic!("the entity is still text");
    };
    assert_eq!(
        text, "Hello!",
        "typing lands at the caret, and Escape stops it"
    );
}
