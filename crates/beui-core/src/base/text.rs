use foldhash::HashMap;
use std::any::Any;
use std::cell::{Cell, Ref, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::color::{Color32, format_hex};
use crate::font::{FontFamily, FontId, Galley, TextLayout};
use crate::font::{Overflow, TextAlign, WrapAt};
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::painter::Painter;

use crate::base::child_list::{ChildHost, ChildItem, ChildList, NodeChildren};
use crate::document::Document;
use crate::node::{Element, NodeId, NodeOf, Rects, SpaceTransform};
use crate::rich::{
    CaretHandle, OBJECT, Piece, RichLayout, RichOptions, Shaper, SpanBackground, SpanStyle,
    TextCaret, TextGlyph, TextMark, handle_shape,
};

pub const DEFAULT_FONT_SIZE: f32 = 14.0;
pub const CARET_BLINK: Duration = Duration::from_millis(530);
const CARET_FLAG: Vec2 = Vec2::new(6.0, 4.0);

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct FontPatch {
    pub size: Option<f32>,
    pub family: Option<FontFamily>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
}

impl FontPatch {
    pub fn apply(self, base: FontId) -> FontId {
        FontId {
            size: self.size.unwrap_or(base.size),
            family: self.family.unwrap_or(base.family),
            bold: self.bold.unwrap_or(base.bold),
            italic: self.italic.unwrap_or(base.italic),
        }
    }
}

impl From<FontId> for FontPatch {
    fn from(font: FontId) -> Self {
        Self {
            size: Some(font.size),
            family: Some(font.family),
            bold: Some(font.bold),
            italic: Some(font.italic),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct IconSpan {
    pub scale: f32,
    pub spacing_em: f32,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct SpanContent {
    pub text: String,
    pub font: FontPatch,
    pub color: Option<Color32>,
    pub underline: Option<bool>,
    pub strikethrough: Option<bool>,
    pub background: Option<SpanBackground>,
    pub break_after: bool,
    pub icon: Option<IconSpan>,
}

impl SpanContent {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    fn style(&self, base: SpanStyle) -> SpanStyle {
        let font = match self.icon {
            Some(icon) => FontId::icons(self.font.size.unwrap_or(base.font.size * icon.scale)),
            None => self.font.apply(base.font),
        };
        SpanStyle {
            font,
            color: self.color.unwrap_or(base.color),
            underline: self.underline.unwrap_or(base.underline),
            strikethrough: self.strikethrough.unwrap_or(base.strikethrough),
        }
    }
}

struct SpanCell {
    content: RefCell<SpanContent>,
    generation: Cell<u64>,
}

#[derive(Clone)]
pub struct SpanHandle(Rc<SpanCell>);

impl SpanHandle {
    pub fn new(content: SpanContent) -> Self {
        Self(Rc::new(SpanCell {
            content: RefCell::new(content),
            generation: Cell::new(0),
        }))
    }

    pub fn content(&self) -> Ref<'_, SpanContent> {
        self.0.content.borrow()
    }

    fn generation(&self) -> u64 {
        self.0.generation.get()
    }
}

#[derive(Clone)]
pub enum TextPart {
    Span(SpanHandle),
    Inline(NodeId),
    Anchored(NodeId),
    Gap(f32),
    Break,
}

impl ChildItem for TextPart {
    fn node(&self) -> Option<NodeId> {
        match self {
            TextPart::Span(_) | TextPart::Gap(_) | TextPart::Break => None,
            TextPart::Inline(node) | TextPart::Anchored(node) => Some(*node),
        }
    }
}

#[derive(Clone)]
struct Placed {
    rect: Rect,
    origin: Pos2,
    layout: Rc<RichLayout>,
    spans: Vec<Option<SpanHandle>>,
    ranges: Vec<Range<usize>>,
}

#[derive(Clone)]
enum Entry {
    Span(SpanHandle, u64),
    Inline(Vec2, Option<Rc<str>>),
    Gap(f32),
    Break,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Entry::Span(left, at), Entry::Span(right, other)) => {
                Rc::ptr_eq(&left.0, &right.0) && at == other
            }
            (Entry::Inline(left, stands), Entry::Inline(right, other)) => {
                left == right && stands == other
            }
            (Entry::Gap(left), Entry::Gap(right)) => left == right,
            (Entry::Break, Entry::Break) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct ShapeKey {
    options: RichOptions,
    line_height: Option<f32>,
    scale: f32,
    generation: u64,
}

struct Shaped {
    key: ShapeKey,
    entries: Vec<Entry>,
    layout: Rc<RichLayout>,
}

impl Shaped {
    fn reusable(&self, key: &ShapeKey, entries: &[Entry]) -> bool {
        let wrap = key.options.wrap_width;
        let laid = self.key.options.wrap_width;
        let serves = self.key.options.least || !key.options.least;
        let rewrapped = ShapeKey {
            options: RichOptions {
                wrap_width: wrap,
                least: key.options.least == serves,
                ..self.key.options
            },
            ..self.key
        };
        rewrapped == *key && self.entries == entries && (laid == wrap || self.layout.fits(wrap))
    }
}

type Fitted = (Rc<RichLayout>, FitKey, Rc<RichLayout>);

#[derive(Clone, Copy, PartialEq)]
struct FitKey {
    width: f32,
    align: TextAlign,
    truncate: bool,
    clamp: Option<usize>,
    scale: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FontKey {
    size: u32,
    family: FontFamily,
    bold: bool,
    italic: bool,
}

impl FontKey {
    fn of(font: FontId) -> Self {
        Self {
            size: font.size.to_bits(),
            family: font.family,
            bold: font.bold,
            italic: font.italic,
        }
    }
}

#[derive(Default)]
struct Galleys {
    key: Option<(f32, u64, Option<f32>)>,
    held: HashMap<(String, FontKey), Galley>,
}

struct CachedShaper<'a> {
    painter: &'a Painter,
    line_height: Option<f32>,
    held: &'a mut HashMap<(String, FontKey), Galley>,
    fresh: HashMap<(String, FontKey), Galley>,
    shapings: &'a Cell<u64>,
}

impl Shaper for CachedShaper<'_> {
    fn galley(&mut self, text: &str, font: FontId) -> Galley {
        let key = (text.to_owned(), FontKey::of(font));
        if let Some(galley) = self.fresh.get(&key) {
            return galley.clone();
        }
        let galley = match self.held.remove(&key) {
            Some(galley) => galley,
            None => {
                self.shapings.set(self.shapings.get() + 1);
                self.painter.layout_text(
                    text,
                    font,
                    TextLayout {
                        line_height: self.line_height,
                        ..TextLayout::DEFAULT
                    },
                )
            }
        };
        self.fresh.insert(key, galley.clone());
        galley
    }

    fn slice(&mut self, galley: &Galley, range: Range<usize>, _font: FontId) -> Galley {
        galley.slice(range, self.painter.ctx().pixels_per_point())
    }
}

pub struct TextItemNode {
    child: Option<NodeId>,
    at: Option<usize>,
    text: Option<Rc<str>>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextStyle {
    pub font: FontId,
    pub line_height: Option<f32>,
    pub color: Color32,
    pub horizontal: TextAlign,
    pub vertical: TextAlign,
    pub overflow: Overflow,
    pub underline: bool,
    pub padding: (f32, f32),
    pub hang: usize,
}

pub struct TextNode {
    string: SpanHandle,
    parts: ChildList<TextPart>,
    content: String,
    font: FontId,
    line_height: Option<f32>,
    color: Color32,
    horizontal: TextAlign,
    vertical: TextAlign,
    overflow: Overflow,
    underline: bool,
    padding: (f32, f32),
    hang: usize,
    marks: Vec<TextMark>,
    glyphs: Vec<TextGlyph>,
    carets: Vec<TextCaret>,
    since: Cell<Instant>,
    selection: Option<(Range<usize>, Color32)>,
    handles: Vec<(usize, CaretHandle, Color32)>,
    placed: Rc<RefCell<Option<Placed>>>,
    shaped: RefCell<Option<Shaped>>,
    galleys: RefCell<Galleys>,
    shapings: Cell<u64>,
    fitted: RefCell<Option<Fitted>>,
}

impl ChildHost for TextNode {
    type Stored = TextPart;

    fn children(&mut self) -> &mut ChildList<TextPart> {
        &mut self.parts
    }

    fn children_changed(&mut self) {
        self.refresh_content();
    }
}

enum Flow<'a> {
    Span(Ref<'a, SpanContent>),
    Inline(usize, Vec2, &'a str),
    Gap(f32),
    Break,
}

impl TextNode {
    pub fn accessible_text(&self) -> Option<&str> {
        let seen = |color: Color32| color.alpha() > 0;
        let shown = (!self.string.content().text.is_empty() && seen(self.color))
            || self.parts.iter().any(|part| match part {
                TextPart::Span(span) => {
                    let content = span.content();
                    content.icon.is_none()
                        && !content.text.is_empty()
                        && seen(content.color.unwrap_or(self.color))
                }
                TextPart::Inline(_) => true,
                TextPart::Anchored(_) | TextPart::Gap(_) | TextPart::Break => false,
            });
        (shown && !self.content.is_empty()).then_some(&self.content)
    }

    fn refresh_content(&mut self) {
        let mut content = self.string.content().text.clone();
        for part in self.parts.iter() {
            match part {
                TextPart::Span(span) if span.content().icon.is_some() => {}
                TextPart::Span(span) => content.push_str(&span.content().text),
                TextPart::Inline(_) => content.push_str(OBJECT),
                TextPart::Anchored(_) | TextPart::Gap(_) | TextPart::Break => {}
            }
        }
        self.content = content;
    }

    fn style(&self) -> SpanStyle {
        SpanStyle {
            font: self.font,
            color: Color32::TRANSPARENT,
            underline: false,
            strikethrough: false,
        }
    }

    fn inline_items(&self) -> Vec<NodeId> {
        self.parts
            .iter()
            .filter_map(|part| match part {
                TextPart::Inline(node) => Some(*node),
                _ => None,
            })
            .collect()
    }

    fn anchored_items(&self) -> Vec<NodeId> {
        self.parts
            .iter()
            .filter_map(|part| match part {
                TextPart::Anchored(node) => Some(*node),
                _ => None,
            })
            .collect()
    }

    fn entries(&self, doc: &mut Document, painter: &Painter) -> Vec<Entry> {
        let mut entries = vec![Entry::Span(self.string.clone(), self.string.generation())];
        for part in self.parts.iter() {
            match part {
                TextPart::Span(span) => entries.push(Entry::Span(span.clone(), span.generation())),
                TextPart::Inline(item) => entries.push(Entry::Inline(
                    crate::layout::measure(doc, painter, *item, Vec2::splat(f32::INFINITY)),
                    doc.arena
                        .get_as::<TextItemNode>(NodeOf::assumed(*item))
                        .text
                        .clone(),
                )),
                TextPart::Gap(width) => entries.push(Entry::Gap(*width)),
                TextPart::Break => entries.push(Entry::Break),
                TextPart::Anchored(_) => {}
            }
        }
        entries
    }

    fn wrap_width(&self, available_width: f32) -> f32 {
        match self.overflow {
            Overflow::Wrap { .. } => available_width.max(0.0),
            Overflow::Ellipsis | Overflow::Visible => f32::INFINITY,
        }
    }

    fn max_lines(&self) -> Option<usize> {
        match self.overflow {
            Overflow::Wrap {
                max_lines: Some(lines),
                ..
            } => Some((lines as usize).max(1)),
            _ => None,
        }
    }

    fn shaper<'a>(&'a self, painter: &'a Painter, galleys: &'a mut Galleys) -> CachedShaper<'a> {
        let ctx = painter.ctx();
        let key = (
            ctx.pixels_per_point(),
            ctx.font_generation(),
            self.line_height,
        );
        if galleys.key != Some(key) {
            galleys.key = Some(key);
            galleys.held.clear();
        }
        CachedShaper {
            painter,
            line_height: self.line_height,
            held: &mut galleys.held,
            fresh: HashMap::default(),
            shapings: &self.shapings,
        }
    }

    fn rich_layout(
        &self,
        doc: &mut Document,
        painter: &Painter,
        available_width: f32,
    ) -> Rc<RichLayout> {
        let entries = self.entries(doc, painter);
        let ctx = painter.ctx();
        let wrap_width = self.wrap_width(available_width);
        let key = ShapeKey {
            options: RichOptions {
                wrap_width,
                padding: self.padding,
                style: self.style(),
                hang: self.hang,
                anywhere: matches!(
                    self.overflow,
                    Overflow::Wrap {
                        at: WrapAt::Anywhere,
                        ..
                    }
                ),
                least: self.overflow.wraps() && !wrap_width.is_finite(),
            },
            line_height: self.line_height,
            scale: ctx.pixels_per_point(),
            generation: ctx.font_generation(),
        };
        let mut previous = self.shaped.borrow_mut().take();
        if let Some(shaped) = &previous
            && shaped.reusable(&key, &entries)
        {
            let layout = Rc::clone(&shaped.layout);
            *self.shaped.borrow_mut() = previous;
            return layout;
        }
        let changed = previous
            .as_ref()
            .filter(|shaped| shaped.key == key)
            .map(|shaped| {
                let same = shaped
                    .entries
                    .iter()
                    .zip(&entries)
                    .take_while(|(old, new)| old == new)
                    .count();
                (Rc::clone(&shaped.layout), same)
            });
        let style = key.options.style;
        let flow: Vec<Flow> = {
            let mut inline = 0;
            entries
                .iter()
                .map(|entry| match entry {
                    Entry::Span(span, _) => Flow::Span(span.content()),
                    Entry::Inline(size, text) => {
                        inline += 1;
                        Flow::Inline(inline - 1, *size, text.as_deref().unwrap_or(OBJECT))
                    }
                    Entry::Gap(width) => Flow::Gap(*width),
                    Entry::Break => Flow::Break,
                })
                .collect()
        };
        let pieces: Vec<Piece> = flow
            .iter()
            .map(|flow| match flow {
                Flow::Span(content) => match content.icon {
                    Some(icon) => {
                        let style = content.style(style);
                        Piece::Icon {
                            glyph: &content.text,
                            style,
                            spacing: icon.spacing_em * style.font.size,
                            text_size: style.font.size / icon.scale,
                        }
                    }
                    None => Piece::Text {
                        text: &content.text,
                        style: content.style(style),
                        break_after: content.break_after,
                    },
                },
                Flow::Inline(index, size, text) => Piece::Inline {
                    index: *index,
                    size: *size,
                    text,
                },
                Flow::Gap(width) => Piece::Gap(*width),
                Flow::Break => Piece::Break,
            })
            .collect();
        let mut galleys = self.galleys.borrow_mut();
        let mut shaper = self.shaper(painter, &mut galleys);
        let layout = match changed {
            Some((old, same)) => {
                let at = pieces[..same.min(pieces.len())]
                    .iter()
                    .map(Piece::len)
                    .sum();
                old.resume(at, &pieces, key.options, &mut shaper)
            }
            None => RichLayout::new(&pieces, key.options, &mut shaper),
        };
        let fresh = std::mem::take(&mut shaper.fresh);
        drop(shaper);
        galleys.held = fresh;
        drop(pieces);
        drop(flow);
        let layout = Rc::new(layout);
        previous = Some(Shaped {
            key,
            entries,
            layout: Rc::clone(&layout),
        });
        *self.shaped.borrow_mut() = previous;
        layout
    }

    fn fitted(&self, painter: &Painter, layout: Rc<RichLayout>, width: f32) -> Rc<RichLayout> {
        let truncate = self.overflow == Overflow::Ellipsis && layout.size.x > width;
        let clamp = self.max_lines().filter(|lines| layout.lines.len() > *lines);
        if !truncate && clamp.is_none() && self.horizontal == TextAlign::Start {
            return layout;
        }
        let key = FitKey {
            width,
            align: self.horizontal,
            truncate,
            clamp,
            scale: painter.ctx().pixels_per_point(),
        };
        if let Some((base, held, fitted)) = &*self.fitted.borrow()
            && Rc::ptr_eq(base, &layout)
            && *held == key
        {
            return Rc::clone(fitted);
        }
        let mut fitted = (*layout).clone();
        if truncate || clamp.is_some() {
            let mut galleys = self.galleys.borrow_mut();
            let mut shaper = self.shaper(painter, &mut galleys);
            if let Some(lines) = clamp {
                fitted.clamp(lines, width, &mut shaper);
            }
            if truncate {
                fitted.truncate(width, &mut shaper);
            }
            let fresh = std::mem::take(&mut shaper.fresh);
            drop(shaper);
            galleys.held.extend(fresh);
        }
        if self.horizontal != TextAlign::Start {
            let grid = painter.pixel_grid();
            fitted.align(width, self.horizontal, |x| grid.snap(x));
        }
        let fitted = Rc::new(fitted);
        *self.fitted.borrow_mut() = Some((layout, key, Rc::clone(&fitted)));
        fitted
    }

    fn place(&self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let layout = self.fitted(
            painter,
            self.rich_layout(doc, painter, rect.width()),
            rect.width(),
        );
        let previous = self.placed.borrow().clone();
        let top = match self.vertical {
            TextAlign::Start => rect.top(),
            TextAlign::Center => rect.center().y - layout.size.y / 2.0,
            TextAlign::End => rect.bottom() - layout.size.y,
        };
        let origin = painter.pixel_grid().snap_pos(pos2(rect.left(), top));
        let inline = self.inline_items();
        for (index, placed) in layout.inline_rects() {
            if let Some(item) = inline.get(index) {
                crate::layout::layout(doc, painter, *item, placed.translate(origin.to_vec2()), out);
            }
        }
        for item in self.anchored_items() {
            let Some(at) = doc.arena.get_as::<TextItemNode>(NodeOf::assumed(item)).at else {
                continue;
            };
            let size = crate::layout::measure(doc, painter, item, Vec2::splat(f32::INFINITY));
            let caret = layout.caret_rect(at, size.x);
            crate::layout::layout(doc, painter, item, caret.translate(origin.to_vec2()), out);
        }
        let spans: Vec<Option<SpanHandle>> = self
            .shaped
            .borrow()
            .iter()
            .flat_map(|shaped| &shaped.entries)
            .map(|entry| match entry {
                Entry::Span(span, _) => Some(span.clone()),
                Entry::Inline(..) | Entry::Gap(_) | Entry::Break => None,
            })
            .collect();
        let mut at = 0;
        let ranges: Vec<Range<usize>> = self
            .shaped
            .borrow()
            .iter()
            .flat_map(|shaped| &shaped.entries)
            .map(|entry| {
                let len = match entry {
                    Entry::Span(span, _) if span.content().icon.is_some() => 0,
                    Entry::Span(span, _) => span.content().text.len(),
                    Entry::Inline(_, text) => text.as_deref().unwrap_or(OBJECT).len(),
                    Entry::Gap(_) | Entry::Break => 0,
                };
                at += len;
                at - len..at
            })
            .collect();
        let changed = previous.is_none_or(|previous| {
            previous.rect != rect
                || previous.origin != origin
                || !Rc::ptr_eq(&previous.layout, &layout)
                || previous.spans.len() != spans.len()
                || previous.spans.iter().zip(&spans).any(|pair| match pair {
                    (Some(left), Some(right)) => !Rc::ptr_eq(&left.0, &right.0),
                    (None, None) => false,
                    _ => true,
                })
        });
        *self.placed.borrow_mut() = Some(Placed {
            rect,
            origin,
            layout,
            spans,
            ranges,
        });
        if changed && let Some(id) = doc.laying_out() {
            doc.arena.note_relaid(id);
        }
    }

    fn paint_layout(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        let Some(placed) = self.placed.borrow().clone() else {
            return;
        };
        let layout = placed.layout;
        let origin = placed.origin.to_vec2() + (rect.min - placed.rect.min);
        let paint_of = |piece: usize| match placed.spans.get(piece).and_then(Option::as_ref) {
            Some(span) => {
                let content = span.content();
                (
                    content.color.unwrap_or(self.color),
                    content.underline.unwrap_or(self.underline),
                    content.strikethrough.unwrap_or(false),
                )
            }
            None => (self.color, self.underline, false),
        };
        let fill = |range: Range<usize>, background: SpanBackground| {
            for area in layout.selection_rects(range) {
                painter.rect_filled(
                    area.expand2(background.outset).translate(origin),
                    background.radius,
                    background.color,
                );
            }
        };
        let mut run: Option<(Range<usize>, SpanBackground)> = None;
        for (span, range) in placed.spans.iter().zip(&placed.ranges) {
            let background = span.as_ref().and_then(|span| span.content().background);
            if let (Some((held, same)), Some(background)) = (&mut run, background)
                && *same == background
                && held.end == range.start
            {
                held.end = range.end;
                continue;
            }
            if background.is_none() && range.is_empty() {
                continue;
            }
            if let Some((held, background)) = run.take() {
                fill(held, background);
            }
            run = background.map(|background| (range.clone(), background));
        }
        if let Some((held, background)) = run {
            fill(held, background);
        }
        let plain = |color: Color32| SpanBackground {
            color,
            radius: 0.0,
            outset: Vec2::ZERO,
        };
        for mark in &self.marks {
            fill(mark.range.clone(), plain(mark.color));
        }
        if let Some((range, color)) = &self.selection {
            fill(range.clone(), plain(*color));
        }
        layout.paint_runs(painter, origin, |run| paint_of(run.piece));
        for glyph in &self.glyphs {
            let shaped = painter.layout_text(&glyph.glyph, glyph.font, TextLayout::DEFAULT);
            for (area, baseline) in layout.range_boxes(glyph.range.clone()) {
                let at = pos2(
                    area.center().x - shaped.size().x / 2.0,
                    baseline - shaped.baseline(),
                );
                painter.galley(at + origin, shaped.clone(), glyph.color);
            }
        }
        for item in self.parts.nodes() {
            if rects.contains_key(&item) {
                crate::paint::paint(doc, painter, rects, item);
            }
        }
        let blinking = doc.motion().animates() && self.carets.iter().any(|caret| caret.blink);
        let shown = match blinking {
            true => {
                let elapsed = painter
                    .ctx()
                    .now()
                    .saturating_duration_since(self.since.get())
                    .as_nanos();
                let interval = CARET_BLINK.as_nanos();
                let remaining = interval - elapsed % interval;
                painter
                    .ctx()
                    .request_repaint_after(Duration::from_nanos(remaining as u64));
                (elapsed / interval).is_multiple_of(2)
            }
            false => true,
        };
        for caret in &self.carets {
            if caret.blink && !shown {
                continue;
            }
            let area = layout.caret_rect(caret.at, caret.width).translate(origin);
            if let Some(handle) = caret.handle {
                let (shape, corners, angle) = handle_shape(area, handle);
                painter
                    .rotated(shape.center(), angle)
                    .rect_filled(shape, corners, caret.color);
                continue;
            }
            painter.rect_filled(area, 0.0, caret.color);
            if caret.flag {
                painter.rect_filled(
                    Rect::from_min_size(pos2(area.min.x, area.min.y - CARET_FLAG.y), CARET_FLAG),
                    0.0,
                    caret.color,
                );
            }
        }
        if !self.handles.is_empty() {
            for (index, handle, color) in &self.handles {
                let caret = layout.caret_rect(*index, 0.0).translate(origin);
                let (shape, corners, angle) = handle_shape(caret, *handle);
                painter
                    .rotated(shape.center(), angle)
                    .rect_filled(shape, corners, *color);
            }
        }
    }
}

impl Element for TextNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        let layout = self.rich_layout(doc, painter, available.x);
        match self.overflow {
            Overflow::Ellipsis => Vec2::new(layout.size.x.min(available.x.max(0.0)), layout.size.y),
            Overflow::Wrap { .. } if self.max_lines().is_some() => {
                let size = self.fitted(painter, layout, available.x).size;
                Vec2::new(size.x.min(available.x.max(0.0)), size.y)
            }
            Overflow::Wrap { .. } | Overflow::Visible => layout.size,
        }
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        match self.overflow {
            Overflow::Wrap { .. } => self.rich_layout(doc, painter, f32::INFINITY).least_width(),
            Overflow::Ellipsis | Overflow::Visible => 0.0,
        }
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        let layout = self.rich_layout(doc, painter, available.x);
        layout.lines.first().map(|line| line.top + line.baseline)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        self.place(doc, painter, rect, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        self.paint_layout(doc, painter, rects, rect);
    }

    fn children(&self) -> Vec<NodeId> {
        self.parts.nodes()
    }

    fn for_each_child(&self, visit: &mut dyn FnMut(NodeId)) {
        for child in self.parts.iter().filter_map(ChildItem::node) {
            visit(child);
        }
    }

    fn remove_child(&mut self, child: NodeId) {
        crate::base::child_list::remove_hosted(self, child);
    }

    fn kind(&self) -> &'static str {
        "text"
    }

    fn ink(&self) -> Option<Rect> {
        let placed = self.placed.borrow();
        let placed = placed.as_ref()?;
        Some(Rect::from_min_size(
            placed.origin - placed.rect.min.to_vec2(),
            placed.layout.size,
        ))
    }

    fn detail(&self) -> Option<String> {
        Some(format!("\"{}\"", self.content))
    }

    fn properties(&self) -> Vec<(&'static str, String)> {
        let styles: Vec<&str> = [
            (self.font.bold, "bold"),
            (self.font.italic, "italic"),
            (self.underline, "underline"),
            (self.font.family == FontFamily::Monospace, "monospace"),
            (self.font.family == FontFamily::Icons, "icon"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        let overflow = match self.overflow {
            Overflow::Wrap { at, max_lines } => {
                let at = match at {
                    WrapAt::Words => "wrap",
                    WrapAt::Anywhere => "wrap anywhere",
                };
                match max_lines {
                    Some(lines) => format!("{at}, at most {lines} lines"),
                    None => at.to_owned(),
                }
            }
            Overflow::Ellipsis => "ellipsis".to_owned(),
            Overflow::Visible => "visible".to_owned(),
        };
        let none = |names: Vec<&str>| match names.is_empty() {
            true => "none".to_owned(),
            false => names.join(" "),
        };
        let spans = self
            .parts
            .iter()
            .filter(|part| matches!(part, TextPart::Span(span) if span.content().icon.is_none()))
            .count();
        let icons = self
            .parts
            .iter()
            .filter(|part| matches!(part, TextPart::Span(span) if span.content().icon.is_some()))
            .count();
        vec![
            ("content", self.content.clone()),
            ("characters", self.content.chars().count().to_string()),
            ("font size", self.font.size.to_string()),
            (
                "line height",
                self.line_height
                    .map_or_else(|| "auto".to_owned(), |height| height.to_string()),
            ),
            ("color", format_hex(self.color, true)),
            (
                "align",
                format!("{:?} {:?}", self.horizontal, self.vertical),
            ),
            ("style", none(styles)),
            ("overflow", overflow),
            ("spans", spans.to_string()),
            ("icons", icons.to_string()),
            (
                "selection",
                self.selection.as_ref().map_or_else(
                    || "none".to_owned(),
                    |(range, _)| format!("{}..{}", range.start, range.end),
                ),
            ),
        ]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_text(
        &mut self,
        content: impl Into<String>,
        font_size: f32,
        color: Color32,
    ) -> NodeOf<TextNode> {
        let content = content.into();
        let since = Cell::new(self.now());
        self.arena.insert(TextNode {
            string: SpanHandle::new(SpanContent::new(content.clone())),
            parts: ChildList::default(),
            content,
            font: FontId::proportional(font_size),
            line_height: None,
            color,
            horizontal: TextAlign::Start,
            vertical: TextAlign::Start,
            overflow: Overflow::default(),
            underline: false,
            padding: (0.0, 0.0),
            hang: 0,
            marks: Vec::new(),
            glyphs: Vec::new(),
            carets: Vec::new(),
            since,
            selection: None,
            handles: Vec::new(),
            placed: Rc::new(RefCell::new(None)),
            shaped: RefCell::new(None),
            galleys: RefCell::new(Galleys::default()),
            shapings: Cell::new(0),
            fitted: RefCell::new(None),
        })
    }

    pub fn set_text(&mut self, id: NodeOf<TextNode>, content: impl Into<String>) {
        let string = self.arena.get_as::<TextNode>(id).string.clone();
        let text = content.into();
        self.update_text_span(id, &string, |span| span.text = text);
    }

    pub fn update_text_span(
        &mut self,
        text: NodeOf<TextNode>,
        span: &SpanHandle,
        change: impl FnOnce(&mut SpanContent),
    ) {
        let mut content = span.content().clone();
        change(&mut content);
        if *span.content() == content {
            return;
        }
        let moved = span.content().text != content.text || span.content().icon != content.icon;
        let painted_only = SpanContent {
            color: content.color,
            underline: content.underline,
            strikethrough: content.strikethrough,
            background: content.background,
            ..span.content().clone()
        } == content;
        *span.0.content.borrow_mut() = content;
        if self.arena.kind_of::<TextNode>(text.id()).is_none() {
            return;
        }
        if painted_only {
            self.arena.paint_mut_as::<TextNode>(text);
            return;
        }
        span.0.generation.set(span.0.generation.get() + 1);
        let node = self.arena.get_mut_as::<TextNode>(text);
        if moved {
            node.refresh_content();
        }
    }

    pub fn text(&self, id: NodeOf<TextNode>) -> &str {
        &self.arena.get_as::<TextNode>(id).content
    }

    pub fn text_icons(&self, id: NodeId) -> Vec<crate::icons::Glyph> {
        let Some(text) = self.arena.kind_of::<TextNode>(id) else {
            return Vec::new();
        };
        self.arena
            .get_as::<TextNode>(text)
            .parts
            .iter()
            .filter_map(|part| match part {
                TextPart::Span(span)
                    if span.content().icon.is_some() && !span.content().text.is_empty() =>
                {
                    crate::icons::Glyph::from_codepoint(&span.content().text)
                }
                _ => None,
            })
            .collect()
    }

    pub fn text_shapings(&self, id: NodeOf<TextNode>) -> u64 {
        self.arena.get_as::<TextNode>(id).shapings.get()
    }
    pub fn set_text_style(&mut self, text: NodeOf<TextNode>, style: TextStyle) {
        self.set_text_font(text, style.font);
        self.set_text_line_height(text, style.line_height);
        self.set_text_color(text, style.color);
        self.set_text_horizontal_align(text, style.horizontal);
        self.set_text_vertical_align(text, style.vertical);
        self.set_text_overflow(text, style.overflow);
        self.set_text_underline(text, style.underline);
        self.set_text_line_padding(text, style.padding);
        self.set_text_hang(text, style.hang);
    }

    pub fn set_text_horizontal_align(&mut self, text: NodeOf<TextNode>, horizontal: TextAlign) {
        if self.arena.get_as::<TextNode>(text).horizontal != horizontal {
            self.arena.get_mut_as::<TextNode>(text).horizontal = horizontal;
        }
    }

    pub fn set_text_vertical_align(&mut self, text: NodeOf<TextNode>, vertical: TextAlign) {
        if self.arena.get_as::<TextNode>(text).vertical != vertical {
            self.arena.get_mut_as::<TextNode>(text).vertical = vertical;
        }
    }

    pub fn set_text_overflow(&mut self, text: NodeOf<TextNode>, overflow: Overflow) {
        if self.arena.get_as::<TextNode>(text).overflow != overflow {
            self.arena.get_mut_as::<TextNode>(text).overflow = overflow;
        }
    }

    pub fn set_text_font(&mut self, text: NodeOf<TextNode>, font: FontId) {
        if self.arena.get_as::<TextNode>(text).font != font {
            self.arena.get_mut_as::<TextNode>(text).font = font;
        }
    }

    pub fn set_text_line_height(&mut self, text: NodeOf<TextNode>, line_height: Option<f32>) {
        if self.arena.get_as::<TextNode>(text).line_height != line_height {
            self.arena.get_mut_as::<TextNode>(text).line_height = line_height;
        }
    }

    pub fn set_text_color(&mut self, text: NodeOf<TextNode>, color: Color32) {
        if self.arena.get_as::<TextNode>(text).color != color {
            self.arena.paint_mut_as::<TextNode>(text).color = color;
        }
    }

    pub fn set_text_selection_handles(
        &mut self,
        text: NodeOf<TextNode>,
        handles: Vec<(usize, CaretHandle, Color32)>,
    ) {
        if self.arena.get_as::<TextNode>(text).handles != handles {
            self.arena.paint_mut_as::<TextNode>(text).handles = handles;
        }
    }

    pub fn set_text_selection(
        &mut self,
        text: NodeOf<TextNode>,
        selection: Option<(Range<usize>, Color32)>,
    ) {
        if self.arena.get_as::<TextNode>(text).selection != selection {
            self.arena.paint_mut_as::<TextNode>(text).selection = selection;
        }
    }

    pub fn texts_within(&self, root: NodeId) -> Vec<NodeOf<TextNode>> {
        let mut found = Vec::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            if !self.arena.contains(id) || self.node_rect(id).is_none() {
                continue;
            }
            if let Some(text) = self.arena.kind_of::<TextNode>(id) {
                let node = self.arena.get_as(text);
                if !node.content.is_empty() {
                    found.push(text);
                }
                continue;
            }
            pending.extend(self.arena.get(id).children().into_iter().rev());
        }
        found
    }

    pub fn set_text_underline(&mut self, text: NodeOf<TextNode>, underline: bool) {
        if self.arena.get_as::<TextNode>(text).underline != underline {
            self.arena.paint_mut_as::<TextNode>(text).underline = underline;
        }
    }
}

impl Element for TextItemNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child?, available)
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        self.child
            .map_or(0.0, |child| crate::layout::min_width(doc, painter, child))
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
    }

    fn kind(&self) -> &'static str {
        "text item"
    }

    fn detail(&self) -> Option<String> {
        self.at.map(|at| format!("at {at}"))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn set_text_line_padding(&mut self, text: NodeOf<TextNode>, padding: (f32, f32)) {
        if self.arena.get_as::<TextNode>(text).padding != padding {
            self.arena.get_mut_as::<TextNode>(text).padding = padding;
        }
    }

    pub fn set_text_hang(&mut self, text: NodeOf<TextNode>, hang: usize) {
        if self.arena.get_as::<TextNode>(text).hang != hang {
            self.arena.get_mut_as::<TextNode>(text).hang = hang;
        }
    }

    pub fn set_text_marks(&mut self, text: NodeOf<TextNode>, marks: Vec<TextMark>) {
        if self.arena.get_as::<TextNode>(text).marks != marks {
            self.arena.paint_mut_as::<TextNode>(text).marks = marks;
        }
    }

    pub fn set_text_carets(&mut self, text: NodeOf<TextNode>, carets: Vec<TextCaret>) {
        if self.arena.get_as::<TextNode>(text).carets == carets {
            return;
        }
        let now = self.now();
        let node = self.arena.paint_mut_as::<TextNode>(text);
        node.carets = carets;
        node.since.set(now);
    }

    pub fn set_text_glyphs(&mut self, text: NodeOf<TextNode>, glyphs: Vec<TextGlyph>) {
        if self.arena.get_as::<TextNode>(text).glyphs != glyphs {
            self.arena.paint_mut_as::<TextNode>(text).glyphs = glyphs;
        }
    }

    pub fn text_glyphs(&self, text: NodeOf<TextNode>) -> &[TextGlyph] {
        &self.arena.get_as::<TextNode>(text).glyphs
    }

    pub fn text_marks(&self, text: NodeOf<TextNode>) -> &[TextMark] {
        &self.arena.get_as::<TextNode>(text).marks
    }

    pub fn text_carets(&self, text: NodeOf<TextNode>) -> &[TextCaret] {
        &self.arena.get_as::<TextNode>(text).carets
    }

    pub fn text_geometry(&self, text: NodeOf<TextNode>) -> TextGeometry {
        TextGeometry {
            node: text.id(),
            placed: Rc::clone(&self.arena.get_as::<TextNode>(text).placed),
            rects: Rc::clone(&self.rects),
        }
    }

    pub fn text_caret_rect(
        &self,
        text: NodeOf<TextNode>,
        index: usize,
        width: f32,
    ) -> Option<Rect> {
        self.text_geometry(text).caret_rect(index, width)
    }

    pub fn text_index_at(&self, text: NodeOf<TextNode>, pos: Pos2) -> Option<usize> {
        let index = self.text_geometry(text).index_at(pos)?;
        Some(index.min(self.text(text).len()))
    }

    pub fn text_inline_at(&self, text: NodeOf<TextNode>, pos: Pos2) -> Option<usize> {
        self.text_geometry(text).inline_at(pos)
    }

    pub fn text_layout(&self, text: NodeOf<TextNode>) -> Option<Rc<RichLayout>> {
        Some(self.text_geometry(text).placement()?.1)
    }

    pub fn create_text_item(&mut self) -> NodeOf<TextItemNode> {
        self.arena.insert(TextItemNode {
            child: None,
            at: None,
            text: None,
        })
    }

    pub fn set_text_item_child(&mut self, item: NodeOf<TextItemNode>, child: NodeId) {
        if self.arena.get_as::<TextItemNode>(item).child != Some(child) {
            self.arena.get_mut_as::<TextItemNode>(item).child = Some(child);
        }
    }

    pub fn set_text_item_text(&mut self, item: NodeOf<TextItemNode>, text: Option<Rc<str>>) {
        if self.arena.get_as::<TextItemNode>(item).text != text {
            self.arena.get_mut_as::<TextItemNode>(item).text = text;
        }
    }

    pub fn set_text_item_at(&mut self, item: NodeOf<TextItemNode>, at: Option<usize>) {
        if self.arena.get_as::<TextItemNode>(item).at != at {
            self.arena.get_mut_as::<TextItemNode>(item).at = at;
        }
    }
}

