use foldhash::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};

use foldhash::fast::FixedState;
use grapheme_segmentation::is_grapheme_boundary;
use std::cell::OnceCell;
use std::ops::Range;
use std::rc::{Rc, Weak};

use crate::color::Color32;
use crate::geometry::{Pos2, Rect, Vec2, pos2, vec2};
use crate::rich::{Piece, RichLayout, RichOptions, Shaper, SpanStyle};

pub use beui_types::text::{FontFamily, FontId, Overflow, TextAlign, WrapAt};

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
    pub raster: u32,
}

impl GlyphId {
    pub const LAID_OUT: u32 = 0x3f80_0000;

    pub fn raster(self) -> f32 {
        f32::from_bits(self.raster)
    }
}

pub struct GlyphImage {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub pixels: Vec<u8>,
    pub color: bool,
}

pub trait GlyphRasterizer {
    fn rasterize(&self, id: GlyphId) -> Option<GlyphImage>;
}

struct Rasterized;

impl GlyphRasterizer for Rasterized {
    fn rasterize(&self, _: GlyphId) -> Option<GlyphImage> {
        None
    }
}

pub struct GlyphRaster {
    bounds: [f32; 4],
    image: OnceCell<Option<GlyphImage>>,
    rasterizer: Weak<dyn GlyphRasterizer>,
}

impl GlyphRaster {
    pub fn lazy(bounds: [f32; 4], rasterizer: Weak<dyn GlyphRasterizer>) -> Self {
        Self {
            bounds,
            image: OnceCell::new(),
            rasterizer,
        }
    }

    pub fn ready(image: GlyphImage) -> Self {
        let (left, top) = (image.left as f32, -image.top as f32);
        Self {
            bounds: [
                left,
                top,
                left + image.width as f32,
                top + image.height as f32,
            ],
            image: OnceCell::from(Some(image)),
            rasterizer: Weak::<Rasterized>::new(),
        }
    }

    fn image(&self, id: GlyphId) -> Option<&GlyphImage> {
        self.image
            .get_or_init(|| self.rasterizer.upgrade()?.rasterize(id))
            .as_ref()
    }
}

#[derive(Clone)]
pub struct Glyph {
    pub id: GlyphId,
    pub origin: Vec2,
    pub raster: Rc<GlyphRaster>,
}

impl Glyph {
    pub fn image(&self) -> Option<&GlyphImage> {
        self.raster.image(self.id)
    }

    pub fn offset(&self) -> Option<Vec2> {
        let image = self.image()?;
        Some(self.origin + vec2(image.left as f32, -image.top as f32))
    }

    pub fn bounds(&self) -> [f32; 4] {
        let [left, top, right, bottom] = self.raster.bounds;
        let Vec2 { x, y } = self.origin;
        [x + left, y + top, x + right, y + bottom]
    }
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
    pixels_per_point: f32,
    glyphs: Vec<Glyph>,
    lines: Vec<GalleyLine>,
    wraps: Wraps,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Wraps {
    fits: f32,
    overflows: f32,
}

impl Wraps {
    pub const ANY: Self = Self {
        fits: f32::NEG_INFINITY,
        overflows: f32::INFINITY,
    };

    pub(crate) fn between(fits: f32, overflows: f32) -> Self {
        Self { fits, overflows }
    }

    pub fn contains(self, wrap: f32) -> bool {
        self.fits <= wrap && (wrap < self.overflows || self.overflows == f32::INFINITY)
    }
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
            if end <= start || text.get(start..end).is_none_or(single_character) {
                continue;
            }
            let inside: Vec<usize> = (start + 1..end)
                .filter(|&at| is_grapheme_boundary(text.as_bytes(), at))
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
        let after = self.cursors.partition_point(|(at, _)| *at <= index);
        after
            .checked_sub(1)
            .map_or(0.0, |found| self.cursors[found].1)
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
        pixels_per_point: f32,
    ) -> Self {
        let lines = lines
            .into_iter()
            .map(|line| line.split_clusters(text))
            .collect();
        Self::split(
            text,
            font,
            size,
            line_height,
            baseline,
            glyphs,
            lines,
            Wraps::ANY,
            pixels_per_point,
        )
    }

