use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn the_palette_and_the_shortcuts_run_the_canvas_actions() {
    let rectangle = card();
    let mut editor = editor(std::slice::from_ref(&rectangle));
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };

    editor.click(&format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    editor.key_press_modifiers(
        Modifiers {
            shift: true,
            ..ctrl
        },
        Key::P,
    );
    editor.run();
    editor.text("duplicate");
    editor.run();
    editor.key_press(Key::Enter);
    editor.run();
    assert_eq!(
        entities(&editor).len(),
        2,
        "the palette duplicates the selection"
    );

    editor.key_press_modifiers(ctrl, Key::A);
    editor.key_press_modifiers(ctrl, Key::G);
    editor.run();
    let groups: HashSet<_> = entities(&editor)
        .iter()
        .map(|entity| entity.group_id)
        .collect();
    assert_eq!(groups.len(), 1, "Ctrl+A then Ctrl+G groups everything");
    assert!(groups.iter().all(Option::is_some));
}
