mod color;
mod library;
mod outline;
mod raster;
#[cfg(not(target_arch = "wasm32"))]
pub mod system;

use std::cell::RefCell;
use std::rc::Rc;

use be_lib_stable_hash::HashMap;
use harfrust::{
    Direction, Language, Script, script, ShapeOptions, ShapePlan, ShaperData, Tag, UnicodeBuffer,
};
use icu_properties::props::Script as Unicode;
use icu_properties::{CodePointMapData, PropertyNamesShort};
use skrifa::charmap::MappingIndex;
use skrifa::instance::{LocationRef, Size};
use skrifa::{FontRef, GlyphId as FontGlyph, MetadataProvider};

use beui_core::font::{
    FontBackend, Galley, GalleyLine, Glyph, GlyphId, GlyphRaster, GlyphRasterizer,
    Shaping,
};
use raster::{RasterFace, Rasterizer};
use beui_types::vec2;
use beui_types::{FontFamily, FontId};

pub use library::{FontBytes, FontData, FontLibrary, FontSources, ICONS_FONT};

pub struct SkrifaFonts {
    fonts: FontLibrary,
    generation: u64,
    fallbacks: usize,
    faces: Vec<FaceData>,
    proportional: Vec<usize>,
    monospace: Vec<usize>,
    icons: Vec<usize>,
    glyphs: HashMap<GlyphId, Rc<GlyphRaster>>,
    rasterizer: Rc<Rasterizer>,
    shaped: ShapedLines,
    covering: HashMap<(FontFamily, char), Option<usize>>,
}

impl SkrifaFonts {
    pub fn new(fonts: FontLibrary) -> Self {
        let mut backend = Self {
            generation: fonts.generation(),
            fonts,
            fallbacks: 0,
            faces: Vec::new(),
            proportional: Vec::new(),
            monospace: Vec::new(),
            icons: Vec::new(),
            glyphs: HashMap::default(),
            rasterizer: Rc::default(),
            shaped: ShapedLines::default(),
            covering: HashMap::default(),
        };
        backend.load_families();
        backend
    }

    fn load_families(&mut self) {
        let sources = self.fonts.sources();
        let proportional = self.load_chain(&sources.proportional);
        let monospace = self.load_chain(&sources.monospace);
        let fallback = self.load_chain(&sources.fallback);
        self.fallbacks = sources.fallback.len();
        self.proportional = chain(&proportional, &[&monospace, &fallback]);
        self.monospace = chain(&monospace, &[&proportional, &fallback]);
        self.icons = self.load_chain(&sources.icons);
        self.covering.clear();
    }

    fn refresh(&mut self) {
        let generation = self.fonts.generation();
        if generation == self.generation {
            return;
        }
        self.generation = generation;
        self.close_faces();
        self.shaped = ShapedLines::default();
        self.load_families();
    }

    fn load_new_fallbacks(&mut self) -> bool {
        if self.fonts.fallback_count() <= self.fallbacks {
            return false;
        }
        let added = self.fonts.fallback_from(self.fallbacks);
        self.fallbacks += added.len();
        let loaded = self.load_chain(&added);
        if !loaded.is_empty() {
            self.shaped = ShapedLines::default();
        }
        for index in &loaded {
            for family in [&mut self.proportional, &mut self.monospace] {
                if !family.contains(index) {
                    family.push(*index);
                }
            }
        }
        if !loaded.is_empty() {
            self.covering.clear();
        }
        !loaded.is_empty()
    }

    fn load_chain(&mut self, sources: &[FontData]) -> Vec<usize> {
        sources
            .iter()
            .filter_map(|source| self.load_face(source))
            .collect()
    }

    fn load_face(&mut self, source: &FontData) -> Option<usize> {
        if let Some(index) = self.faces.iter().position(|face| face.data.same(source)) {
            return Some(index);
        }
        let font = FontRef::from_index(&source.bytes, source.index).ok()?;
        self.rasterizer
            .faces
            .borrow_mut()
            .push(RasterFace::new(source.clone(), &font));
        self.faces.push(FaceData {
            data: source.clone(),
            scalable: raster::scalable(&font),
            charmap: MappingIndex::new(&font),
            shaper: ShaperData::new(&font),
            plans: RefCell::default(),
        });
        Some(self.faces.len() - 1)
    }

