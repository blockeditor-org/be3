use super::*;
use block_editor_beui::be_block::canvas::CanvasTextStyle;
use block_editor_beui::beui::{Key, Modifiers, pos2};

fn retyped(label: &CanvasEntity, text: &str) -> CanvasEntity {
    let mut label = label.clone();
    if let CanvasEntityKind::Text { text: held, .. } = &mut label.kind {
        *held = text.to_owned();
    }
    label
}

fn text_of(editor: &BeuiTest<CanvasApp>) -> String {
    match &entities(editor)[0].kind {
        CanvasEntityKind::Text { text, .. } => text.clone(),
        _ => panic!("the entity is still text"),
    }
}

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

    apply(
        &mut editor,
        InfiniteCanvasOperation::Update {
            entities: vec![retyped(&label, "Oh, Hello!")],
        },
    );
    editor.run();
    assert_eq!(text_of(&editor), "Oh, Hello!");

    editor.key_press_modifiers(Modifiers::CTRL, Key::Z);
    editor.run();
    assert_eq!(
        text_of(&editor),
        "Oh, Hello",
        "undo takes back what was typed here, and leaves what a peer typed"
    );

    editor.key_press_modifiers(Modifiers::CTRL, Key::Z);
    editor.run();
    assert_eq!(
        text_of(&editor),
        "Oh, Hello",
        "there is nothing more of ours to undo"
    );

    editor.key_press_modifiers(
        Modifiers {
            shift: true,
            ..Modifiers::CTRL
        },
        Key::Z,
    );
    editor.run();
    assert_eq!(text_of(&editor), "Oh, Hello!", "redo puts it back");
}
