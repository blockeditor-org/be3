use super::*;
use infinite_canvas_types::{CanvasHue, CanvasTone};

#[test]
fn the_style_popover_fits_a_phone_above_the_dock() {
    let rectangle = card();
    let mut editor = phone(std::slice::from_ref(&rectangle));

    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    editor.click("infinite-canvas.style");
    editor.run();
    let panel = editor.rect_of("infinite-canvas.style.panel");
    assert!(
        editor.rect().contains_rect(panel),
        "the popover fits the phone"
    );
    assert!(
        panel.bottom() <= editor.rect_of("infinite-canvas.selection-bar").top(),
        "the popover opens above the selection bar"
    );

    editor.click("infinite-canvas.stroke.Blue");
    editor.run();
    editor.snapshot("the_style_popover_fits_a_phone_above_the_dock");
    let [restyled] = entities(&editor).try_into().unwrap();
    assert_eq!(
        restyled.style.stroke_color,
        CanvasColor::Palette {
            hue: CanvasHue::Blue,
            tone: CanvasTone::Strong
        }
    );
}
