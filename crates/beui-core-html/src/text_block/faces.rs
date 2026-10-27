use beui_fonts::{HACK, INTER, LUCIDE, NOTO_EMOJI};
use beui_types::text::{FontFamily, FontId};
use ttf_parser::{Face, GlyphId};

thread_local! {
    static FACES: Vec<Face<'static>> = [INTER, HACK, NOTO_EMOJI, LUCIDE]
        .iter()
        .filter_map(|face| Face::parse(beui_fonts::bytes(face.file)?, 0).ok())
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
    faces: &'a [Face<'static>],
    family: FontFamily,
    ch: char,
) -> Option<(&'a Face<'static>, GlyphId)> {
    chain(family).iter().find_map(|&index| {
        let face = faces.get(index)?;
        face.glyph_index(ch).map(|glyph| (face, glyph))
    })
}

pub(super) fn width(text: &str, font: FontId) -> Option<f32> {
    FACES.with(|faces| {
        let primary = faces.get(chain(font.family)[0])?;
        Some(
            text.chars()
                .map(|ch| {
                    let (face, glyph) =
                        glyph(faces, font.family, ch).unwrap_or((primary, GlyphId(0)));
                    let advance = face.glyph_hor_advance(glyph).unwrap_or(0) as f32;
                    advance * font.size / face.units_per_em() as f32
                })
                .sum(),
        )
    })
}

pub(super) fn extents(font: FontId) -> Option<(f32, f32)> {
    FACES.with(|faces| {
        let face = faces.get(chain(font.family)[0])?;
        let scale = font.size / face.units_per_em() as f32;
        Some((
            face.ascender() as f32 * scale,
            -(face.descender() as f32) * scale,
        ))
    })
}

pub(super) fn covers(text: &str, font: FontId) -> bool {
    FACES.with(|faces| {
        text.chars()
            .filter(|ch| !ch.is_whitespace() && !ch.is_control())
            .all(|ch| glyph(faces, font.family, ch).is_some())
    })
}