    fn close_faces(&mut self) {
        self.faces.clear();
        self.glyphs.clear();
        self.rasterizer = Rc::default();
        self.proportional.clear();
        self.monospace.clear();
        self.icons.clear();
        self.covering.clear();
        self.fallbacks = 0;
    }

    fn family(&self, family: FontFamily) -> &[usize] {
        match family {
            FontFamily::Proportional => &self.proportional,
            FontFamily::Monospace => &self.monospace,
            FontFamily::Icons => &self.icons,
        }
    }

    fn shape_galley(
        &mut self,
        text: &str,
        family: FontFamily,
        pixel_size: u32,
        shape: Shaping,
        scale: f32,
    ) -> Galley {
        self.refresh();
        let (ascent, descent) = self.metrics(family, pixel_size);
        let line_height = shape.line();
        let baseline = ((line_height - (ascent - descent)) / 2.0).round() + ascent;
        let shaped = self.shaped(text, family, pixel_size);
        let mut placed = Vec::with_capacity(shaped.len());
        let mut cursors = Vec::with_capacity(shaped.len() + 1);
        let mut pen = 0.0;
        for glyph in shaped.iter() {
            if cursors
                .last()
                .is_none_or(|(previous, _)| *previous != glyph.cluster)
            {
                cursors.push((glyph.cluster, pen / scale));
            }
            if let Some(glyph) = self.place(*glyph, shape, pixel_size, pen, baseline) {
                placed.push(glyph);
            }
            pen += glyph.x_advance;
        }
        cursors.push((text.len(), pen / scale));
        let font = FontId {
            size: pixel_size as f32 / scale,
            family,
            bold: shape.bold,
            italic: shape.italic,
        };
        Galley::new(
            text,
            font,
            vec2(pen.ceil() / scale, line_height / scale),
            line_height / scale,
            baseline / scale,
            placed,
            vec![GalleyLine {
                top: 0.0,
                range: 0..text.len(),
                cursors,
            }],
            scale,
        )
    }

    fn place(
        &mut self,
        glyph: ShapedGlyph,
        shape: Shaping,
        pixel_size: u32,
        pen: f32,
        baseline: f32,
    ) -> Option<Glyph> {
        let (whole, subpixel) = split_subpixel(pen + glyph.x_offset);
        let id = GlyphId {
            face: glyph.face,
            glyph: glyph.glyph,
            pixel_size,
            subpixel,
            bold: shape.bold,
            italic: shape.italic,
            raster: GlyphId::LAID_OUT,
        };
        let raster = match self.glyphs.get(&id) {
            Some(raster) => raster.clone(),
            None => {
                let bounds = ink_bounds(self.extents(glyph, pixel_size)?, pixel_size, shape);
                let rasterizer: Rc<dyn GlyphRasterizer> = self.rasterizer.clone();
                let raster = Rc::new(GlyphRaster::lazy(bounds, Rc::downgrade(&rasterizer)));
                self.glyphs.insert(id, raster.clone());
                raster
            }
        };
        Some(Glyph {
            id,
            origin: vec2(whole, (baseline - glyph.y_offset).round()),
            raster,
        })
    }

