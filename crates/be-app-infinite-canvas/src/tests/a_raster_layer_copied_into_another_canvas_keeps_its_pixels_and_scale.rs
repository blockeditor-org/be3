use super::*;
use be_sdk_beui::beui::clipboard::{CanvasItems, formats};
use be_sdk_beui::beui::{ClipItem, Input, Key, Modifiers};

#[test]
fn a_raster_layer_copied_into_another_canvas_keeps_its_pixels_and_scale() {
    let (mut source, layer) = painted_scaled_layer();
    source.key(Key::C, Modifiers::CTRL);
    source.run();
    let [copied] = &source.take_copied(formats::CANVAS_ITEMS)[..] else {
        panic!("Copy puts the layer on the clipboard as canvas items");
    };
    let copied = ClipItem::new(formats::CANVAS_ITEMS, copied.clone());
    let items = copied
        .decode::<CanvasItems>()
        .expect("the canvas items decode");

    let mut editor = raster_editor(&[]);
    let center = editor.rect_of("infinite-canvas.canvas").center();
    editor.click(center);
    editor.run();
    editor.step(vec![Input::Paste(copied)]);
    editor.run();
    assert!(
        rasters(&editor).is_empty(),
        "the paste waits for the canvas it came from, for the layer's pixels"
    );
    editor.hold(Some(items.canvas), source.content::<CanvasContent>(None));
    editor.run();
    let [pasted] = &rasters(&editor)[..] else {
        panic!("the paste adds one layer");
    };
    assert_ne!(pasted.id, layer);
    assert_eq!(
        pasted.transform.size,
        CanvasPoint::new(16.0, 16.0),
        "the paste keeps the layer's scale"
    );
    assert_eq!(
        raster_of(&editor, pasted.id),
        raster_of(&source, layer),
        "and its pixels"
    );
}
