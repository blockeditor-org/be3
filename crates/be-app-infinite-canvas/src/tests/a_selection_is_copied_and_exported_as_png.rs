use super::*;
use be_sdk_beui::SavedFile;
use be_sdk_test::HostCall;

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

    let png = &copied_export(
        &mut editor,
        "png",
        be_sdk_beui::beui::clipboard::formats::PNG,
    );
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

    let mut dialog = export_dialog(&mut editor, "png");
    dialog.click("infinite-canvas.export.background");
    dialog.snapshot("a_selection_is_copied_and_exported_as_png");
    dialog.click("export.save");
    let saved: Vec<SavedFile> = dialog
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