    fn rastered(&mut self, galley: &Galley, raster: f32) -> Galley {
        let positions = SUBPIXEL_POSITIONS as f32;
        let glyphs = galley
            .glyphs()
            .iter()
            .map(|glyph| {
                let pen = glyph.origin.x + glyph.id.subpixel as f32 / positions;
                let (whole, subpixel) = split_subpixel(pen * raster);
                let id = GlyphId {
                    subpixel,
                    raster: (glyph.id.raster() * raster).to_bits(),
                    ..glyph.id
                };
                let shown = match self.glyphs.get(&id) {
                    Some(shown) => shown.clone(),
                    None => {
                        let [left, top, right, bottom] = glyph.bounds();
                        let (x, y) = (glyph.origin.x, glyph.origin.y);
                        let bounds = [
                            (left - x) * raster - 1.0,
                            (top - y) * raster - 1.0,
                            (right - x) * raster + 1.0,
                            (bottom - y) * raster + 1.0,
                        ];
                        let rasterizer: Rc<dyn GlyphRasterizer> = self.rasterizer.clone();
                        let shown = Rc::new(GlyphRaster::lazy(bounds, Rc::downgrade(&rasterizer)));
                        self.glyphs.insert(id, shown.clone());
                        shown
                    }
                };
                Glyph {
                    id,
                    origin: vec2(whole, (glyph.origin.y * raster).round()),
                    raster: shown,
                }
            })
            .collect();
        galley.with_glyphs(glyphs, galley.pixels_per_point() * raster)
    }

    /// The ink box of a glyph, y down, or None for one that draws nothing. Read only for a
    /// glyph not yet in the cache, since a face whose outlines must be drawn to be measured
    /// (CFF, gvar) makes it costly.
    fn extents(&self, glyph: ShapedGlyph, pixel_size: u32) -> Option<[f32; 4]> {
        let face = &self.faces[glyph.face];
        let unknown = || {
            let size = pixel_size as f32;
            Some([-size, -2.0 * size, 2.0 * size, size])
        };
        let Ok(font) = FontRef::from_index(&face.data.bytes, face.data.index) else {
            return unknown();
        };
        let id = FontGlyph::new(glyph.glyph);
        if !face.scalable {
            return color::bitmap_extents(&font, id, pixel_size).or_else(unknown);
        }
        let metrics = font.glyph_metrics(Size::new(pixel_size as f32), LocationRef::default());
        let Some(bounds) = metrics.bounds(id) else {
            return unknown();
        };
        if bounds.x_max <= bounds.x_min || bounds.y_max <= bounds.y_min {
            return None;
        }
        let grid = |value: f32, round: fn(f32) -> f32| round(value * 64.0) / 64.0;
        Some([
            grid(bounds.x_min, f32::floor),
            -grid(bounds.y_max, f32::ceil),
            grid(bounds.x_max, f32::ceil),
            -grid(bounds.y_min, f32::floor),
        ])
    }

    fn metrics(&self, family: FontFamily, pixel_size: u32) -> (f32, f32) {
        let fallback = (pixel_size as f32 * 0.8, pixel_size as f32 * -0.2);
        let Some(&index) = self.family(family).first() else {
            return fallback;
        };
        self.rasterizer
            .metrics(index, pixel_size)
            .unwrap_or(fallback)
    }

    fn shaped(&mut self, line: &str, family: FontFamily, pixel_size: u32) -> Rc<[ShapedGlyph]> {
        if let Some(glyphs) = self.shaped.get(line, family, pixel_size) {
            return glyphs;
        }
        let glyphs: Rc<[ShapedGlyph]> = self.shape_line(line, family, pixel_size).into();
        self.shaped.insert(line, family, pixel_size, glyphs.clone());
        glyphs
    }

    fn shape_line(&mut self, text: &str, family: FontFamily, pixel_size: u32) -> Vec<ShapedGlyph> {
        let mut glyphs = Vec::new();
        if self.faces.is_empty() {
            return glyphs;
        }
        for run in self.font_runs(text, family) {
            let offset = run.start;
            let face = &self.faces[run.face];
            let Ok(font) = FontRef::from_index(&face.data.bytes, face.data.index) else {
                continue;
            };
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str(&text[run.start..run.end]);
            if let Some(script) = run.script {
                buffer.set_script(script);
            }
            buffer.guess_segment_properties();
            let shaper = face.shaper.shaper(&font).build();
            let script = Some(buffer.script()).filter(|script| *script != script::UNKNOWN);
            let key = (buffer.direction(), script, buffer.language());
            let mut plans = face.plans.borrow_mut();
            let plan = plans
                .entry(key)
                .or_insert_with_key(|(direction, script, language)| {
                    ShapePlan::new(&shaper, *direction, *script, language.as_ref(), &[])
                });
            let options = ShapeOptions::new()
                .plan(Some(plan))
                .scale(Some(pixel_size as i32 * 64))
                .point_size(Some(pixel_size as f32));
            let output = shaper.shape(buffer, options);
            glyphs.extend(
                output
                    .glyph_infos()
                    .iter()
                    .zip(output.glyph_positions())
                    .map(|(info, position)| ShapedGlyph {
                        face: run.face,
                        glyph: info.glyph_id,
                        cluster: offset + info.cluster as usize,
                        x_advance: position.x_advance as f32 / 64.0,
                        x_offset: position.x_offset as f32 / 64.0,
                        y_offset: position.y_offset as f32 / 64.0,
                    }),
            );
        }
        glyphs
    }