#[derive(Clone)]
pub struct TextGeometry {
    node: NodeId,
    placed: Rc<RefCell<Option<Placed>>>,
    rects: Rc<Rects>,
}

impl TextGeometry {
    fn placement(&self) -> Option<(SpaceTransform, Rc<RichLayout>)> {
        let laid = self.rects.placed(&self.node)?;
        let placed = self.placed.borrow();
        let placed = placed.as_ref()?;
        let shift = laid.rect.min - placed.rect.min + placed.origin.to_vec2();
        let space = self.rects.transform(laid.space);
        let transform = SpaceTransform {
            offset: space.offset + shift * space.scale,
            scale: space.scale,
        };
        Some((transform, Rc::clone(&placed.layout)))
    }

    pub fn rect(&self) -> Option<Rect> {
        self.rects.get(&self.node)
    }

    pub fn scale(&self) -> f32 {
        self.rects.scale_of(&self.node)
    }

    pub fn bounds(&self) -> Option<Rect> {
        let (transform, layout) = self.placement()?;
        Some(transform.rect(Rect::from_min_size(Pos2::ZERO, layout.size)))
    }

    pub fn caret_rect(&self, index: usize, width: f32) -> Option<Rect> {
        let (transform, layout) = self.placement()?;
        Some(transform.rect(layout.caret_rect(index, width)))
    }

    pub fn index_at(&self, pos: Pos2) -> Option<usize> {
        let (transform, layout) = self.placement()?;
        Some(layout.index_at(transform.local_pos(pos)))
    }

    pub fn inline_at(&self, pos: Pos2) -> Option<usize> {
        let (transform, layout) = self.placement()?;
        layout.inline_at(transform.local_pos(pos))
    }
}
