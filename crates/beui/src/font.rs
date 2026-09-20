use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::ptr;
use std::rc::Rc;

use freetype::freetype as ft;
use harfbuzz_rs::{Face as HbFace, Font as HbFont, Owned, Tag, UnicodeBuffer, shape};
use unicode_script::{Script, UnicodeScript};

use crate::base::TextAlign;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FontFamily {
    Proportional,
    Monospace,
    Icons,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FontId {
    pub size: f32,
    pub family: FontFamily,
}

impl FontId {
    pub fn proportional(size: f32) -> Self {
        Self {
            size,
            family: FontFamily::Proportional,
        }
    }

    pub fn monospace(size: f32) -> Self {
        Self {
            size,
            family: FontFamily::Monospace,
        }
    }

    pub fn icons(size: f32) -> Self {
        Self {
            size,
            family: FontFamily::Icons,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextLayout {
    pub wrap_width: f32,
    pub align: TextAlign,
    pub line_spacing: f32,
}

impl TextLayout {
    pub const DEFAULT: Self = Self {
        wrap_width: f32::INFINITY,
        align: TextAlign::Start,
        line_spacing: 1.0,
    };

    pub fn wrapped(wrap_width: f32) -> Self {
        Self {
            wrap_width,
            ..Self::DEFAULT
        }
    }

    fn indent_of(self, width: f32, line: f32) -> f32 {
        match self.align {
            TextAlign::Start => 0.0,
            TextAlign::Center => ((width - line) * 0.5).round(),
            TextAlign::End => (width - line).round(),
        }
    }
}

impl Default for TextLayout {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GlyphId {
    face: usize,
    glyph: u32,
    pixel_size: u32,
    subpixel: u32,
}

pub struct GlyphImage {
    pub width: u32,
    pub height: u32,
    left: i32,
    top: i32,
    pub pixels: Vec<u8>,
}

#[derive(Clone)]
pub struct Glyph {
    pub id: GlyphId,
    pub image: Rc<GlyphImage>,
    pub offset: Vec2,
}

#[derive(Clone)]
pub struct Galley {
    inner: Rc<GalleyData>,
}

struct GalleyData {
    size: Vec2,
    line_height: f32,
    baseline: f32,
    pixel_bounds: [f32; 4],
    glyphs: Vec<Glyph>,
    lines: Vec<GalleyLine>,
}

struct GalleyLine {
    top: f32,
    range: Range<usize>,
    cursors: Vec<(usize, f32)>,
}

impl GalleyLine {
    fn x(&self, index: usize) -> f32 {
        let mut x = 0.0;
        for (at, candidate) in &self.cursors {
            if *at > index {
                break;
            }
            x = *candidate;
        }
        x
    }
}

impl PartialEq for Galley {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Galley {
    pub fn size(&self) -> Vec2 {
        self.inner.size
    }

    pub fn line_height(&self) -> f32 {
        self.inner.line_height
    }

    pub fn baseline(&self) -> f32 {
        self.inner.baseline
    }

    pub fn line_rects(&self, origin: Pos2) -> Vec<Rect> {
        self.inner
            .lines
            .iter()
            .map(|line| {
                let top = origin.y + line.top;
                Rect::from_min_max(
                    pos2(origin.x + line.x(line.range.start), top),
                    pos2(
                        origin.x + line.x(line.range.end),
                        top + self.inner.line_height,
                    ),
                )
            })
            .filter(|rect| rect.width() > 0.0)
            .collect()
    }

    pub fn cursor_pos(&self, origin: Pos2, index: usize) -> Pos2 {
        match self.line_of(index) {
            Some(line) => origin + vec2(line.x(index), line.top),
            None => origin,
        }
    }

    pub fn cursor_at(&self, origin: Pos2, pos: Pos2) -> usize {
        let Some(line) = self.line_at(pos.y - origin.y) else {
            return 0;
        };
        let target = pos.x - origin.x;
        let mut closest = line.range.start;
        let mut distance = f32::INFINITY;
        for (index, x) in &line.cursors {
            let candidate = (*x - target).abs();
            if candidate < distance {
                distance = candidate;
                closest = *index;
            }
        }
        closest
    }

    pub fn selection_rects(&self, origin: Pos2, range: Range<usize>) -> Vec<Rect> {
        self.inner
            .lines
            .iter()
            .filter_map(|line| {
                let start = range.start.max(line.range.start);
                let end = range.end.min(line.range.end);
                if start >= end {
                    return None;
                }
                let top = origin.y + line.top;
                Some(Rect::from_min_max(
                    pos2(origin.x + line.x(start), top),
                    pos2(origin.x + line.x(end), top + self.inner.line_height),
                ))
            })
            .collect()
    }

    pub fn glyphs(&self) -> &[Glyph] {
        &self.inner.glyphs
    }

    pub fn pixel_bounds(&self) -> [f32; 4] {
        self.inner.pixel_bounds
    }

    fn line_of(&self, index: usize) -> Option<&GalleyLine> {
        self.inner
            .lines
            .iter()
            .find(|line| index <= line.range.end)
            .or_else(|| self.inner.lines.last())
    }

    fn line_at(&self, y: f32) -> Option<&GalleyLine> {
        self.inner
            .lines
            .iter()
            .find(|line| y < line.top + self.inner.line_height)
            .or_else(|| self.inner.lines.last())
    }
}

#[derive(Clone, PartialEq, Eq)]
struct GalleyKey {
    text: String,
    size: u32,
    family: FontFamily,
    shape: Shaping,
    scale: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Shaping {
    wrap: u32,
    align: TextAlign,
    spacing: u32,
}

impl Shaping {
    fn of(layout: TextLayout, pixels_per_point: f32) -> Self {
        Self {
            wrap: (layout.wrap_width * pixels_per_point).max(0.0).to_bits(),
            align: layout.align,
            spacing: layout.line_spacing.max(0.1).to_bits(),
        }
    }

    fn wrap(self) -> f32 {
        f32::from_bits(self.wrap)
    }

    fn spacing(self) -> f32 {
        f32::from_bits(self.spacing)
    }
}

impl GalleyKey {
    fn matches(
        &self,
        text: &str,
        size: u32,
        family: FontFamily,
        shape: Shaping,
        scale: u32,
    ) -> bool {
        self.size == size
            && self.family == family
            && self.shape == shape
            && self.scale == scale
            && self.text == text
    }
}

fn galley_hash(text: &str, size: u32, family: FontFamily, shape: Shaping, scale: u32) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    size.hash(&mut hasher);
    family.hash(&mut hasher);
    shape.hash(&mut hasher);
    scale.hash(&mut hasher);
    hasher.finish()
}

struct Row {
    glyphs: Vec<ShapedGlyph>,
    start: usize,
    end: usize,
    advance: f32,
}

const GALLEY_CACHE_LIMIT: usize = 4096;
const SUBPIXEL_POSITIONS: u32 = 4;

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

fn split_subpixel(x: f32) -> (f32, u32) {
    let positions = SUBPIXEL_POSITIONS as f32;
    let steps = (x * positions).round();
    let whole = (steps / positions).floor();
    (whole, (steps - whole * positions) as u32)
}

#[derive(Clone, Debug)]
pub struct FontSources {
    pub proportional: Vec<&'static [u8]>,
    pub monospace: Vec<&'static [u8]>,
    pub fallback: Vec<&'static [u8]>,
    pub icons: Vec<&'static [u8]>,
}

impl Default for FontSources {
    fn default() -> Self {
        Self {
            proportional: vec![epaint_default_fonts::UBUNTU_LIGHT],
            monospace: vec![epaint_default_fonts::HACK_REGULAR],
            fallback: vec![epaint_default_fonts::NOTO_EMOJI_REGULAR],
            icons: vec![ICONS_FONT],
        }
    }
}

struct FaceData {
    source: &'static [u8],
    face: ft::FT_Face,
    font: Owned<HbFont<'static>>,
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

pub(crate) struct Fonts {
    library: ft::FT_Library,
    faces: Vec<FaceData>,
    proportional: Vec<usize>,
    monospace: Vec<usize>,
    icons: Vec<usize>,
    glyphs: HashMap<GlyphId, Rc<GlyphImage>>,
    galleys: HashMap<u64, Vec<(GalleyKey, Galley)>>,
    cooling: HashMap<u64, Vec<(GalleyKey, Galley)>>,
    cached_galleys: usize,
}

impl Fonts {
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
            galleys: HashMap::new(),
            cooling: HashMap::new(),
            cached_galleys: 0,
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

    pub(crate) fn layout(
        &mut self,
        text: &str,
        font: FontId,
        layout: TextLayout,
        pixels_per_point: f32,
    ) -> Galley {
        let pixel_size = ((font.size * pixels_per_point).round() as u32).max(1);
        let shape = Shaping::of(layout, pixels_per_point);
        let scale = pixels_per_point.to_bits();
        let hash = galley_hash(text, pixel_size, font.family, shape, scale);
        if let Some(galley) = self.remembered(hash, text, pixel_size, font.family, shape, scale) {
            return galley;
        }
        let galley = self.build(text, font.family, pixel_size, shape, pixels_per_point);
        if self.cached_galleys >= GALLEY_CACHE_LIMIT {
            self.cooling = std::mem::take(&mut self.galleys);
            self.cached_galleys = 0;
        }
        let key = GalleyKey {
            text: text.to_owned(),
            size: pixel_size,
            family: font.family,
            shape,
            scale,
        };
        self.remember(hash, key, galley.clone());
        galley
    }

    fn remembered(
        &mut self,
        hash: u64,
        text: &str,
        size: u32,
        family: FontFamily,
        shape: Shaping,
        scale: u32,
    ) -> Option<Galley> {
        if let Some(bucket) = self.galleys.get(&hash)
            && let Some((_, galley)) = bucket
                .iter()
                .find(|(key, _)| key.matches(text, size, family, shape, scale))
        {
            return Some(galley.clone());
        }
        let bucket = self.cooling.get_mut(&hash)?;
        let found = bucket
            .iter()
            .position(|(key, _)| key.matches(text, size, family, shape, scale))?;
        let (key, galley) = bucket.swap_remove(found);
        self.remember(hash, key, galley.clone());
        Some(galley)
    }

    fn remember(&mut self, hash: u64, key: GalleyKey, galley: Galley) {
        self.galleys.entry(hash).or_default().push((key, galley));
        self.cached_galleys += 1;
    }

    fn build(
        &mut self,
        text: &str,
        family: FontFamily,
        pixel_size: u32,
        shape: Shaping,
        scale: f32,
    ) -> Galley {
        let (ascent, natural) = self.metrics(family, pixel_size);
        let line_height = natural * shape.spacing();
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
                wrap_width: f32::INFINITY,
                align: shape.align,
                line_spacing: 1.0,
            }
            .indent_of(width, row.advance);
            let mut pen = indent;
            let mut cursors = Vec::new();
            for glyph in &row.glyphs {
                let at = row.start + glyph.cluster;
                if cursors.last().is_none_or(|(previous, _)| *previous != at) {
                    cursors.push((at, pen / scale));
                }
                if let Some(glyph) = self.place(*glyph, pixel_size, pen, cursor + ascent) {
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
                baseline: ascent / scale,
                pixel_bounds: pixel_bounds(&placed),
                glyphs: placed,
                lines,
            }),
        }
    }

    fn place(
        &mut self,
        glyph: ShapedGlyph,
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
        let fallback = (pixel_size as f32 * 0.8, pixel_size as f32 * 1.2);
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
            (metrics.ascender as f32 / 64.0, metrics.height as f32 / 64.0)
        }
    }

    fn shape_line(&mut self, text: &str, family: FontFamily, pixel_size: u32) -> Vec<ShapedGlyph> {
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

impl Drop for Fonts {
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

fn break_lines(glyphs: &[ShapedGlyph], text: &str, wrap: f32) -> Vec<std::ops::Range<usize>> {
    let mut lines = Vec::new();
    if glyphs.is_empty() {
        lines.push(0..0);
        return lines;
    }
    if !wrap.is_finite() {
        lines.push(0..glyphs.len());
        return lines;
    }

    let mut start = 0;
    let mut width = 0.0;
    let mut candidate = None;

    for (index, glyph) in glyphs.iter().enumerate() {
        if index > start && width + glyph.x_advance > wrap {
            let end = candidate.filter(|end| *end > start).unwrap_or(index);
            lines.push(start..end);
            start = end;
            width = glyphs[start..index].iter().map(|it| it.x_advance).sum();
            candidate = None;
        }
        if index > start && follows_whitespace(text, glyph.cluster) {
            candidate = Some(index);
        }
        width += glyph.x_advance;
    }

    lines.push(start..glyphs.len());
    lines
}

fn follows_whitespace(text: &str, cluster: usize) -> bool {
    text.get(..cluster)
        .and_then(|before| before.chars().next_back())
        .is_some_and(char::is_whitespace)
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

pub const ICONS_FONT: &[u8] = include_bytes!("../assets/icons/MaterialSymbolsRounded-Filled.ttf");

#[cfg(test)]
mod tests;
