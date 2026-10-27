use super::*;
use beui_core::font::{FontId, Galley, TextLayout};
use beui_core::rich::{Piece, RichOptions};

const COLUMNS: usize = 12;

#[test]
fn breaking_anywhere_wraps_monospace_text_every_column() {
    let context = Context::new(crate::native::FreetypeFonts::default());
    let mut shaper =
        |text: &str, font: FontId| -> Galley { context.layout(text, font, TextLayout::DEFAULT) };
    let text = "ls -la /usr/local/share/caf\u{e9} && echo done-and-dusted now";
    for size in [11.0, 12.5, 13.0, 14.0, 15.0, 17.0] {
        let style = SpanStyle::new(FontId::monospace(size), Color32::WHITE);
        let bold = SpanStyle::new(FontId::monospace(size).bold(true), Color32::WHITE);
        let cell = shaper("M", style.font).cursor_pos(pos2(0.0, 0.0), 1).x;
        let options = |anywhere: bool| RichOptions {
            wrap_width: cell * COLUMNS as f32,
            padding: (0.0, 0.0),
            style,
            hang: 0,
            anywhere,
            least: false,
        };
        let pieces = [
            Piece::text(&text[..20], style),
            Piece::text(&text[20..], bold),
        ];

        let anywhere = RichLayout::new(&pieces, options(true), &mut shaper);
        let lines: Vec<&str> = anywhere
            .lines
            .iter()
            .map(|line| &text[line.range.clone()])
            .collect();
        let (last, full) = lines.split_last().expect("the text has lines");
        for line in full {
            assert_eq!(
                line.chars().count(),
                COLUMNS,
                "at {size}, every line but the last holds {COLUMNS} characters: {lines:?}"
            );
        }
        assert!(last.chars().count() <= COLUMNS);
        assert_eq!(
            lines.concat(),
            text,
            "the lines hold every byte once, in order"
        );

        let words = RichLayout::new(&pieces, options(false), &mut shaper);
        assert_eq!(
            &text[words.lines[0].range.clone()],
            "ls -la /usr/",
            "without breaking anywhere the first line ends where a word does"
        );
    }
}
