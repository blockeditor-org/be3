use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn choosing_a_pixel_tool_marks_it_in_the_dock() {
    let mut editor = raster_editor(&[]);
    assert!(pressed(&editor, "infinite-canvas.dock.tool.Select"));

    editor.click("infinite-canvas.dock.tool.pixel-eraser");
    editor.run();
    assert!(pressed(&editor, "infinite-canvas.dock.tool.pixel-eraser"));
    assert!(!pressed(&editor, "infinite-canvas.dock.tool.Select"));

    let keys = [
        (Key::B, "pencil"),
        (Key::E, "pixel-eraser"),
        (Key::G, "fill"),
        (Key::I, "eyedropper"),
        (Key::C, "replace-color"),
        (Key::L, "pixel-line"),
        (Key::R, "pixel-rectangle"),
        (Key::O, "pixel-ellipse"),
        (Key::P, "pencil"),
        (Key::V, "Select"),
    ];
    for (key, tool) in keys {
        editor.key(key, Modifiers::NONE);
        editor.run();
        assert!(
            pressed(&editor, &format!("infinite-canvas.dock.tool.{tool}")),
            "{key:?} picks the {tool} in the raster profile"
        );
    }

    editor.key(Key::H, Modifiers::NONE);
    editor.key(Key::X, Modifiers::NONE);
    editor.key(Key::BracketRight, Modifiers::NONE);
    editor.run();
    assert!(pressed(&editor, "infinite-canvas.raster.mirror-horizontal"));
    assert!(pressed(&editor, "infinite-canvas.raster.filled"));
    assert_eq!(editor.label("infinite-canvas.raster.brush-size"), "2");

    editor.click("infinite-canvas.mode.prototyping");
    editor.key(Key::R, Modifiers::NONE);
    editor.run();
    assert!(
        pressed(&editor, "infinite-canvas.dock.tool.Rectangle"),
        "R is the rectangle again outside the raster profile"
    );
}
