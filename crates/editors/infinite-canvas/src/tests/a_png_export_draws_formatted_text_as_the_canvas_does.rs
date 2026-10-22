use super::copying_as_svg_writes_every_shape_kind::palette_runs;
use super::*;
use block_editor_beui::be_block::be_model::RichText;
use text_editor_core::schema::basic::{BOLD, UNDERLINE, mark};

fn exported(text: RichText) -> image::RgbaImage {
    let mut note = shaped(Uuid::from_u128(1), CanvasShape::Text);
    note.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(160.0, 40.0), 0.0);
    note.text = text;
    let mut editor = editor(&[note]);
    palette_runs(&mut editor, "copy as png");
    let copied = editor.take_copied_images();
    let [png] = copied.as_slice() else {
        panic!("Copy as PNG puts one image on the clipboard");
    };
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("the export is a readable PNG")
        .into_rgba8()
}

fn longest_line(image: &image::RgbaImage, background: [u8; 4]) -> u32 {
    let (width, height) = image.dimensions();
    (0..height)
        .map(|y| {
            let mut longest = 0;
            let mut run = 0;
            for x in 0..width {
                let pixel = image.get_pixel(x, y).0;
                let inked = pixel
                    .iter()
                    .zip(background)
                    .any(|(channel, under)| channel.abs_diff(under) > 128);
                match inked {
                    false => run = 0,
                    true => {
                        run += 1;
                        longest = longest.max(run);
                    }
                }
            }
            longest
        })
        .max()
        .unwrap_or(0)
}

#[test]
fn a_png_export_draws_formatted_text_as_the_canvas_does() {
    let plain = exported(RichText::new("Hi there", Vec::new()));
    let formatted = exported(RichText::new(
        "Hi there",
        vec![mark(0..2, BOLD), mark(3..8, UNDERLINE)],
    ));
    assert_eq!(plain.dimensions(), formatted.dimensions());
    let background = plain.get_pixel(1, 1).0;
    let (plain_line, formatted_line) = (
        longest_line(&plain, background),
        longest_line(&formatted, background),
    );
    assert!(
        plain_line < 40,
        "plain text has no long horizontal stroke, but has one of {plain_line} pixels in {:?}",
        plain.dimensions()
    );
    assert!(
        formatted_line >= 70,
        "the underlined word is drawn with a line under it, as on the canvas, longest {formatted_line}"
    );
}
