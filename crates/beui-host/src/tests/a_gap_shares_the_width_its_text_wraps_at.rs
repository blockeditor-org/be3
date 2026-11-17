use super::*;
use beui::text::{FontId, TextLayout};
use beui::text::{GapWidth, Piece, RichOptions};
use beui_core::font::Galley;

#[test]
fn a_gap_shares_the_width_its_text_wraps_at() {
    let half = GapWidth { px: -10.0, share: 0.5 };
    assert_eq!(half.of(200.0), 90.0, "pixels plus a share of the width");
    assert_eq!(half.of(f32::INFINITY), 0.0, "no room for a share, never narrower than none");
    assert_eq!(GapWidth::px(12.0).of(200.0), 12.0);

    let context = Context::new(crate::native::FreetypeFonts::default());
    let mut shaper =
        |text: &str, font: FontId| -> Galley { context.layout(text, font, TextLayout::DEFAULT) };
    let style = SpanStyle::new(FontId::proportional(14.0), Color32::WHITE);
    let at = |wrap_width: f32| RichOptions {
        wrap_width,
        padding: (0.0, 0.0),
        style,
        hang: 0,
        anywhere: false,
    };
    let pieces = [
        Piece::Gap(GapWidth { px: 0.0, share: 0.5 }),
        Piece::text("ab", style),
    ];
    let plain = RichLayout::new(&[Piece::text("ab", style)], at(200.0), &mut shaper);
    let a = plain.caret_rect(1, 1.0).min.x;
    let laid = RichLayout::new(&pieces, at(200.0), &mut shaper);
    assert_eq!(laid.caret_rect(1, 1.0).min.x, a + 100.0, "the text starts halfway");
    assert!(laid.fits(200.0), "the layout holds at its own width");
    assert!(!laid.fits(201.0), "but at no wider one");
    assert!(!laid.fits(199.0), "nor a narrower one");

    let unwrapped = RichLayout::new(&pieces, at(f32::INFINITY), &mut shaper);
    assert_eq!(
        unwrapped.caret_rect(1, 1.0).min.x,
        a,
        "a text that wraps nowhere gives a share no room"
    );
    assert!(!unwrapped.fits(200.0));
}
