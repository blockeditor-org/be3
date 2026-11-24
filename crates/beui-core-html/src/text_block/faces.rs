use beui_fonts::{HACK, INTER, LUCIDE, NOTO_EMOJI};
use beui_types::text::{FontFamily, FontId};
use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::{GlyphMetrics, Metrics};
use skrifa::{FontRef, GlyphId, MetadataProvider};

/// A bundled face, read once per thread: its cmap, its advances in font units, and its
/// ascent and descent in ems.
struct Face {
    charmap: Charmap<'static>,
    advances: GlyphMetrics<'static>,
    units_per_em: f32,
    ascent: f32,
    descent: f32,
}

impl Face {
    fn new(font: FontRef<'static>) -> Self {
        let metrics = Metrics::new(&font, Size::unscaled(), LocationRef::default());
        let units_per_em = metrics.units_per_em.max(1) as f32;
        Self {
            charmap: font.charmap(),
            advances: GlyphMetrics::new(&font, Size::unscaled(), LocationRef::default()),
            units_per_em,
            ascent: metrics.ascent / units_per_em,
            descent: -metrics.descent / units_per_em,
        }
    }
}

thread_local! {
    static FACES: Vec<Face> = [INTER, HACK, NOTO_EMOJI, LUCIDE]
        .iter()
        .filter_map(|face| FontRef::new(beui_fonts::bytes(face.file)?).ok())
        .map(Face::new)
        .collect();
}

fn chain(family: FontFamily) -> &'static [usize] {
    match family {
        FontFamily::Proportional => &[0, 2],
        FontFamily::Monospace => &[1, 2],
        FontFamily::Icons => &[3],
    }
}

fn glyph(faces: &[Face], family: FontFamily, ch: char) -> Option<(&Face, GlyphId)> {
    chain(family).iter().find_map(|&index| {
        let face = faces.get(index)?;
        face.charmap.map(ch).map(|glyph| (face, glyph))
    })
}

pub(super) fn width(text: &str, font: FontId) -> Option<f32> {
    FACES.with(|faces| {
        let primary = faces.get(chain(font.family)[0])?;
        Some(
            text.chars()
                .map(|ch| {
                    let (face, glyph) =
                        glyph(faces, font.family, ch).unwrap_or((primary, GlyphId::NOTDEF));
                    let advance = face.advances.advance_width(glyph).unwrap_or(0.0);
                    advance * font.size / face.units_per_em
                })
                .sum(),
        )
    })
}

pub(super) fn extents(font: FontId) -> Option<(f32, f32)> {
    FACES.with(|faces| {
        let face = faces.get(chain(font.family)[0])?;
        Some((face.ascent * font.size, face.descent * font.size))
    })
}

pub(super) fn covers(text: &str, font: FontId) -> bool {
    FACES.with(|faces| {
        text.chars()
            .filter(|ch| !ch.is_whitespace() && !ch.is_control())
            .all(|ch| glyph(faces, font.family, ch).is_some())
    })
}
