use super::*;
use block_editor_beui::beui::{Key, Modifiers, Vec2};

#[test]
fn a_drawing_tool_returns_to_select_unless_the_tool_is_locked() {
    let mut editor = vector_editor(&[]);
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let at =
        |column: f32, row: f32| canvas.min + Vec2::new(40.0 + column * 140.0, 100.0 + row * 140.0);
    let draw = |editor: &mut BeuiTest<CanvasApp>, from| {
        editor.drag(from, from + Vec2::new(100.0, 60.0));
        editor.run();
        entities(editor).len()
    };

    editor.click("infinite-canvas.dock.tool.Rectangle");
    editor.run();
    assert_eq!(draw(&mut editor, at(0.0, 0.0)), 1);
    assert_eq!(
        draw(&mut editor, at(1.0, 0.0)),
        1,
        "after one shape the tool returns to select, so the next drag selects"
    );

    editor.click("infinite-canvas.dock.tool-lock");
    editor.click("infinite-canvas.dock.tool.Ellipse");
    editor.run();
    assert_eq!(draw(&mut editor, at(0.0, 1.0)), 2);
    assert_eq!(
        draw(&mut editor, at(1.0, 1.0)),
        3,
        "a locked tool stays picked"
    );
    editor.snapshot("a_drawing_tool_returns_to_select_unless_the_tool_is_locked");

    editor.key(Key::Q, Modifiers::NONE);
    editor.run();
    assert_eq!(draw(&mut editor, at(2.0, 1.0)), 4);
    assert_eq!(
        draw(&mut editor, at(3.0, 1.0)),
        4,
        "Q unlocks the tool again"
    );
    assert!(
        entities(&editor)
            .iter()
            .skip(1)
            .all(|entity| entity.shape == CanvasShape::Ellipse)
    );
}
