use super::*;
use beui_core::font::{FontId, Galley, TextLayout};
use beui_core::rich::Piece;
use beui_core::rich::{OBJECT, RichOptions};

const BREAK: &str = "|";

const WORDS: [&str; 14] = [
    "a ",
    "breaking ",
    "line-",
    "wraps ",
    "x",
    "unbrokenwordthatislong ",
    "and/or ",
    "of ",
    "ten ",
    "\n",
    "words ",
    "z ",
    BREAK,
    BREAK,
];

fn options(wrap_width: f32, anywhere: bool) -> RichOptions {
    RichOptions {
        wrap_width,
        padding: (0.0, 0.0),
        style: SpanStyle::new(FontId::proportional(14.0), Color32::WHITE),
        hang: 0,
        anywhere,
        least: false,
    }
}

fn breaks(layout: &RichLayout) -> Vec<std::ops::Range<usize>> {
    layout.lines.iter().map(|line| line.range.clone()).collect()
}

#[test]
fn a_reused_or_resumed_rich_layout_breaks_lines_as_a_fresh_one_would() {
    let context = Context::new(crate::native::FreetypeFonts::default());
    let mut shaper =
        |text: &str, font: FontId| -> Galley { context.layout(text, font, TextLayout::DEFAULT) };
    let mut seed: u64 = 7;
    let mut next = move |below: usize| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as usize % below
    };
    let style = SpanStyle::new(FontId::proportional(14.0), Color32::WHITE);
    let bold = SpanStyle::new(FontId::proportional(14.0).bold(true), Color32::WHITE);
    let mut texts: Vec<String> = (0..40)
        .map(|_| WORDS[next(WORDS.len())].to_owned())
        .collect();
    for round in 0..200 {
        let pieces: Vec<Piece> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| match index % 7 {
                3 => Piece::Inline {
                    index: index / 7,
                    size: Vec2::new(18.0, 10.0),
                    text: OBJECT,
                },
                _ if text == BREAK => Piece::Break,
                5 => Piece::text(text, bold),
                _ => Piece::text(text, style),
            })
            .collect();
        let wrap = 40.0 + next(400) as f32;
        let anywhere = round % 2 == 1;
        let laid = RichLayout::new(&pieces, options(wrap, anywhere), &mut shaper);
        for _ in 0..20 {
            let other = 20.0 + next(500) as f32;
            if laid.fits(other) {
                let fresh = RichLayout::new(&pieces, options(other, anywhere), &mut shaper);
                assert_eq!(
                    breaks(&laid),
                    breaks(&fresh),
                    "round {round}: a layout at {wrap} said it fits {other}"
                );
            }
        }
        let changed = next(texts.len());
        texts[changed] = WORDS[next(WORDS.len())].to_owned();
        let edited: Vec<Piece> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| match index % 7 {
                3 => Piece::Inline {
                    index: index / 7,
                    size: Vec2::new(18.0, 10.0),
                    text: OBJECT,
                },
                _ if text == BREAK => Piece::Break,
                5 => Piece::text(text, bold),
                _ => Piece::text(text, style),
            })
            .collect();
        let at: usize = edited[..changed].iter().map(Piece::len).sum();
        let resumed = laid.resume(at, &edited, options(wrap, anywhere), &mut shaper);
        let fresh = RichLayout::new(&edited, options(wrap, anywhere), &mut shaper);
        assert_eq!(
            breaks(&resumed),
            breaks(&fresh),
            "round {round}: resuming after piece {changed} at {wrap}"
        );
    }
}
