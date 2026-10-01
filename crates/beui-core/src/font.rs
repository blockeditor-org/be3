use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};

use foldhash::fast::FixedState;
use std::ops::Range;
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;

use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

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

    pub fn indent_of(self, width: f32, line: f32) -> f32 {
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
    pub face: usize,
    pub glyph: u32,
    pub pixel_size: u32,
    pub subpixel: u32,
    pub bold: bool,
    pub italic: bool,
}

pub struct GlyphImage {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
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
    text: Rc<str>,
    font: FontId,
    size: Vec2,
    line_height: f32,
    baseline: f32,
    pixel_bounds: [f32; 4],
    glyphs: Vec<Glyph>,
    lines: Vec<GalleyLine>,
}

pub struct GalleyLine {
    pub top: f32,
    pub range: Range<usize>,
    pub cursors: Vec<(usize, f32)>,
}

impl GalleyLine {
    fn split_clusters(mut self, text: &str) -> Self {
        let mut cursors = Vec::with_capacity(self.cursors.len());
        for pair in self.cursors.windows(2) {
            let ((start, left), (end, right)) = (pair[0], pair[1]);
            cursors.push((start, left));
            let inside: Vec<usize> = text
                .get(start..end)
                .into_iter()
                .flat_map(|cluster| cluster.grapheme_indices(true))
                .map(|(offset, _)| start + offset)
                .filter(|at| *at > start)
                .collect();
            let parts = inside.len() + 1;
            for (step, at) in inside.into_iter().enumerate() {
                let share = (step + 1) as f32 / parts as f32;
                cursors.push((at, left + (right - left) * share));
            }
        }
        cursors.extend(self.cursors.last().copied());
        self.cursors = cursors;
        self
    }

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

impl Galley {
    pub fn new(
        text: &str,
        font: FontId,
        size: Vec2,
        line_height: f32,
        baseline: f32,
        glyphs: Vec<Glyph>,
        lines: Vec<GalleyLine>,
    ) -> Self {
        let lines = lines
            .into_iter()
            .map(|line| line.split_clusters(text))
            .collect();
        Self {
            inner: Rc::new(GalleyData {
                text: text.into(),
                font,
                size,
                line_height,
                baseline,
                pixel_bounds: pixel_bounds(&glyphs),
                glyphs,
                lines,
            }),
        }
    }
}

impl PartialEq for Galley {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Galley {
    pub fn text(&self) -> &str {
        &self.inner.text
    }

    pub fn font(&self) -> FontId {
        self.inner.font
    }

    pub fn is_blank(&self) -> bool {
        self.inner.text.chars().all(char::is_whitespace)
    }

    pub fn lines(&self) -> &[GalleyLine] {
        &self.inner.lines
    }

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
pub struct Shaping {
    wrap: u32,
    pub align: TextAlign,
    spacing: u32,
    line: u32,
    pub bold: bool,
    pub italic: bool,
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

    pub fn wrap(self) -> f32 {
        f32::from_bits(self.wrap)
    }

    pub fn spacing(self) -> f32 {
        f32::from_bits(self.spacing)
    }

    pub fn line(self) -> f32 {
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
    let mut hasher = FixedState::with_seed(0).build_hasher();
    text.hash(&mut hasher);
    size.hash(&mut hasher);
    family.hash(&mut hasher);
    shape.hash(&mut hasher);
    scale.hash(&mut hasher);
    hasher.finish()
}

pub const GALLEY_CACHE_LIMIT: usize = 4096;

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

pub fn break_lines<T>(
    steps: &[T],
    text: &str,
    wrap: f32,
    step: impl Fn(&T) -> (usize, f32),
) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    if steps.is_empty() {
        lines.push(0..0);
        return lines;
    }
    if !wrap.is_finite() {
        lines.push(0..steps.len());
        return lines;
    }

    let advance = |item: &T| step(item).1;
    let mut start = 0;
    let mut width = 0.0;
    let mut candidate = None;

    for (index, item) in steps.iter().enumerate() {
        let (cluster, x_advance) = step(item);
        if index > start && width + x_advance > wrap {
            let end = candidate.filter(|end| *end > start).unwrap_or(index);
            lines.push(start..end);
            start = end;
            width = steps[start..index].iter().map(advance).sum();
            candidate = None;
        }
        if index > start && follows_whitespace(text, cluster) {
            candidate = Some(index);
        }
        width += x_advance;
    }

    lines.push(start..steps.len());
    lines
}

fn follows_whitespace(text: &str, cluster: usize) -> bool {
    text.get(..cluster)
        .and_then(|before| before.chars().next_back())
        .is_some_and(char::is_whitespace)
}

pub trait FontBackend {
    fn build(
        &mut self,
        text: &str,
        family: FontFamily,
        pixel_size: u32,
        shape: Shaping,
        pixels_per_point: f32,
    ) -> Galley;

    fn generation(&self) -> u64 {
        0
    }
}

pub struct Fonts {
    backend: Box<dyn FontBackend>,
    galleys: HashMap<u64, Vec<(GalleyKey, Galley)>>,
    cooling: HashMap<u64, Vec<(GalleyKey, Galley)>>,
    cached_galleys: usize,
    generation: u64,
}

impl Fonts {
    pub fn new(backend: impl FontBackend + 'static) -> Self {
        Self {
            backend: Box::new(backend),
            galleys: HashMap::new(),
            cooling: HashMap::new(),
            cached_galleys: 0,
            generation: 0,
        }
    }

    pub fn generation(&mut self) -> u64 {
        let generation = self.backend.generation();
        if generation != self.generation {
            self.generation = generation;
            self.galleys.clear();
            self.cooling.clear();
            self.cached_galleys = 0;
        }
        generation
    }

    pub fn layout(
        &mut self,
        text: &str,
        font: FontId,
        layout: TextLayout,
        pixels_per_point: f32,
    ) -> Galley {
        let pixel_size = ((font.size * pixels_per_point).round() as u32).max(1);
        let shape = Shaping::of(font, layout, pixels_per_point);
        let scale = pixels_per_point.to_bits();
        self.generation();
        let hash = galley_hash(text, pixel_size, font.family, shape, scale);
        if let Some(galley) = self.remembered(hash, text, pixel_size, font.family, shape, scale) {
            return galley;
        }
        let galley = self
            .backend
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
