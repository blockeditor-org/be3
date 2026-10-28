use super::*;
use block_editor_beui::beui::Vec2;
use infinite_canvas_types::{CanvasHue, CanvasLook, CanvasTone};

#[test]
fn a_new_shape_takes_the_last_style_used() {
    let mut editor = editor(&[]);

    editor.click("infinite-canvas.dock.tool-lock");
    editor.click("infinite-canvas.dock.tool.Rectangle");
    assert!(
        editor.shown("infinite-canvas.style"),
        "with nothing selected the style is for the next shape"
    );
    editor.click("infinite-canvas.style");
    editor.click("infinite-canvas.stroke.Green");
    editor.click("infinite-canvas.width.Thin");
    editor.click("infinite-canvas.style");

    let canvas = editor.rect_of("infinite-canvas.canvas");
    let from = canvas.center() - Vec2::new(150.0, 60.0);
    editor.drag(from, from + Vec2::new(120.0, 80.0));

    let [first] = entities(&editor).try_into().unwrap();
    let green = CanvasColor::Palette {
        hue: CanvasHue::Green,
        tone: CanvasTone::Strong,
    };
    assert_eq!(first.style.stroke_color, green);
    assert_eq!(first.style.stroke_width, 1.0);
    assert_eq!(
        first.style.look,
        CanvasLook::HandDrawn,
        "prototyping draws by hand by default"
    );

    editor.click("infinite-canvas.style");
    editor.click("infinite-canvas.roughness.Clean");
    editor.click("infinite-canvas.style");
    editor.click("infinite-canvas.dock.tool.Ellipse");
    let from = canvas.center() + Vec2::new(30.0, -40.0);
    editor.drag(from, from + Vec2::new(120.0, 80.0));

    let drawn = entities(&editor);
    assert_eq!(drawn.len(), 2);
    let second = &drawn[1];
    assert_eq!(second.shape, CanvasShape::Ellipse);
    assert_eq!(second.style.stroke_color, green);
    assert_eq!(second.style.stroke_width, 1.0);
    assert_eq!(second.style.look, CanvasLook::Clean);
    assert_eq!(
        entities(&editor)[0].style.look,
        CanvasLook::HandDrawn,
        "restyling the selection leaves other shapes alone"
    );
    editor.snapshot("a_new_shape_takes_the_last_style_used");
}
