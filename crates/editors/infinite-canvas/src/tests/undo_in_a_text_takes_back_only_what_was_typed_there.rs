use super::*;
use block_editor_beui::BlockCommand;
use block_editor_beui::be_block::canvas::CanvasTextStyle;
use block_editor_beui::beui::{Key, Modifiers, pos2};

#[test]
fn undo_in_a_text_takes_back_only_what_was_typed_there() {
    let mut label = entity(Uuid::from_u128(1));
    label.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(220.0, 40.0), 0.0);
    label.kind = CanvasEntityKind::Text {
        text: "Hello".to_owned(),
        text_style: CanvasTextStyle::default(),
        placeholder: String::new(),
    };
    let mut editor = editor(std::slice::from_ref(&label));
    let field = format!("infinite-canvas.text.{}", label.id);
    let resting = editor.rect_of(&field);
    editor.double_click_at(pos2(resting.right() - 2.0, resting.center().y));
    editor.run();
    editor.text("!");
    editor.run();
    peer_types(&mut editor, label.id, 0, "Oh, ");
    editor.run();
    editor.take_block_commands();

    editor.key_press_modifiers(Modifiers::CTRL, Key::Z);
    editor.run();

    let block = editor.block_id().expect("the canvas has a block");
    assert_eq!(
        editor.take_block_commands(),
        [(block, BlockCommand::Undo)],
        "undo in a text is the canvas's undo, which takes back only this peer's edits"
    );
    let CanvasEntityKind::Text { text, .. } = &entities(&editor)[0].kind else {
        panic!("the entity is still text");
    };
    assert_eq!(text, "Oh, Hello!", "the field undoes nothing on its own");
}
