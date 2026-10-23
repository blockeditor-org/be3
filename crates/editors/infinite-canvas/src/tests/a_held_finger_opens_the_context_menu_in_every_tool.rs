use super::*;
use block_editor_beui::beui::TouchPhase;

#[test]
fn a_held_finger_opens_the_context_menu_in_every_tool() {
    for tool in [
        "Select",
        "Pen",
        "Rectangle",
        "Ellipse",
        "Diamond",
        "Arrow",
        "Line",
        "Text",
        "Frame",
        "Eraser",
    ] {
        let rectangle = card();
        let mut editor = vector_phone(std::slice::from_ref(&rectangle));
        editor.tap(format!("infinite-canvas.dock.tool.{tool}"));
        editor.run();
        let center = editor
            .rect_of(&format!("infinite-canvas.entity.{}", rectangle.id))
            .center();

        editor.fingers(TouchPhase::Start, &[(1, center)]);
        editor.run();
        editor.advance(std::time::Duration::from_secs(1));
        editor.fingers(TouchPhase::End, &[(1, center)]);
        editor.run();
        assert!(
            editor.shown("infinite-canvas.menu.canvas.delete"),
            "holding a finger opens the menu with the {tool} tool"
        );
        assert_eq!(
            entities(&editor),
            vec![rectangle.clone()],
            "and adds nothing with the {tool} tool"
        );
        if tool == "Pen" {
            editor.snapshot("a_held_finger_opens_the_context_menu");
        }
        editor.tap("infinite-canvas.menu.canvas.delete");
        editor.run();
        assert!(
            entities(&editor).is_empty(),
            "the menu acts on what was held with the {tool} tool"
        );
    }
}