    fn of_rich(text: &str, strut: &Galley, rich: &RichLayout, pixels_per_point: f32) -> Self {
        let mut glyphs = Vec::new();
        let mut lines = Vec::with_capacity(rich.lines.len());
        for line in &rich.lines {
            let mut cursors: Vec<(usize, f32)> = Vec::new();
            for run in &line.runs {
                let Some(galley) = &run.galley else {
                    continue;
                };
                let (left, top) = (line.run_left(run), line.run_top(run));
                let shift = vec2(
                    (left * pixels_per_point).round(),
                    (top * pixels_per_point).round(),
                );
                glyphs.extend(galley.glyphs().iter().map(|glyph| Glyph {
                    origin: glyph.origin + shift,
                    ..glyph.clone()
                }));
                for (at, x) in galley.lines().iter().flat_map(|shaped| &shaped.cursors) {
                    let at = run.range.start + at;
                    if cursors.last().is_none_or(|(previous, _)| *previous != at) {
                        cursors.push((at, left + x));
                    }
                }
            }
            if cursors.is_empty() {
                cursors.push((line.range.start, line.left));
            }
            lines.push(GalleyLine {
                top: line.top,
                range: line.range.clone(),
                cursors,
            });
        }
        Self::split(
            text,
            strut.font(),
            rich.size,
            strut.line_height(),
            strut.baseline(),
            glyphs,
            lines,
            rich.wraps(),
            pixels_per_point,
        )
    }

