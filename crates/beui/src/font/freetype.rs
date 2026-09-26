use std::collections::HashMap;
use std::ptr;
use std::rc::Rc;

use freetype::freetype as ft;
use harfbuzz_rs::{Face as HbFace, Font as HbFont, Owned, Tag, UnicodeBuffer, shape};
use unicode_script::{Script, UnicodeScript};

use super::{
    Advance, FontFamily, FontSources, Galley, GalleyData, GalleyLine, Glyph, GlyphId, GlyphImage,
    Shaping, TextLayout, break_lines,
};
use crate::geometry::vec2;

unsafe extern "C" {
    fn FT_GlyphSlot_Embolden(slot: ft::FT_GlyphSlot);
    fn FT_GlyphSlot_Oblique(slot: ft::FT_GlyphSlot);
}

struct Row {
    glyphs: Vec<ShapedGlyph>,
    start: usize,
    end: usize,
    advance: f32,
}

pub(super) const SUBPIXEL_POSITIONS: u32 = 4;

fn pixel_bounds(glyphs: &[Glyph]) -> [f32; 4] {
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for glyph in glyphs {
        bounds[0] = bounds[0].min(glyph.offset.x);
        bounds[1] = bounds[1].min(glyph.offset.y);
        bounds[2] = bounds[2].max(glyph.offset.x + glyph.image.width as f32);
        bounds[3] = bounds[3].max(glyph.offset.y + glyph.image.height as f32);
    }
    bounds
}

pub(super) fn split_subpixel(x: f32) -> (f32, u32) {
    let positions = SUBPIXEL_POSITIONS as f32;
    let steps = (x * positions).round();
    let whole = (steps / positions).floor();
    (whole, (steps - whole * positions) as u32)
}

struct FaceData {
    source: &'static [u8],
    face: ft::FT_Face,
    font: Owned<HbFont<'static>>,
}

#[derive(Clone, Copy)]
pub(super) struct ShapedGlyph {
    face: usize,
    glyph: u32,
    cluster: usize,
    pub(super) x_advance: f32,
    pub(super) x_offset: f32,
    y_offset: f32,
}

impl Advance for ShapedGlyph {
    fn cluster(&self) -> usize {
        self.cluster
    }

    fn x_advance(&self) -> f32 {
        self.x_advance
    }
}

pub(crate) struct Faces {
    library: ft::FT_Library,
    faces: Vec<FaceData>,
    proportional: Vec<usize>,
    monospace: Vec<usize>,
    icons: Vec<usize>,
    glyphs: HashMap<GlyphId, Rc<GlyphImage>>,
}

impl Faces {
    pub(crate) fn new(sources: &FontSources) -> Self {
        let mut library = ptr::null_mut();
        let opened = unsafe { ft::FT_Init_FreeType(&mut library) == 0 };
        let mut fonts = Self {
            library: if opened { library } else { ptr::null_mut() },
            faces: Vec::new(),
            proportional: Vec::new(),
            monospace: Vec::new(),
            icons: Vec::new(),
            glyphs: HashMap::new(),
        };
        if opened {
            fonts.load_families(sources);
        }
        fonts
    }

    fn load_families(&mut self, sources: &FontSources) {
        let proportional = self.load_chain(&sources.proportional);
        let monospace = self.load_chain(&sources.monospace);
        let fallback = self.load_chain(&sources.fallback);
        self.proportional = chain(&proportional, &[&monospace, &fallback]);
        self.monospace = chain(&monospace, &[&proportional, &fallback]);
        self.icons = self.load_chain(&sources.icons);
    }

