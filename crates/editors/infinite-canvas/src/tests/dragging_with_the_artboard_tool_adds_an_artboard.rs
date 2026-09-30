use super::*;
use block_editor_beui::beui::{Key, Vec2};

#[test]
fn dragging_with_the_artboard_tool_adds_an_artboard() {
    let mut editor = editor(&[card()]).with_top_bar(false);
    assert!(
        entities(&editor).iter().all(|entity| !entity.is_artboard()),
        "a canvas has no artboard until one is drawn"
    );

    editor.click("editor.menu");
    editor.run();
    for key in [Key::ArrowUp, Key::ArrowUp, Key::Enter] {
        editor.key_press(key);
        editor.run();
    }
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let from = canvas.center() - Vec2::new(200.0, 150.0);
    editor.drag(from, from + Vec2::new(300.0, 200.0));
    editor.run();

    let artboards: Vec<_> = entities(&editor)
        .into_iter()
        .filter(|entity| entity.is_artboard())
        .collect();
    assert_eq!(artboards.len(), 1, "the drag adds one artboard");
    assert_eq!(
        artboards[0].kind,
        CanvasEntityKind::Artboard {
            name: "Artboard 1".to_owned()
        }
    );
    assert_eq!(artboards[0].transform.size, CanvasPoint::new(300.0, 200.0));
    editor.snapshot("dragging_with_the_artboard_tool_adds_an_artboard");
}
