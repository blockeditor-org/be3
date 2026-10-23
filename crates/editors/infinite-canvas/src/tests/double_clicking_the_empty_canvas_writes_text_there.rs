use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn double_clicking_the_empty_canvas_writes_text_there() {
    let mut editor = editor(&[]);
    let at = editor.rect_of("infinite-canvas.canvas").center();

    editor.double_click(at);
    editor.run();
    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();
    assert!(
        entities(&editor).is_empty(),
        "text left empty is taken away when the typing stops"
    );

    editor.advance(std::time::Duration::from_secs(1));
    editor.double_click(at);
    editor.run();
    editor.type_text("Hello");
    editor.run();
    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();
    let written = entities(&editor);
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].shape, CanvasShape::Text);
    assert_eq!(written[0].text.to_str_lossy(), "Hello");
    let field = editor.rect_of(&format!("infinite-canvas.text.{}", written[0].id));
    assert!(
        field.left() <= at.x && field.right() > at.x + 20.0,
        "the text starts where the canvas was double-clicked"
    );
}