    fn split(
        text: &str,
        font: FontId,
        size: Vec2,
        line_height: f32,
        baseline: f32,
        glyphs: Vec<Glyph>,
        lines: Vec<GalleyLine>,
        wraps: Wraps,
        pixels_per_point: f32,
    ) -> Self {
        Self {
            inner: Rc::new(GalleyData {
                text: text.into(),
                font,
                size,
                line_height,
                baseline,
                pixel_bounds: pixel_bounds(&glyphs),
                pixels_per_point,
                glyphs,
                lines,
                wraps,
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
    pub fn with_glyphs(&self, glyphs: Vec<Glyph>, pixels_per_point: f32) -> Galley {
        let inner = &self.inner;
        Galley {
            inner: Rc::new(GalleyData {
                text: Rc::clone(&inner.text),
                font: inner.font,
                size: inner.size,
                line_height: inner.line_height,
                baseline: inner.baseline,
                pixel_bounds: pixel_bounds(&glyphs),
                pixels_per_point,
                glyphs,
                lines: inner
                    .lines
                    .iter()
                    .map(|line| GalleyLine {
                        top: line.top,
                        range: line.range.clone(),
                        cursors: line.cursors.clone(),
                    })
                    .collect(),
                wraps: inner.wraps,
            }),
        }
    }

    pub fn same_layout(&self, other: &Self) -> bool {
        let (left, right) = (&self.inner, &other.inner);
        Rc::ptr_eq(left, right)
            || (left.text == right.text
                && left.font == right.font
                && left.size == right.size
                && left.line_height == right.line_height
                && left.baseline == right.baseline
                && left.pixels_per_point == right.pixels_per_point
                && left.glyphs.len() == right.glyphs.len()
                && left
                    .glyphs
                    .iter()
                    .zip(&right.glyphs)
                    .all(|(glyph, other)| glyph.id == other.id && glyph.origin == other.origin))
    }

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

    pub fn slice(&self, range: Range<usize>, pixels_per_point: f32) -> Galley {
        let Some(line) = self
            .inner
            .lines
            .iter()
            .find(|line| line.range.start <= range.start && range.end <= line.range.end)
        else {
            return self.clone();
        };
        let (left, right) = (line.x(range.start), line.x(range.end));
        let shift = vec2(
            (left * pixels_per_point).round(),
            (line.top * pixels_per_point).round(),
        );
        let (from, to) = (left * pixels_per_point, right * pixels_per_point);
        let (top, bottom) = (
            line.top * pixels_per_point,
            (line.top + self.inner.line_height) * pixels_per_point,
        );
        let glyphs = &self.inner.glyphs;
        let key = |glyph: &Glyph| {
            let [min_x, min_y, max_x, max_y] = glyph.bounds();
            let (x, y) = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
            match () {
                _ if y < top => 0,
                _ if y >= bottom => 4,
                _ if x < from => 1,
                _ if x < to => 2,
                _ => 3,
            }
        };
        let first = glyphs.partition_point(|glyph| key(glyph) < 2);
        let last = glyphs.partition_point(|glyph| key(glyph) < 3);
        let glyphs = glyphs[first..last.max(first)]
            .iter()
            .map(|glyph| Glyph {
                origin: glyph.origin - shift,
                ..glyph.clone()
            })
            .collect();
        let dx = shift.x / pixels_per_point;
        let start = line.cursors.partition_point(|(at, _)| *at < range.start);
        let end = line.cursors.partition_point(|(at, _)| *at <= range.end);
        let cursors = line.cursors[start..end.max(start)]
            .iter()
            .map(|(at, x)| (at - range.start, x - dx))
            .collect();
        let text = self.inner.text.get(range.clone()).unwrap_or_default();
        Galley::split(
            text,
            self.inner.font,
            vec2(right - dx, self.inner.line_height),
            self.inner.line_height,
            self.inner.baseline,
            glyphs,
            vec![GalleyLine {
                top: 0.0,
                range: 0..text.len(),
                cursors,
            }],
            Wraps::ANY,
            self.inner.pixels_per_point,
        )
    }

    pub fn wraps(&self) -> Wraps {
        self.inner.wraps
    }

    pub fn glyphs(&self) -> &[Glyph] {
        &self.inner.glyphs
    }

    pub fn pixel_bounds(&self) -> [f32; 4] {
        self.inner.pixel_bounds
    }

    pub fn pixels_per_point(&self) -> f32 {
        self.inner.pixels_per_point
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
    align: TextAlign,
    scale: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Shaping {
    line: u32,
    pub bold: bool,
    pub italic: bool,
}

impl Shaping {
    fn of(font: FontId, layout: TextLayout, pixels_per_point: f32) -> Self {
        let line = layout.line_height.unwrap_or_else(|| line_height(font.size));
        let line = (line * pixels_per_point).round().max(1.0);
        Self {
            line: (line * layout.line_spacing.max(0.1))
                .round()
                .max(1.0)
                .to_bits(),
            bold: font.bold,
            italic: font.italic,
        }
    }

    pub fn line(self) -> f32 {
        f32::from_bits(self.line)
    }
}

#[derive(Clone, Copy)]
struct Request<'a> {
    text: &'a str,
    size: u32,
    family: FontFamily,
    shape: Shaping,
    align: TextAlign,
    scale: u32,
}

impl Request<'_> {
    fn matches(&self, key: &GalleyKey) -> bool {
        key.size == self.size
            && key.family == self.family
            && key.shape == self.shape
            && key.align == self.align
            && key.scale == self.scale
            && key.text == self.text
    }

    fn hash(&self) -> u64 {
        let mut hasher = FixedState::with_seed(0).build_hasher();
        self.text.hash(&mut hasher);
        self.size.hash(&mut hasher);
        self.family.hash(&mut hasher);
        self.shape.hash(&mut hasher);
        self.align.hash(&mut hasher);
        self.scale.hash(&mut hasher);
        hasher.finish()
    }

    fn key(&self) -> GalleyKey {
        GalleyKey {
            text: self.text.to_owned(),
            size: self.size,
            family: self.family,
            shape: self.shape,
            align: self.align,
            scale: self.scale,
        }
    }
}

pub const GALLEY_CACHE_LIMIT: usize = 4096;
const GALLEY_WIDTHS_PER_TEXT: usize = 4;

fn pixel_bounds(glyphs: &[Glyph]) -> [f32; 4] {
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for glyph in glyphs {
        let [left, top, right, bottom] = glyph.bounds();
        bounds[0] = bounds[0].min(left);
        bounds[1] = bounds[1].min(top);
        bounds[2] = bounds[2].max(right);
        bounds[3] = bounds[3].max(bottom);
    }
    bounds
}

fn single_character(text: &str) -> bool {
    let mut characters = text.chars();
    characters.next();
    characters.next().is_none()
}

pub trait FontBackend {
    fn shape(
        &mut self,
        line: &str,
        family: FontFamily,
        pixel_size: u32,
        shape: Shaping,
        pixels_per_point: f32,
    ) -> Galley;

    fn raster(&mut self, galley: &Galley, _raster: f32) -> Galley {
        galley.clone()
    }

    fn generation(&self) -> u64 {
        0
    }
}

type Buckets = HashMap<u64, Vec<(GalleyKey, Galley)>>;

#[derive(Default)]
struct GalleyCache {
    recent: Buckets,
    cooling: Buckets,
    count: usize,
}

impl GalleyCache {
    fn find(&mut self, hash: u64, fits: impl Fn(&GalleyKey, &Galley) -> bool) -> Option<Galley> {
        let fits = |(key, galley): &(GalleyKey, Galley)| fits(key, galley);
        if let Some(bucket) = self.recent.get_mut(&hash)
            && let Some(found) = bucket.iter().position(fits)
        {
            let entry = bucket.remove(found);
            let galley = entry.1.clone();
            bucket.push(entry);
            return Some(galley);
        }
        let bucket = self.cooling.get_mut(&hash)?;
        let found = bucket.iter().position(fits)?;
        let (key, galley) = bucket.swap_remove(found);
        self.remember(hash, key, galley.clone());
        Some(galley)
    }

    fn remember(&mut self, hash: u64, key: GalleyKey, galley: Galley) {
        if self.count >= GALLEY_CACHE_LIMIT {
            self.cooling = std::mem::take(&mut self.recent);
            self.count = 0;
        }
        let bucket = self.recent.entry(hash).or_default();
        let widths = bucket.iter().filter(|(other, _)| *other == key).count();
        if widths >= GALLEY_WIDTHS_PER_TEXT
            && let Some(oldest) = bucket.iter().position(|(other, _)| *other == key)
        {
            bucket.remove(oldest);
        } else {
            self.count += 1;
        }
        bucket.push((key, galley));
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

pub struct Fonts {
    backend: Box<dyn FontBackend>,
    lines: GalleyCache,
    wrapped: GalleyCache,
    backend_generation: u64,
    generation: u64,
}

struct LineShaper<'a> {
    fonts: &'a mut Fonts,
    layout: TextLayout,
    pixels_per_point: f32,
}

impl Shaper for LineShaper<'_> {
    fn galley(&mut self, text: &str, font: FontId) -> Galley {
        self.fonts
            .layout(text, font, self.layout, self.pixels_per_point)
    }

    fn slice(&mut self, galley: &Galley, range: Range<usize>, _font: FontId) -> Galley {
        galley.slice(range, self.pixels_per_point)
    }
}

impl Fonts {
    pub fn new(backend: impl FontBackend + 'static) -> Self {
        Self::boxed(Box::new(backend))
    }

    pub fn boxed(backend: Box<dyn FontBackend>) -> Self {
        Self {
            backend_generation: backend.generation(),
            backend,
            lines: GalleyCache::default(),
            wrapped: GalleyCache::default(),
            generation: 0,
        }
    }

    pub fn replace(&mut self, backend: Box<dyn FontBackend>) -> Box<dyn FontBackend> {
        self.backend_generation = backend.generation();
        self.forget();
        std::mem::replace(&mut self.backend, backend)
    }

    pub fn generation(&mut self) -> u64 {
        let backend = self.backend.generation();
        if backend != self.backend_generation {
            self.backend_generation = backend;
            self.forget();
        }
        self.generation
    }

    fn forget(&mut self) {
        self.generation += 1;
        self.lines.clear();
        self.wrapped.clear();
    }

    pub fn layout(
        &mut self,
        text: &str,
        font: FontId,
        layout: TextLayout,
        pixels_per_point: f32,
    ) -> Galley {
        self.generation();
        let request = Request {
            text,
            size: ((font.size * pixels_per_point).round() as u32).max(1),
            family: font.family,
            shape: Shaping::of(font, layout, pixels_per_point),
            align: TextAlign::Start,
            scale: pixels_per_point.to_bits(),
        };
        if !text.contains('\n') {
            let line = self.line(request, pixels_per_point);
            if line.size().x <= layout.wrap_width {
                return line;
            }
        }
        let request = Request {
            align: layout.align,
            ..request
        };
        let hash = request.hash();
        let wrap = layout.wrap_width;
        if let Some(galley) = self.wrapped.find(hash, |key, galley| {
            request.matches(key) && galley.wraps().contains(wrap)
        }) {
            return galley;
        }
        let galley = self.lay_out(text, font, layout, pixels_per_point);
        self.wrapped.remember(hash, request.key(), galley.clone());
        galley
    }

    pub fn rastered(&mut self, galley: &Galley, raster: f32) -> Galley {
        if raster == 1.0 || !raster.is_finite() || raster <= 0.0 || galley.glyphs().is_empty() {
            return galley.clone();
        }
        self.generation();
        self.backend.raster(galley, raster)
    }

    fn line(&mut self, request: Request, pixels_per_point: f32) -> Galley {
        let hash = request.hash();
        if let Some(galley) = self.lines.find(hash, |key, _| request.matches(key)) {
            return galley;
        }
        let galley = self.backend.shape(
            request.text,
            request.family,
            request.size,
            request.shape,
            pixels_per_point,
        );
        self.lines.remember(hash, request.key(), galley.clone());
        galley
    }

    fn lay_out(
        &mut self,
        text: &str,
        font: FontId,
        layout: TextLayout,
        pixels_per_point: f32,
    ) -> Galley {
        let style = SpanStyle::new(font, Color32::TRANSPARENT);
        let options = RichOptions {
            wrap_width: layout.wrap_width,
            padding: (0.0, 0.0),
            style,
            hang: 0,
            anywhere: false,
            least: false,
        };
        let mut shaper = LineShaper {
            fonts: self,
            layout: TextLayout {
                wrap_width: f32::INFINITY,
                align: TextAlign::Start,
                ..layout
            },
            pixels_per_point,
        };
        let strut = shaper.galley("", font);
        let mut rich = RichLayout::new(&[Piece::text(text, style)], options, &mut shaper);
        let width = rich.size.x;
        rich.align(width, layout.align, |x| {
            (x * pixels_per_point).round() / pixels_per_point
        });
        Galley::of_rich(text, &strut, &rich, pixels_per_point)
    }
}