    fn font_runs(&mut self, text: &str, family: FontFamily) -> Vec<FontRun> {
        let mut runs = Vec::new();
        for run in script_runs(text) {
            let mut start = run.start;
            let mut current = None;
            for (index, character) in text[run.start..run.end].char_indices() {
                let index = run.start + index;
                let face = self.face_for(character, family).or(current).unwrap_or(0);
                match current {
                    None => current = Some(face),
                    Some(previous) if previous == face => {}
                    Some(previous) => {
                        runs.push(FontRun {
                            start,
                            end: index,
                            script: run.script,
                            face: previous,
                        });
                        start = index;
                        current = Some(face);
                    }
                }
            }
            if let Some(face) = current {
                runs.push(FontRun {
                    start,
                    end: run.end,
                    script: run.script,
                    face,
                });
            }
        }
        runs
    }

    fn face_for(&mut self, character: char, family: FontFamily) -> Option<usize> {
        if let Some(face) = self.covering(character, family) {
            return Some(face);
        }
        if family == FontFamily::Icons || !wants_fallback(character) {
            return None;
        }
        if self.load_new_fallbacks()
            && let Some(face) = self.covering(character, family)
        {
            return Some(face);
        }
        if self.fonts.missing(character) && self.load_new_fallbacks() {
            return self.covering(character, family);
        }
        None
    }

    fn covering(&mut self, character: char, family: FontFamily) -> Option<usize> {
        if let Some(face) = self.covering.get(&(family, character)) {
            return *face;
        }
        let face = self.family(family).iter().copied().find(|index| {
            let face = &self.faces[*index];
            FontRef::from_index(&face.data.bytes, face.data.index)
                .is_ok_and(|font| face.charmap.charmap(&font).map(character).is_some())
        });
        self.covering.insert((family, character), face);
        face
    }
}

impl Default for SkrifaFonts {
    fn default() -> Self {
        Self::new(FontLibrary::bundled())
    }
}

impl FontBackend for SkrifaFonts {
    fn shape(
        &mut self,
        text: &str,
        family: FontFamily,
        pixel_size: u32,
        shape: Shaping,
        pixels_per_point: f32,
    ) -> Galley {
        self.shape_galley(text, family, pixel_size, shape, pixels_per_point)
    }

    fn raster(&mut self, galley: &Galley, raster: f32) -> Galley {
        self.rastered(galley, raster)
    }

    fn generation(&self) -> u64 {
        self.fonts.generation()
    }
}

const SUBPIXEL_POSITIONS: u32 = 4;
const SHAPED_LINE_LIMIT: usize = 4096;

type Lines = HashMap<String, Rc<[ShapedGlyph]>>;

#[derive(Default)]
struct ShapedLines {
    recent: HashMap<(FontFamily, u32), Lines>,
    cooling: HashMap<(FontFamily, u32), Lines>,
    count: usize,
}

impl ShapedLines {
    fn get(&mut self, line: &str, family: FontFamily, size: u32) -> Option<Rc<[ShapedGlyph]>> {
        if let Some(glyphs) = self
            .recent
            .get(&(family, size))
            .and_then(|lines| lines.get(line))
        {
            return Some(glyphs.clone());
        }
        let glyphs = self.cooling.get_mut(&(family, size))?.remove(line)?;
        self.insert(line, family, size, glyphs.clone());
        Some(glyphs)
    }

