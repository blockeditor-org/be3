mod color;
mod library;
#[cfg(not(target_arch = "wasm32"))]
pub mod system;

use std::cell::{Cell, RefCell};
use std::ptr;
use std::rc::Rc;

use foldhash::HashMap;
use freetype::freetype as ft;
use harfbuzz_rs::{Blob, Face as HbFace, Font as HbFont, Owned, Tag, UnicodeBuffer, shape};
use unicode_script::{Script, UnicodeScript};

use beui_core::font::{
    FontBackend, FontFamily, FontId, Galley, GalleyLine, Glyph, GlyphId, GlyphImage, GlyphRaster,
    GlyphRasterizer, Shaping,
};
use beui_core::geometry::vec2;

pub use library::{FontBytes, FontData, FontLibrary, FontSources, ICONS_FONT};

pub struct FreetypeFonts {
    library: ft::FT_Library,
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
}

// unsafe(ffi)
impl FreetypeFonts {
    pub fn new(fonts: FontLibrary) -> Self {
        let mut library = ptr::null_mut();
        let opened = unsafe { ft::FT_Init_FreeType(&mut library) == 0 };
        let mut backend = Self {
            library: if opened { library } else { ptr::null_mut() },
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
        };
        backend.load_families();
        backend
    }

    fn load_families(&mut self) {
        if self.library.is_null() {
            return;
        }
        let sources = self.fonts.sources();
        let proportional = self.load_chain(&sources.proportional);
        let monospace = self.load_chain(&sources.monospace);
        let fallback = self.load_chain(&sources.fallback);
        self.fallbacks = sources.fallback.len();
        self.proportional = chain(&proportional, &[&monospace, &fallback]);
        self.monospace = chain(&monospace, &[&proportional, &fallback]);
        self.icons = self.load_chain(&sources.icons);
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
        if self.library.is_null() || self.fonts.fallback_count() <= self.fallbacks {
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
        let mut face = ptr::null_mut();
        unsafe {
            if ft::FT_New_Memory_Face(
                self.library,
                source.bytes.as_ptr(),
                source.bytes.len() as ft::FT_Long,
                source.index as ft::FT_Long,
                &mut face,
            ) != 0
            {
                return None;
            }
        }
        let blob = Blob::with_bytes_owned(source.bytes.clone(), |bytes| bytes);
        let hb_face = HbFace::new(blob, source.index);
        self.rasterizer.faces.borrow_mut().push(RasterFace {
            face,
            size: Cell::new(None),
        });
        self.faces.push(FaceData {
            data: source.clone(),
            face,
            font: HbFont::new(hb_face),
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
                let bounds = ink_bounds(glyph.extents?, pixel_size, shape);
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

    fn metrics(&self, family: FontFamily, pixel_size: u32) -> (f32, f32) {
        let fallback = (pixel_size as f32 * 0.8, pixel_size as f32 * -0.2);
        let Some(&index) = self.family(family).first() else {
            return fallback;
        };
        if self.rasterizer.sized(index, pixel_size) != Some(1.0) {
            return fallback;
        }
        let face = self.faces[index].face;
        unsafe {
            let size = (*face).size;
            if size.is_null() {
                return fallback;
            }
            let metrics = (*size).metrics;
            (
                metrics.ascender as f32 / 64.0,
                metrics.descender as f32 / 64.0,
            )
        }
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
            let face = &mut self.faces[run.face];
            let scale = pixel_size as i32 * 64;
            face.font.set_scale(scale, scale);
            face.font.set_ppem(pixel_size, pixel_size);
            let mut buffer = UnicodeBuffer::new().add_str(&text[run.start..run.end]);
            if let Some(script) = run.script {
                let tag = script.as_iso15924_tag().to_be_bytes();
                buffer = buffer.set_script(Tag::new(
                    tag[0] as char,
                    tag[1] as char,
                    tag[2] as char,
                    tag[3] as char,
                ));
            }
            let output = shape(&face.font, buffer.guess_segment_properties(), &[]);
            glyphs.extend(
                output
                    .get_glyph_infos()
                    .iter()
                    .zip(output.get_glyph_positions())
                    .map(|(info, position)| ShapedGlyph {
                        face: run.face,
                        glyph: info.codepoint,
                        extents: extents(&face.font, info.codepoint, pixel_size),
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

    fn covering(&self, character: char, family: FontFamily) -> Option<usize> {
        self.family(family).iter().copied().find(|index| unsafe {
            ft::FT_Get_Char_Index(self.faces[*index].face, character as ft::FT_ULong) != 0
        })
    }
}

impl Default for FreetypeFonts {
    fn default() -> Self {
        Self::new(FontLibrary::bundled())
    }
}

impl FontBackend for FreetypeFonts {
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

// unsafe(ffi)
unsafe extern "C" {
    fn FT_GlyphSlot_Embolden(slot: ft::FT_GlyphSlot);
    fn FT_GlyphSlot_Oblique(slot: ft::FT_GlyphSlot);
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
    face: ft::FT_Face,
    font: Owned<HbFont<'static>>,
}

struct RasterFace {
    face: ft::FT_Face,
    size: Cell<Option<(u32, Option<f32>)>>,
}

#[derive(Default)]
struct Rasterizer {
    faces: RefCell<Vec<RasterFace>>,
}

// unsafe(ffi)
impl Rasterizer {
    fn sized(&self, index: usize, pixel_size: u32) -> Option<f32> {
        let faces = self.faces.borrow();
        let face = faces.get(index)?;
        if let Some((size, scale)) = face.size.get()
            && size == pixel_size
        {
            return scale;
        }
        let scale = unsafe { color::set_size(face.face, pixel_size) };
        face.size.set(Some((pixel_size, scale)));
        scale
    }
}

// unsafe(ffi)
impl GlyphRasterizer for Rasterizer {
    fn rasterize(&self, key: GlyphId) -> Option<GlyphImage> {
        let scale = self.sized(key.face, key.pixel_size)?;
        let face = self.faces.borrow().get(key.face)?.face;
        let raster = key.raster();
        let scaled = key.raster != GlyphId::LAID_OUT;
        let flags = match scaled {
            true => ft::FT_LOAD_COLOR | ft::FT_LOAD_NO_HINTING,
            false => ft::FT_LOAD_COLOR,
        };
        unsafe {
            if ft::FT_Load_Glyph(face, key.glyph, flags as i32) != 0 {
                return None;
            }
            let slot = (*face).glyph;
            if key.bold {
                FT_GlyphSlot_Embolden(slot);
            }
            if key.italic {
                FT_GlyphSlot_Oblique(slot);
            }
            let outline = (*slot).format == ft::FT_Glyph_Format::FT_GLYPH_FORMAT_OUTLINE;
            if outline && scaled {
                let fixed = (raster * 65536.0).round() as ft::FT_Fixed;
                let matrix = ft::FT_Matrix {
                    xx: fixed,
                    xy: 0,
                    yx: 0,
                    yy: fixed,
                };
                ft::FT_Outline_Transform(&(*slot).outline, &matrix);
            }
            if outline {
                let shift = (key.subpixel as ft::FT_Pos * 64) / SUBPIXEL_POSITIONS as ft::FT_Pos;
                ft::FT_Outline_Translate(&(*slot).outline, shift, 0);
            }
            if ft::FT_Render_Glyph(slot, ft::FT_Render_Mode::FT_RENDER_MODE_NORMAL) != 0 {
                return None;
            }
            let bitmap = &(*slot).bitmap;
            let (left, top) = ((*slot).bitmap_left, (*slot).bitmap_top);
            if bitmap.pixel_mode == ft::FT_Pixel_Mode::FT_PIXEL_MODE_BGRA as u8 {
                return Some(color::image(bitmap, left, top, scale * raster));
            }
            Some(GlyphImage {
                width: bitmap.width,
                height: bitmap.rows,
                left,
                top,
                pixels: pixels(bitmap),
                color: false,
            })
        }
    }
}

// unsafe(ffi)
impl Drop for Rasterizer {
    fn drop(&mut self) {
        for face in self.faces.get_mut().drain(..) {
            unsafe {
                ft::FT_Done_Face(face.face);
            }
        }
    }
}

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
    extents: Option<[f32; 4]>,
    cluster: usize,
    x_advance: f32,
    x_offset: f32,
    y_offset: f32,
}

fn extents(font: &HbFont<'static>, glyph: u32, pixel_size: u32) -> Option<[f32; 4]> {
    let Some(extents) = font.get_glyph_extents(glyph) else {
        let size = pixel_size as f32;
        return Some([-size, -2.0 * size, 2.0 * size, size]);
    };
    if extents.width == 0 || extents.height == 0 {
        return None;
    }
    let left = extents.x_bearing as f32 / 64.0;
    let top = -extents.y_bearing as f32 / 64.0;
    Some([
        left,
        top,
        left + extents.width as f32 / 64.0,
        top - extents.height as f32 / 64.0,
    ])
}

// unsafe(ffi)
impl Drop for FreetypeFonts {
    fn drop(&mut self) {
        self.close_faces();
        unsafe {
            if !self.library.is_null() {
                ft::FT_Done_FreeType(self.library);
            }
        }
    }
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
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current = None;

    for (index, character) in text.char_indices() {
        let script = character.script();
        if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
            continue;
        }
        match current {
            None => current = Some(script),
            Some(previous) if previous == script => {}
            Some(_) => {
                runs.push(ScriptRun {
                    start,
                    end: index,
                    script: current,
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
            script: current,
        });
    }
    runs
}

// unsafe(ffi)
fn pixels(bitmap: &ft::FT_Bitmap) -> Vec<u8> {
    let width = bitmap.width as usize;
    let rows = bitmap.rows as usize;
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    if bitmap.buffer.is_null() || width == 0 || rows == 0 {
        return Vec::new();
    }
    let buffer = unsafe { std::slice::from_raw_parts(bitmap.buffer, pitch * rows) };
    let mut pixels = vec![0; width * rows];
    for row in 0..rows {
        let source = if bitmap.pitch >= 0 {
            row
        } else {
            rows - 1 - row
        };
        for column in 0..width {
            if let Some(value) = buffer.get(source * pitch + column) {
                pixels[row * width + column] = *value;
            }
        }
    }
    pixels
}

#[cfg(test)]
mod tests;
