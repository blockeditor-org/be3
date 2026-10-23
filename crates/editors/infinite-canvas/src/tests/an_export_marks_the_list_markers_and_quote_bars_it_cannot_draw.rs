use super::copying_as_svg_writes_every_shape_kind::palette_runs;
use super::*;
use block_editor_beui::be_block::be_model::RichText;
use text_editor_core::Value;
use text_editor_core::schema::basic::{self, BULLET, LIST, QUOTE};

const PINK: [u8; 3] = [255, 105, 180];

fn note(text: RichText) -> BeuiTest<CanvasApp> {
    let mut note = shaped(Uuid::from_u128(1), CanvasShape::Text);
    note.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(200.0, 80.0), 0.0);
    note.text = text;
    editor(&[note])
}

fn pink_pixels(editor: &mut BeuiTest<CanvasApp>) -> usize {
    palette_runs(editor, "copy as png");
    let copied = editor.take_copied_images();
    let [png] = copied.as_slice() else {
        panic!("Copy as PNG puts one image on the clipboard");
    };
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("the export is a readable PNG")
        .into_rgba8()
        .pixels()
        .filter(|pixel| pixel.0[..3] == PINK)
        .count()
}

fn pink_rects(editor: &mut BeuiTest<CanvasApp>) -> usize {
    palette_runs(editor, "copy as svg");
    let copied = editor.take_copied_text();
    copied
        .iter()
        .flat_map(|svg| svg.lines())
        .filter(|line| line.starts_with("<rect") && line.contains("#ff69b4"))
        .count()
}

#[test]
fn an_export_marks_the_list_markers_and_quote_bars_it_cannot_draw() {
    let text = "Milk\nA quote\n";
    let marked = || {
        RichText::new(
            text,
            vec![
                basic::block(0..4, LIST, Value::text(BULLET)),
                basic::block(5..12, QUOTE, Value::Bool(true)),
            ],
        )
    };
    let mut plain = note(RichText::new(text, Vec::new()));
    assert_eq!(pink_pixels(&mut plain), 0, "plain text exports no pink box");
    let mut list = note(marked());
    assert!(
        pink_pixels(&mut list) > 100,
        "the bullet and the quote bar are exported as pink boxes"
    );
    let mut svg = note(marked());
    assert!(
        pink_rects(&mut svg) >= 2,
        "the SVG export draws a pink box for the bullet and one for the quote"
    );
}