    fn insert(&mut self, line: &str, family: FontFamily, size: u32, glyphs: Rc<[ShapedGlyph]>) {
        if self.count >= SHAPED_LINE_LIMIT {
            self.cooling = std::mem::take(&mut self.recent);
            self.count = 0;
        }
        self.recent
            .entry((family, size))
            .or_default()
            .insert(line.to_owned(), glyphs);
        self.count += 1;
    }
}

fn split_subpixel(x: f32) -> (f32, u32) {
    let positions = SUBPIXEL_POSITIONS as f32;
    let steps = (x * positions).round();
    let whole = (steps / positions).floor();
    (whole, (steps - whole * positions) as u32)
}

struct FaceData {
    data: FontData,
    scalable: bool,
    charmap: MappingIndex,
    shaper: ShaperData,
    plans: RefCell<HashMap<PlanKey, ShapePlan>>,
}

/// What a shape plan is made for, as HarfBuzz keys the plans it caches on a face.
type PlanKey = (Direction, Option<Script>, Option<Language>);

fn ink_bounds(extents: [f32; 4], pixel_size: u32, shape: Shaping) -> [f32; 4] {
    let [left, top, right, bottom] = extents;
    let size = pixel_size as f32;
    let bold = if shape.bold { size / 16.0 } else { 0.0 };
    let lean = if shape.italic {
        0.25 * top.abs().max(bottom.abs())
    } else {
        0.0
    };
    let hinting = 2.0 + size / 16.0;
    let x = hinting + bold + lean;
    let y = hinting + bold;
    [left - x, top - y, right + x, bottom + y]
}

#[derive(Clone, Copy)]
struct ShapedGlyph {
    face: usize,
    glyph: u32,
    cluster: usize,
    x_advance: f32,
    x_offset: f32,
    y_offset: f32,
}

struct FontRun {
    start: usize,
    end: usize,
    script: Option<Script>,
    face: usize,
}

struct ScriptRun {
    start: usize,
    end: usize,
    script: Option<Script>,
}

fn wants_fallback(character: char) -> bool {
    !character.is_whitespace()
        && !character.is_control()
        && !matches!(
            character,
            '\u{200b}'..='\u{200f}'
                | '\u{2060}'..='\u{206f}'
                | '\u{fe00}'..='\u{fe0f}'
                | '\u{feff}'
                | '\u{fffc}'
                | '\u{1f3fb}'..='\u{1f3ff}'
                | '\u{e0000}'..='\u{e007f}'
                | '\u{e0100}'..='\u{e01ef}'
        )
}

fn chain(primary: &[usize], others: &[&[usize]]) -> Vec<usize> {
    let mut chain = primary.to_vec();
    for other in others {
        for index in *other {
            if !chain.contains(index) {
                chain.push(*index);
            }
        }
    }
    chain
}

fn script_runs(text: &str) -> Vec<ScriptRun> {
    let scripts = CodePointMapData::<Unicode>::new();
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current = None;

    for (index, character) in text.char_indices() {
        let script = scripts.get(character);
        if matches!(script, Unicode::Common | Unicode::Inherited | Unicode::Unknown) {
            continue;
        }
        match current {
            None => current = Some(script),
            Some(previous) if previous == script => {}
            Some(_) => {
                runs.push(ScriptRun {
                    start,
                    end: index,
                    script: current.and_then(shaping_script),
                });
                start = index;
                current = Some(script);
            }
        }
    }

    if !text.is_empty() {
        runs.push(ScriptRun {
            start,
            end: text.len(),
            script: current.and_then(shaping_script),
        });
    }
    runs
}

fn shaping_script(script: Unicode) -> Option<Script> {
    let tag = PropertyNamesShort::<Unicode>::new().get_locale_script(script)?;
    Script::from_iso15924_tag(Tag::new(&tag.into_raw()))
}

#[cfg(test)]
mod tests;
