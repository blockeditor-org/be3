use super::*;
use be_sdk_beui::beui::{Key, Modifiers};

#[test]
fn a_raster_layer_is_exported_pixel_sharp() {
    let layer = raster_layer(1, CanvasPoint::default(), 2);
    let mut editor = raster_editor(std::slice::from_ref(&layer));
    paint(&mut editor, layer.id, &[(0, 0), (1, 1)], RED);
    let mut scaled = layer.clone();
    scaled.transform.size = CanvasPoint::new(16.0, 16.0);
    apply(
        &mut editor,
        InfiniteCanvasOperation::Update {
            entities: vec![scaled],
        },
    );
    editor.run();
    editor.key(
        Key::A,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );

    let copied = vec![String::from_utf8(copied_export(&mut editor, "svg", "image/svg+xml"))
        .expect("the SVG is text")];
    let [svg] = copied.as_slice() else {
        panic!("Copy as SVG puts one SVG on the clipboard, put {copied:?}");
    };
    assert!(
        svg.contains(
            "<image x=\"-8\" y=\"-8\" width=\"16\" height=\"16\" preserveAspectRatio=\"none\" image-rendering=\"pixelated\""
        ),
        "the layer, scaled eightfold, is an image drawn with square pixels:\n{svg}"
    );
    assert!(svg.contains("href=\"data:image/png;base64,"));

    let png = &copied_export(&mut editor, "png", be_sdk_beui::beui::clipboard::formats::PNG);
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("the export is a readable PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let (middle_x, middle_y) = (width / 2, height / 2);
    assert_eq!(
        image.get_pixel(middle_x - 1, middle_y - 1).0,
        [220, 40, 40, 255],
        "a pixel keeps its colour right up to its edge"
    );
    assert_ne!(
        image.get_pixel(middle_x + 1, middle_y - 1).0,
        [220, 40, 40, 255]
    );
}
