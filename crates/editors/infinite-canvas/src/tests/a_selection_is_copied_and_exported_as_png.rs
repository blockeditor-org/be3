use super::copying_as_svg_writes_every_shape_kind::palette_runs;
use super::*;
use block_editor_beui::SavedFile;
use block_ui_test::HostCall;

fn pixels(png: &[u8]) -> image::RgbaImage {
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("the export is a readable PNG")
        .into_rgba8()
}

#[test]
fn a_selection_is_copied_and_exported_as_png() {
    let card = card();
    let mut editor = editor(std::slice::from_ref(&card));
    editor.click(format!("infinite-canvas.entity.{}", card.id));
    editor.run();

    palette_runs(&mut editor, "copy as png");
    let copied = editor.take_copied_images();
    let [png] = copied.as_slice() else {
        panic!(
            "Copy as PNG puts one image on the clipboard, put {}",
            copied.len()
        );
    };
    let copied = pixels(png);
    let side = |world: f32| ((world + 2.0 * (2.0 + 2.0 + 16.0)) * 2.0).ceil() as u32;
    assert_eq!(
        copied.dimensions(),
        (side(180.0), side(120.0)),
        "the selection and its padding are drawn at twice their size"
    );
    let (width, height) = copied.dimensions();
    assert_eq!(
        copied.get_pixel(width / 2, height / 2).0,
        [60, 110, 90, 255],
        "the card is filled with its own colour"
    );
    assert_eq!(
        copied.get_pixel(1, 1).0[3],
        255,
        "the theme's background is behind it by default"
    );

    palette_runs(&mut editor, "export as png");
    editor.click("infinite-canvas.export.background");
    editor.run();
    editor.snapshot("a_selection_is_copied_and_exported_as_png");
    editor.click("infinite-canvas.export.confirm");
    editor.run();
    let saved: Vec<SavedFile> = editor
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::SaveFile { file, .. } => Some(file),
            _ => None,
        })
        .collect();
    let [file] = saved.as_slice() else {
        panic!(
            "saving asks the host to save one file, asked {}",
            saved.len()
        );
    };
    assert_eq!(file.name, "Canvas.png");
    assert_eq!(file.mime_type, "image/png");
    let saved = pixels(&file.data);
    assert_eq!(saved.dimensions(), (width, height));
    assert_eq!(
        saved.get_pixel(1, 1).0,
        [0, 0, 0, 0],
        "with the background off, around the card is transparent"
    );
    assert_eq!(saved.get_pixel(width / 2, height / 2).0, [60, 110, 90, 255]);
}
