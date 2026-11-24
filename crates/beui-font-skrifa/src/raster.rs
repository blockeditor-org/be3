use std::cell::RefCell;

use be_lib_stable_hash::HashMap;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, Engine, GlyphStyles, HintingInstance, HintingOptions};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId as FontGlyph, MetadataProvider, OutlineGlyphCollection};

use beui_core::font::{GlyphId, GlyphImage, GlyphRasterizer};

use crate::outline::{Outline, Placed};
use crate::{FontData, SUBPIXEL_POSITIONS, color};

/// The shear `FT_GlyphSlot_Oblique` gives a synthetic italic: 0x0366A in 16.16.
const OBLIQUE: f32 = 0x0366A as f32 / 65536.0;

/// Whether a face has outlines, rather than bitmaps alone.
pub(crate) fn scalable(font: &FontRef) -> bool {
    font.glyf().is_ok() || font.cff().is_ok() || font.cff2().is_ok()
}

pub(crate) struct RasterFace {
    data: FontData,
    scalable: bool,
    engine: RefCell<Option<Engine>>,
    hinting: RefCell<HashMap<u32, Option<HintingInstance>>>,
}

impl RasterFace {
    pub(crate) fn new(data: FontData, font: &FontRef) -> Self {
        Self {
            data,
            scalable: scalable(font),
            engine: RefCell::new(None),
            hinting: RefCell::default(),
        }
    }

    fn font(&self) -> Option<FontRef<'_>> {
        FontRef::from_index(&self.data.bytes, self.data.index).ok()
    }

    /// The engine FreeType's defaults pick: the font's own instructions, or the autohinter
    /// for a font without them. The autohinter's styles are worked out once per face.
    fn engine(&self, outlines: &OutlineGlyphCollection) -> Engine {
        self.engine
            .borrow_mut()
            .get_or_insert_with(|| match outlines.prefer_interpreter() {
                true => Engine::Interpreter,
                false => Engine::Auto(Some(GlyphStyles::new(outlines))),
            })
            .clone()
    }

    fn draw(&self, font: &FontRef, glyph: FontGlyph, pixel_size: u32, hinted: bool) -> Option<Outline> {
        let outlines = font.outline_glyphs();
        let outline = outlines.get(glyph)?;
        let mut path = Outline::default();
        let size = Size::new(pixel_size as f32);
        if hinted {
            let mut cache = self.hinting.borrow_mut();
            let instance = cache.entry(pixel_size).or_insert_with(|| {
                let options = HintingOptions {
                    engine: self.engine(&outlines),
                    target: Default::default(),
                };
                HintingInstance::new(&outlines, size, LocationRef::default(), options).ok()
            });
            if let Some(instance) = instance {
                outline
                    .draw(DrawSettings::hinted(instance, false), &mut path)
                    .ok()?;
                return Some(path);
            }
        }
        outline
            .draw(DrawSettings::unhinted(size, LocationRef::default()), &mut path)
            .ok()?;
        Some(path)
    }
}

#[derive(Default)]
pub(crate) struct Rasterizer {
    pub(crate) faces: RefCell<Vec<RasterFace>>,
}

impl Rasterizer {
    /// The ascender and descender of a face at a pixel size, read from hhea (or OS/2 when
    /// hhea has none) and fitted to the grid as FreeType fits them: out to whole pixels, or
    /// to the nearest for a TrueType face that asks for integer scaling. None for a face of
    /// bitmaps alone.
    pub(crate) fn metrics(&self, index: usize, pixel_size: u32) -> Option<(f32, f32)> {
        let faces = self.faces.borrow();
        let face = faces.get(index)?;
        if !face.scalable {
            return None;
        }
        let font = face.font()?;
        let head = font.head().ok()?;
        let units = i64::from(head.units_per_em().max(1));
        let hhea = font.hhea().ok()?;
        let (mut ascender, mut descender) = (
            i64::from(hhea.ascender().to_i16()),
            i64::from(hhea.descender().to_i16()),
        );
        if ascender == 0
            && descender == 0
            && let Ok(os2) = font.os2()
        {
            (ascender, descender) = match (os2.s_typo_ascender(), os2.s_typo_descender()) {
                (0, 0) => (
                    i64::from(os2.us_win_ascent()),
                    -i64::from(os2.us_win_descent()),
                ),
                (ascender, descender) => (i64::from(ascender), i64::from(descender)),
            };
        }
        let scale = ((i64::from(pixel_size) * 64) << 16).saturating_add(units / 2) / units;
        let scaled = |value: i64| {
            let product = (value.abs() * scale + 0x8000) >> 16;
            value.signum() * product
        };
        let truetype = font.glyf().is_ok();
        let (ascender, descender) = match truetype && head.flags().bits() & 8 != 0 {
            true => (
                (scaled(ascender) + 32) & -64,
                (scaled(descender) + 32) & -64,
            ),
            false => ((scaled(ascender) + 63) & -64, scaled(descender) & -64),
        };
        Some((ascender as f32 / 64.0, descender as f32 / 64.0))
    }
}

impl GlyphRasterizer for Rasterizer {
    fn rasterize(&self, key: GlyphId) -> Option<GlyphImage> {
        let faces = self.faces.borrow();
        let face = faces.get(key.face)?;
        let font = face.font()?;
        let glyph = FontGlyph::new(key.glyph);
        let raster = key.raster();
        if !face.scalable {
            return color::bitmap(&font, glyph, key.pixel_size, raster);
        }
        let hinted = key.raster == GlyphId::LAID_OUT;
        let placed = Placed {
            strength: match key.bold {
                true => (key.pixel_size * 64 / 24) as f32 / 64.0,
                false => 0.0,
            },
            shear: if key.italic { OBLIQUE } else { 0.0 },
            scale: raster,
            shift: key.subpixel as f32 / SUBPIXEL_POSITIONS as f32,
        };
        let draw = |glyph| face.draw(&font, glyph, key.pixel_size, hinted);
        if let Some(layers) = color::layers(&font, glyph) {
            let layers = layers
                .into_iter()
                .filter_map(|(glyph, paint)| Some((draw(glyph)?.place(placed), paint)))
                .collect::<Vec<_>>();
            return Some(color::paint_layers(&layers));
        }
        let (pixels, placement) = draw(glyph)?.place(placed).fill();
        Some(GlyphImage {
            width: placement.width,
            height: placement.height,
            left: placement.left,
            top: -placement.top,
            pixels,
            color: false,
        })
    }
}
