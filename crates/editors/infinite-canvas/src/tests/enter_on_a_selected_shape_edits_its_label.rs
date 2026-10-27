use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn enter_on_a_selected_shape_edits_its_label() {
    let mut editor = editor(&[card()]);
    let id = card().id;

    editor.click(format!("infinite-canvas.entity.{id}"));
    editor.key(Key::Enter, Modifiers::NONE);
    editor.type_text("Plan ");
    editor.key(Key::B, Modifiers::CTRL);
    editor.type_text("now");
    editor.snapshot("a_label_is_edited_in_its_shape");
    editor.key(Key::Escape, Modifiers::NONE);

    let label = &entities(&editor)[0];
    assert_eq!(
        label.shape,
        CanvasShape::Rectangle,
        "the shape stays a shape"
    );
    assert_eq!(label.text.to_str_lossy(), "Plan now");
    assert!(
        !label.text.seeds().is_empty(),
        "the shortcut makes what follows bold, as in the text editor"
    );
    editor.snapshot("a_shape_draws_its_rich_label");

    editor.double_click(format!("infinite-canvas.entity.{id}"));
    editor.key(Key::A, Modifiers::CTRL);
    editor.type_text("Again");
    editor.key(Key::Escape, Modifiers::NONE);
    assert_eq!(
        entities(&editor)[0].text.to_str_lossy(),
        "Again",
        "a double-click on a shape edits its label too"
    );
}
