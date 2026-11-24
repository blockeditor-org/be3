use beui_fonts::{HACK, INTER, LUCIDE, NOTO_EMOJI};
use beui_types::text::{FontFamily, FontId};
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::{GlyphMetrics, Metrics};
use skrifa::{FontRef, GlyphId, MetadataProvider};

thread_local! {
    static FACES: Vec<FontRef<'static>> = [INTER, HACK, NOTO_EMOJI, LUCIDE]
        .iter()
        .filter_map(|face| FontRef::new(beui_fonts::bytes(face.file)?).ok())
        .collect();
}

fn chain(family: FontFamily) -> &'static [usize] {
    match family {
        FontFamily::Proportional => &[0, 2],
        FontFamily::Monospace => &[1, 2],
        FontFamily::Icons => &[3],
    }
}

fn glyph<'a>(
    faces: &'a [FontRef<'static>],
    family: FontFamily,
    ch: char,
) -> Option<(&'a FontRef<'static>, GlyphId)> {
    chain(family).iter().find_map(|&index| {
        let face = faces.get(index)?;
        face.charmap().map(ch).map(|glyph| (face, glyph))
    })
}

fn units_per_em(face: &FontRef) -> f32 {
    face.metrics(Size::unscaled(), LocationRef::default())
        .units_per_em
        .max(1) as f32
}

pub(super) fn width(text: &str, font: FontId) -> Option<f32> {
    FACES.with(|faces| {
        let primary = faces.get(chain(font.family)[0])?;
        Some(
            text.chars()
                .map(|ch| {
                    let (face, glyph) =
                        glyph(faces, font.family, ch).unwrap_or((primary, GlyphId::NOTDEF));
                    let advance = GlyphMetrics::new(face, Size::unscaled(), LocationRef::default())
                        .advance_width(glyph)
                        .unwrap_or(0.0);
                    advance * font.size / units_per_em(face)
                })
                .sum(),
        )
    })
}

pub(super) fn extents(font: FontId) -> Option<(f32, f32)> {
    FACES.with(|faces| {
        let face = faces.get(chain(font.family)[0])?;
        let metrics = Metrics::new(face, Size::unscaled(), LocationRef::default());
        let scale = font.size / metrics.units_per_em.max(1) as f32;
        Some((metrics.ascent * scale, -metrics.descent * scale))
    })
}

pub(super) fn covers(text: &str, font: FontId) -> bool {
    FACES.with(|faces| {
        text.chars()
            .filter(|ch| !ch.is_whitespace() && !ch.is_control())
            .all(|ch| glyph(faces, font.family, ch).is_some())
    })
}
