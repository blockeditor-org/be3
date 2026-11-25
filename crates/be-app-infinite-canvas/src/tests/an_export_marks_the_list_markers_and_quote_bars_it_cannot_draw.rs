use super::*;
use be_sdk_beui::be_block_content::be_block_model::RichText;
use beui_text_editor_core::Value;
use beui_text_editor_core::schema::basic::{self, BULLET, LIST, QUOTE};

const PINK: [u8; 3] = [255, 105, 180];

fn note(text: RichText) -> BeuiTest<CanvasRoot> {
    let mut note = shaped(Uuid::from_u128(1), CanvasShape::Text);
    note.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(200.0, 80.0), 0.0);
    note.text = text;
    editor(&[note])
}

fn pink_pixels(editor: &mut BeuiTest<CanvasRoot>) -> usize {
    let png = &copied_export(editor, "png", be_sdk_beui::beui::clipboard::formats::PNG);
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("the export is a readable PNG")
        .into_rgba8()
        .pixels()
        .filter(|pixel| pixel.0[..3] == PINK)
        .count()
}

fn pink_rects(editor: &mut BeuiTest<CanvasRoot>) -> usize {
    let copied = [String::from_utf8(copied_export(editor, "svg", "image/svg+xml")).expect("the SVG is text")];
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
