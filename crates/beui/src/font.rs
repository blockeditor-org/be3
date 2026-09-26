use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;

use crate::base::TextAlign;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};

#[cfg(feature = "dom")]
mod browser;
#[cfg(not(feature = "dom"))]
mod freetype;

#[cfg(feature = "dom")]
use browser::Faces;
#[cfg(feature = "dom")]
pub(crate) use browser::{css_family, fonts_loaded};
#[cfg(not(feature = "dom"))]
use freetype::Faces;

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
    pub bold: bool,
    pub italic: bool,
}

impl FontId {
    pub fn new(size: f32, family: FontFamily) -> Self {
        Self {
            size,
            family,
            bold: false,
            italic: false,
        }
    }

    pub fn proportional(size: f32) -> Self {
        Self::new(size, FontFamily::Proportional)
    }

    pub fn monospace(size: f32) -> Self {
        Self::new(size, FontFamily::Monospace)
    }

    pub fn icons(size: f32) -> Self {
        Self::new(size, FontFamily::Icons)
    }

    pub fn bold(self, bold: bool) -> Self {
        Self { bold, ..self }
    }

    pub fn italic(self, italic: bool) -> Self {
        Self { italic, ..self }
    }
}

const LINE_HEIGHT_RATIO: f32 = 1.15;

pub fn line_height(font_size: f32) -> f32 {
    (font_size * LINE_HEIGHT_RATIO).max(1.0)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextLayout {
    pub wrap_width: f32,
    pub align: TextAlign,
    pub line_spacing: f32,
    pub line_height: Option<f32>,
}

impl TextLayout {
    pub const DEFAULT: Self = Self {
        wrap_width: f32::INFINITY,
        align: TextAlign::Start,
        line_spacing: 1.0,
        line_height: None,
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
    bold: bool,
    italic: bool,
}

pub struct GlyphImage {
    pub width: u32,
    pub height: u32,
    #[cfg_attr(feature = "dom", allow(dead_code))]
    left: i32,
    #[cfg_attr(feature = "dom", allow(dead_code))]
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
    blank: bool,
    #[cfg(feature = "dom")]
    text: String,
    #[cfg(feature = "dom")]
    css: Rc<str>,
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

    pub fn is_blank(&self) -> bool {
        self.inner.blank
    }

    #[cfg(feature = "dom")]
    pub(crate) fn css(&self) -> &str {
        &self.inner.css
    }

    #[cfg(feature = "dom")]
    pub(crate) fn text_lines(&self) -> impl Iterator<Item = (&str, Pos2)> {
        self.inner.lines.iter().map(|line| {
            let text = self.inner.text.get(line.range.clone()).unwrap_or_default();
            (text, pos2(line.x(line.range.start), line.top))
        })
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
    line: u32,
    bold: bool,
    italic: bool,
}

impl Shaping {
    fn of(font: FontId, layout: TextLayout, pixels_per_point: f32) -> Self {
        let line = layout.line_height.unwrap_or_else(|| line_height(font.size));
        Self {
            wrap: (layout.wrap_width * pixels_per_point).max(0.0).to_bits(),
            align: layout.align,
            spacing: layout.line_spacing.max(0.1).to_bits(),
            line: (line * pixels_per_point).round().max(1.0).to_bits(),
            bold: font.bold,
            italic: font.italic,
        }
    }

    fn wrap(self) -> f32 {
        f32::from_bits(self.wrap)
    }

    fn spacing(self) -> f32 {
        f32::from_bits(self.spacing)
    }

    fn line(self) -> f32 {
        f32::from_bits(self.line)
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

const GALLEY_CACHE_LIMIT: usize = 4096;

trait Advance {
    fn cluster(&self) -> usize;
    fn x_advance(&self) -> f32;
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "dom", derive(Default))]
pub struct FontSources {
    pub proportional: Vec<&'static [u8]>,
    pub monospace: Vec<&'static [u8]>,
    pub fallback: Vec<&'static [u8]>,
    pub icons: Vec<&'static [u8]>,
}

#[cfg(not(feature = "dom"))]
impl Default for FontSources {
    fn default() -> Self {
        Self {
            proportional: vec![UBUNTU_LIGHT],
            monospace: vec![HACK_REGULAR],
            fallback: vec![NOTO_EMOJI_REGULAR],
            icons: vec![ICONS_FONT],
        }
    }
}

pub(crate) struct Fonts {
    faces: Faces,
    galleys: HashMap<u64, Vec<(GalleyKey, Galley)>>,
    cooling: HashMap<u64, Vec<(GalleyKey, Galley)>>,
    cached_galleys: usize,
    generation: u64,
}

impl Fonts {
    pub(crate) fn new(sources: &FontSources) -> Self {
        let mut faces = Faces::new(sources);
        Self {
            generation: faces.generation(),
            faces,
            galleys: HashMap::new(),
            cooling: HashMap::new(),
            cached_galleys: 0,
        }
    }

    pub(crate) fn generation(&mut self) -> u64 {
        let generation = self.faces.generation();
        if generation != self.generation {
            self.generation = generation;
            self.galleys.clear();
            self.cooling.clear();
            self.cached_galleys = 0;
        }
        generation
    }

    pub(crate) fn layout(
        &mut self,
        text: &str,
        font: FontId,
        layout: TextLayout,
        pixels_per_point: f32,
    ) -> Galley {
        let pixel_size = ((font.size * pixels_per_point).round() as u32).max(1);
        let shape = Shaping::of(font, layout, pixels_per_point);
        let scale = pixels_per_point.to_bits();
        let hash = galley_hash(text, pixel_size, font.family, shape, scale);
        if let Some(galley) = self.remembered(hash, text, pixel_size, font.family, shape, scale) {
            return galley;
        }
        let galley = self
            .faces
            .build(text, font.family, pixel_size, shape, pixels_per_point);
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
}

fn break_lines<G: Advance>(glyphs: &[G], text: &str, wrap: f32) -> Vec<Range<usize>> {
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
        if index > start && width + glyph.x_advance() > wrap {
            let end = candidate.filter(|end| *end > start).unwrap_or(index);
            lines.push(start..end);
            start = end;
            width = glyphs[start..index].iter().map(Advance::x_advance).sum();
            candidate = None;
        }
        if index > start && follows_whitespace(text, glyph.cluster()) {
            candidate = Some(index);
        }
        width += glyph.x_advance();
    }

    lines.push(start..glyphs.len());
    lines
}

fn follows_whitespace(text: &str, cluster: usize) -> bool {
    text.get(..cluster)
        .and_then(|before| before.chars().next_back())
        .is_some_and(char::is_whitespace)
}

pub const ICONS_FONT: &[u8] = include_bytes!("../assets/icons/MaterialSymbolsRounded-Filled.ttf");
#[cfg(not(feature = "dom"))]
const UBUNTU_LIGHT: &[u8] = include_bytes!("../assets/fonts/Ubuntu-Light.ttf");
#[cfg(not(feature = "dom"))]
const HACK_REGULAR: &[u8] = include_bytes!("../assets/fonts/Hack-Regular.ttf");
#[cfg(not(feature = "dom"))]
const NOTO_EMOJI_REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf");

#[cfg(all(test, not(feature = "dom")))]
mod tests;
