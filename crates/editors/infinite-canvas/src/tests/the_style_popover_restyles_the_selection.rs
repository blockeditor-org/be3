use super::*;
use infinite_canvas_types::{CanvasHue, CanvasLook, CanvasStrokeStyle, CanvasTextSize, CanvasTone};

#[test]
fn the_style_popover_restyles_the_selection() {
    let rectangle = card();
    let mut editor = editor(std::slice::from_ref(&rectangle));
    assert!(
        !editor.shown("chrome.sidebar"),
        "prototyping has no inspector"
    );

    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));
    editor.click("infinite-canvas.style");
    assert!(editor.shown("infinite-canvas.style.panel"));
    assert!(
        !editor.shown("infinite-canvas.text-size.XL"),
        "a shape with no text has no text size to pick"
    );

    editor.click("infinite-canvas.stroke.Red");
    editor.click("infinite-canvas.background.Blue");
    editor.click("infinite-canvas.fill-style.Hatched");
    editor.click("infinite-canvas.width.Extra-bold");
    editor.click("infinite-canvas.stroke-style.Dashed");
    editor.click("infinite-canvas.roughness.High");
    editor.snapshot("the_style_popover_restyles_the_selection");

    let [restyled] = entities(&editor).try_into().unwrap();
    let style = restyled.style;
    assert_eq!(
        style.stroke_color,
        CanvasColor::Palette {
            hue: CanvasHue::Red,
            tone: CanvasTone::Strong
        }
    );
    assert_eq!(
        style.fill_color,
        CanvasColor::Palette {
            hue: CanvasHue::Blue,
            tone: CanvasTone::Soft
        }
    );
    assert_eq!(style.fill, CanvasFill::Hatched);
    assert_eq!(style.stroke_width, 4.0);
    assert_eq!(style.stroke_style, CanvasStrokeStyle::Dashed);
    assert_eq!(style.look, CanvasLook::HandDrawn);
    assert_eq!(style.roughness, 2.0);
    assert_eq!(style.text_size, CanvasTextSize::Medium);
    assert_eq!(restyled.transform, rectangle.transform);

    editor.click("infinite-canvas.background.none");
    let [cleared] = entities(&editor).try_into().unwrap();
    assert_eq!(cleared.style.fill, CanvasFill::None);
}