    fn load_chain(&mut self, sources: &[&'static [u8]]) -> Vec<usize> {
        sources
            .iter()
            .filter_map(|source| self.load_face(source))
            .collect()
    }

    fn load_face(&mut self, source: &'static [u8]) -> Option<usize> {
        if let Some(index) = self
            .faces
            .iter()
            .position(|face| ptr::eq(face.source, source))
        {
            return Some(index);
        }
        let mut face = ptr::null_mut();
        unsafe {
            if ft::FT_New_Memory_Face(
                self.library,
                source.as_ptr(),
                source.len() as ft::FT_Long,
                0,
                &mut face,
            ) != 0
            {
                return None;
            }
        }
        let hb_face = HbFace::from_bytes(source, 0);
        self.faces.push(FaceData {
            source,
            face,
            font: HbFont::new(hb_face),
        });
        Some(self.faces.len() - 1)
    }

    fn family(&self, family: FontFamily) -> &[usize] {
        match family {
            FontFamily::Proportional => &self.proportional,
            FontFamily::Monospace => &self.monospace,
            FontFamily::Icons => &self.icons,
        }
    }

    pub(crate) fn generation(&mut self) -> u64 {
        0
    }

    pub(crate) fn build(
        &mut self,
        text: &str,
        family: FontFamily,
        pixel_size: u32,
        shape: Shaping,
        scale: f32,
    ) -> Galley {
        let (ascent, descent) = self.metrics(family, pixel_size);
        let line_height = (shape.line() * shape.spacing()).round().max(1.0);
        let baseline = ((line_height - (ascent - descent)) / 2.0).round() + ascent;
        let mut rows: Vec<Row> = Vec::new();
        let mut width = 0.0f32;
        let mut start = 0;

        for line in text.split('\n') {
            let shaped = self.shape_line(line, family, pixel_size);
            let runs = break_lines(&shaped, line, shape.wrap());
            let last = runs.len() - 1;
            for (index, run) in runs.into_iter().enumerate() {
                let end = start
                    + match shaped.get(run.end) {
                        Some(glyph) if index < last => glyph.cluster,
                        _ => line.len(),
                    };
                let glyphs = shaped[run].to_vec();
                let advance = glyphs.iter().map(|glyph| glyph.x_advance).sum::<f32>();
                width = width.max(advance);
                rows.push(Row {
                    glyphs,
                    start,
                    end,
                    advance,
                });
            }
            start += line.len() + 1;
        }

        let mut placed = Vec::new();
        let mut lines = Vec::new();
        let mut cursor = 0.0;
        for row in rows {
            let indent = TextLayout {
                align: shape.align,
                ..TextLayout::DEFAULT
            }
            .indent_of(width, row.advance);
            let mut pen = indent;
            let mut cursors = Vec::new();
            for glyph in &row.glyphs {
                let at = row.start + glyph.cluster;
                if cursors.last().is_none_or(|(previous, _)| *previous != at) {
                    cursors.push((at, pen / scale));
                }
                if let Some(glyph) = self.place(*glyph, shape, pixel_size, pen, cursor + baseline) {
                    placed.push(glyph);
                }
                pen += glyph.x_advance;
            }
            cursors.push((row.end, pen / scale));
            lines.push(GalleyLine {
                top: cursor / scale,
                range: cursors[0].0..row.end,
                cursors,
            });
            cursor += line_height;
        }

        Galley {
            inner: Rc::new(GalleyData {
                size: vec2(width.ceil() / scale, cursor.ceil() / scale),
                line_height: line_height / scale,
                baseline: baseline / scale,
                pixel_bounds: pixel_bounds(&placed),
                blank: placed.is_empty(),
                glyphs: placed,
                lines,
            }),
        }
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
        };
        let image = self.image(id)?;
        if image.width == 0 || image.height == 0 {
            return None;
        }
        let x = whole + image.left as f32;
        let y = (baseline - glyph.y_offset).round() - image.top as f32;
        Some(Glyph {
            id,
            image,
            offset: vec2(x, y),
        })
    }

    fn image(&mut self, key: GlyphId) -> Option<Rc<GlyphImage>> {
        if let Some(image) = self.glyphs.get(&key) {
            return Some(image.clone());
        }
        let image = Rc::new(self.rasterize(key)?);
        self.glyphs.insert(key, image.clone());
        Some(image)
    }

    fn rasterize(&self, key: GlyphId) -> Option<GlyphImage> {
        let face = self.faces.get(key.face)?.face;
        unsafe {
            if ft::FT_Set_Pixel_Sizes(face, 0, key.pixel_size) != 0 {
                return None;
            }
            if ft::FT_Load_Glyph(face, key.glyph, ft::FT_LOAD_DEFAULT as i32) != 0 {
                return None;
            }
            let slot = (*face).glyph;
            if (*slot).format == ft::FT_Glyph_Format::FT_GLYPH_FORMAT_OUTLINE {
                let shift = (key.subpixel as ft::FT_Pos * 64) / SUBPIXEL_POSITIONS as ft::FT_Pos;
                ft::FT_Outline_Translate(&(*slot).outline, shift, 0);
            }
            if key.bold {
                FT_GlyphSlot_Embolden(slot);
            }
            if key.italic {
                FT_GlyphSlot_Oblique(slot);
            }
            if ft::FT_Render_Glyph(slot, ft::FT_Render_Mode::FT_RENDER_MODE_NORMAL) != 0 {
                return None;
            }
            let bitmap = &(*slot).bitmap;
            Some(GlyphImage {
                width: bitmap.width,
                height: bitmap.rows,
                left: (*slot).bitmap_left,
                top: (*slot).bitmap_top,
                pixels: pixels(bitmap),
            })
        }
    }

    fn metrics(&self, family: FontFamily, pixel_size: u32) -> (f32, f32) {
        let fallback = (pixel_size as f32 * 0.8, pixel_size as f32 * -0.2);
        let Some(&index) = self.family(family).first() else {
            return fallback;
        };
        let face = self.faces[index].face;
        unsafe {
            if ft::FT_Set_Pixel_Sizes(face, 0, pixel_size) != 0 {
                return fallback;
            }
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

    pub(super) fn shape_line(
        &mut self,
        text: &str,
        family: FontFamily,
        pixel_size: u32,
    ) -> Vec<ShapedGlyph> {
        let mut glyphs = Vec::new();
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
                        cluster: offset + info.cluster as usize,
                        x_advance: position.x_advance as f32 / 64.0,
                        x_offset: position.x_offset as f32 / 64.0,
                        y_offset: position.y_offset as f32 / 64.0,
                    }),
            );
        }
        glyphs
    }

    fn font_runs(&self, text: &str, family: FontFamily) -> Vec<FontRun> {
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

    fn face_for(&self, character: char, family: FontFamily) -> Option<usize> {
        self.family(family).iter().copied().find(|index| unsafe {
            ft::FT_Get_Char_Index(self.faces[*index].face, character as ft::FT_ULong) != 0
        })
    }
}

impl Drop for Faces {
    fn drop(&mut self) {
        unsafe {
            for face in &self.faces {
                ft::FT_Done_Face(face.face);
            }
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
