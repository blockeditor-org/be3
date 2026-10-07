use std::any::Any;
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::color::Color32;
use crate::font::TextAlign;
use crate::font::{FontFamily, FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::painter::Painter;

use crate::base::child_list::{ChildHost, ChildItem, ChildList, NodeChildren};
use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::rich::{
    CaretHandle, OBJECT, Piece, RichLayout, RichOptions, Shaper, SpanStyle, TextCaret, TextMark,
    handle_shape,
};

pub const DEFAULT_FONT_SIZE: f32 = 14.0;
pub const CARET_BLINK: Duration = Duration::from_millis(530);
const SPAN_UNDERLINE_OFFSET: f32 = 0.12;
const SPAN_STRIKE_OFFSET: f32 = 0.32;
const SPAN_LINE_THICKNESS: f32 = 1.0 / 16.0;
const CARET_FLAG: Vec2 = Vec2::new(6.0, 4.0);

#[derive(Clone, PartialEq, Debug, Default)]
pub struct SpanContent {
    pub text: String,
    pub font: Option<FontId>,
    pub color: Option<Color32>,
    pub underline: Option<bool>,
    pub strikethrough: Option<bool>,
    pub break_after: bool,
}

impl SpanContent {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    fn style(&self, base: SpanStyle) -> SpanStyle {
        SpanStyle {
            font: self.font.unwrap_or(base.font),
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
}

impl ChildItem for TextPart {
    fn node(&self) -> Option<NodeId> {
        match self {
            TextPart::Span(_) => None,
            TextPart::Inline(node) | TextPart::Anchored(node) => Some(*node),
        }
    }

    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (TextPart::Span(left), TextPart::Span(right)) => Rc::ptr_eq(&left.0, &right.0),
            _ => self.node().is_some() && self.node() == other.node(),
        }
    }
}

#[derive(Clone)]
struct Placed {
    rect: Rect,
    origin: Pos2,
    layout: Rc<RichLayout>,
    spans: Vec<Option<SpanHandle>>,
}

#[derive(Clone)]
enum Entry {
    Span(SpanHandle, u64),
    Inline(Vec2),
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Entry::Span(left, at), Entry::Span(right, other)) => {
                Rc::ptr_eq(&left.0, &right.0) && at == other
            }
            (Entry::Inline(left), Entry::Inline(right)) => left == right,
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
        let rewrapped = ShapeKey {
            options: RichOptions {
                wrap_width: wrap,
                ..self.key.options
            },
            ..self.key
        };
        rewrapped == *key
            && self.entries == entries
            && (laid == wrap || self.layout.fits(wrap, laid))
    }
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
}

pub struct TextItemNode {
    child: Option<NodeId>,
    at: Option<usize>,
}

pub struct TextNode {
    string: SpanHandle,
    parts: ChildList<TextPart>,
    content: String,
    font_size: f32,
    line_height: Option<f32>,
    color: Color32,
    horizontal: TextAlign,
    vertical: TextAlign,
    wrap: bool,
    monospace: bool,
    bold: bool,
    italic: bool,
    icon: bool,
    clip: bool,
    underline: bool,
    ellipsis: bool,
    padding: (f32, f32),
    marks: Vec<TextMark>,
    carets: Vec<TextCaret>,
    since: Cell<Instant>,
    selection: Option<(Range<usize>, Color32)>,
    handles: Vec<(usize, CaretHandle, Color32)>,
    placed: Rc<RefCell<Option<Placed>>>,
    shaped: RefCell<Option<Shaped>>,
    galleys: RefCell<Galleys>,
    shapings: Cell<u64>,
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
    Inline(usize, Vec2),
}

impl TextNode {
    pub fn accessible_text(&self) -> Option<&str> {
        if self.icon { None } else { Some(&self.content) }
    }

    fn refresh_content(&mut self) {
        let mut content = self.string.content().text.clone();
        for part in self.parts.iter() {
            match part {
                TextPart::Span(span) => content.push_str(&span.content().text),
                TextPart::Inline(_) => content.push_str(OBJECT),
                TextPart::Anchored(_) => {}
            }
        }
        self.content = content;
    }

    fn font(&self) -> FontId {
        if self.icon {
            return FontId::icons(self.font_size);
        }
        let family = match self.monospace {
            true => FontId::monospace(self.font_size),
            false => FontId::proportional(self.font_size),
        };
        family.bold(self.bold).italic(self.italic)
    }

    fn style(&self) -> SpanStyle {
        SpanStyle {
            font: self.font(),
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
                TextPart::Inline(item) => entries.push(Entry::Inline(crate::layout::measure(
                    doc,
                    painter,
                    *item,
                    Vec2::splat(f32::INFINITY),
                ))),
                TextPart::Anchored(_) => {}
            }
        }
        entries
    }

    fn wrap_width(&self, available_width: f32) -> f32 {
        if self.wrap {
            available_width.max(0.0)
        } else {
            f32::INFINITY
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
            fresh: HashMap::new(),
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
        let key = ShapeKey {
            options: RichOptions {
                wrap_width: self.wrap_width(available_width),
                padding: self.padding,
                style: self.style(),
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
                    Entry::Inline(size) => {
                        inline += 1;
                        Flow::Inline(inline - 1, *size)
                    }
                })
                .collect()
        };
        let pieces: Vec<Piece> = flow
            .iter()
            .map(|flow| match flow {
                Flow::Span(content) => Piece::Text {
                    text: &content.text,
                    style: content.style(style),
                    break_after: content.break_after,
                },
                Flow::Inline(index, size) => Piece::Inline {
                    index: *index,
                    size: *size,
                },
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

    fn truncates(&self) -> bool {
        self.ellipsis && !self.wrap && !self.icon
    }

    fn place(&self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let mut layout = self.rich_layout(doc, painter, rect.width());
        if self.truncates() && layout.size.x > rect.width() {
            let mut galleys = self.galleys.borrow_mut();
            let mut shaper = self.shaper(painter, &mut galleys);
            let mut truncated = (*layout).clone();
            truncated.truncate(rect.width(), &mut shaper);
            let fresh = std::mem::take(&mut shaper.fresh);
            drop(shaper);
            galleys.held.extend(fresh);
            layout = Rc::new(truncated);
        }
        if self.horizontal != TextAlign::Start {
            let grid = painter.pixel_grid();
            let mut aligned = (*layout).clone();
            aligned.align(rect.width(), self.horizontal, |x| grid.snap(x));
            layout = Rc::new(aligned);
        }
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
        let spans = self
            .shaped
            .borrow()
            .iter()
            .flat_map(|shaped| &shaped.entries)
            .map(|entry| match entry {
                Entry::Span(span, _) => Some(span.clone()),
                Entry::Inline(_) => None,
            })
            .collect();
        *self.placed.borrow_mut() = Some(Placed {
            rect,
            origin,
            layout,
            spans,
        });
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
        for mark in &self.marks {
            for area in layout.selection_rects(mark.range.clone()) {
                painter.rect_filled(
                    area.expand2(mark.outset).translate(origin),
                    mark.radius,
                    mark.color,
                );
            }
        }
        if let Some((range, color)) = &self.selection {
            for area in layout.selection_rects(range.clone()) {
                painter.rect_filled(area.translate(origin), 0.0, *color);
            }
        }
        for line in &layout.lines {
            for run in &line.runs {
                let Some(galley) = &run.galley else {
                    continue;
                };
                let left = origin.x + line.run_left(run);
                let (color, underline, strikethrough) = paint_of(run.piece);
                painter.galley(
                    pos2(left, origin.y + line.run_top(run)),
                    galley.clone(),
                    color,
                );
                let size = run.style.font.size;
                let thickness = (size * SPAN_LINE_THICKNESS).max(1.0);
                let baseline = origin.y + line.top + line.baseline;
                if underline {
                    let y = baseline + (size * SPAN_UNDERLINE_OFFSET).max(1.0);
                    painter.rect_filled(
                        Rect::from_min_size(pos2(left, y), Vec2::new(run.width, thickness)),
                        0.0,
                        color,
                    );
                }
                if strikethrough {
                    let y = baseline - size * SPAN_STRIKE_OFFSET;
                    painter.rect_filled(
                        Rect::from_min_size(pos2(left, y), Vec2::new(run.width, thickness)),
                        0.0,
                        color,
                    );
                }
            }
        }
        for item in self.parts.nodes() {
            if rects.contains_key(&item) {
                crate::paint::paint(doc, painter, rects, item);
            }
        }
        let blinking = self.carets.iter().any(|caret| caret.blink);
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
                let top = painter.on_top();
                let (shape, corners, angle) = handle_shape(area, handle);
                top.rotated(shape.center(), angle)
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
            let top = painter.on_top();
            for (index, handle, color) in &self.handles {
                let caret = layout.caret_rect(*index, 0.0).translate(origin);
                let (shape, corners, angle) = handle_shape(caret, *handle);
                top.rotated(shape.center(), angle)
                    .rect_filled(shape, corners, *color);
            }
        }
    }
}

impl Element for TextNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        let size = self.rich_layout(doc, painter, available.x).size;
        match self.truncates() {
            true => Vec2::new(size.x.min(available.x.max(0.0)), size.y),
            false => size,
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
        let clipped = painter.with_clip_rect(if self.clip { rect } else { Rect::EVERYTHING });
        self.paint_layout(doc, &clipped, rects, rect);
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        children.extend(self.parts.nodes());
    }

    fn children(&self) -> Vec<NodeId> {
        self.parts.nodes()
    }

    fn kind(&self) -> &'static str {
        "text"
    }

    fn detail(&self) -> Option<String> {
        Some(format!("\"{}\"", self.content))
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
        self.arena.insert(TextNode {
            string: SpanHandle::new(SpanContent::new(content.clone())),
            parts: ChildList::default(),
            content,
            font_size,
            line_height: None,
            color,
            horizontal: TextAlign::Start,
            vertical: TextAlign::Start,
            wrap: false,
            monospace: false,
            bold: false,
            italic: false,
            icon: false,
            clip: false,
            underline: false,
            ellipsis: false,
            padding: (0.0, 0.0),
            marks: Vec::new(),
            carets: Vec::new(),
            since: Cell::new(Instant::now()),
            selection: None,
            handles: Vec::new(),
            placed: Rc::new(RefCell::new(None)),
            shaped: RefCell::new(None),
            galleys: RefCell::new(Galleys::default()),
            shapings: Cell::new(0),
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
        let moved = span.content().text != content.text;
        let painted_only = SpanContent {
            color: content.color,
            underline: content.underline,
            strikethrough: content.strikethrough,
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

    pub fn text_shapings(&self, id: NodeOf<TextNode>) -> u64 {
        self.arena.get_as::<TextNode>(id).shapings.get()
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

    pub fn set_text_wrap(&mut self, text: NodeOf<TextNode>, wrap: bool) {
        if self.arena.get_as::<TextNode>(text).wrap != wrap {
            self.arena.get_mut_as::<TextNode>(text).wrap = wrap;
        }
    }

    pub fn set_text_font_size(&mut self, text: NodeOf<TextNode>, font_size: f32) {
        if self.arena.get_as::<TextNode>(text).font_size != font_size {
            self.arena.get_mut_as::<TextNode>(text).font_size = font_size;
        }
    }

    pub fn set_text_line_height(&mut self, text: NodeOf<TextNode>, line_height: Option<f32>) {
        if self.arena.get_as::<TextNode>(text).line_height != line_height {
            self.arena.get_mut_as::<TextNode>(text).line_height = line_height;
        }
    }

    pub fn set_text_monospace(&mut self, text: NodeOf<TextNode>, monospace: bool) {
        if self.arena.get_as::<TextNode>(text).monospace != monospace {
            self.arena.get_mut_as::<TextNode>(text).monospace = monospace;
        }
    }

    pub fn set_text_bold(&mut self, text: NodeOf<TextNode>, bold: bool) {
        if self.arena.get_as::<TextNode>(text).bold != bold {
            self.arena.get_mut_as::<TextNode>(text).bold = bold;
        }
    }

    pub fn set_text_italic(&mut self, text: NodeOf<TextNode>, italic: bool) {
        if self.arena.get_as::<TextNode>(text).italic != italic {
            self.arena.get_mut_as::<TextNode>(text).italic = italic;
        }
    }

    pub fn set_text_icon(&mut self, text: NodeOf<TextNode>, icon: bool) {
        if self.arena.get_as::<TextNode>(text).icon != icon {
            self.arena.get_mut_as::<TextNode>(text).icon = icon;
        }
    }

    pub fn set_text_color(&mut self, text: NodeOf<TextNode>, color: Color32) {
        if self.arena.get_as::<TextNode>(text).color != color {
            self.arena.paint_mut_as::<TextNode>(text).color = color;
        }
    }

    pub fn set_text_clip(&mut self, text: NodeOf<TextNode>, clip: bool) {
        if self.arena.get_as::<TextNode>(text).clip != clip {
            self.arena.paint_mut_as::<TextNode>(text).clip = clip;
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
                if !node.icon && !node.content.is_empty() {
                    found.push(text);
                }
                continue;
            }
            pending.extend(self.arena.get(id).children().into_iter().rev());
        }
        found
    }

    pub fn set_text_ellipsis(&mut self, text: NodeOf<TextNode>, ellipsis: bool) {
        if self.arena.get_as::<TextNode>(text).ellipsis != ellipsis {
            self.arena.get_mut_as::<TextNode>(text).ellipsis = ellipsis;
        }
    }

    pub fn set_text_underline(&mut self, text: NodeOf<TextNode>, underline: bool) {
        if self.arena.get_as::<TextNode>(text).underline != underline {
            self.arena.paint_mut_as::<TextNode>(text).underline = underline;
        }
    }
}

impl Element for TextItemNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child?, available)
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

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        children.extend(self.child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
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
        })
    }

    pub fn set_text_item_child(&mut self, item: NodeOf<TextItemNode>, child: NodeId) {
        if self.arena.get_as::<TextItemNode>(item).child != Some(child) {
            self.arena.get_mut_as::<TextItemNode>(item).child = Some(child);
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
    fn placement(&self) -> Option<(Vec2, Rc<RichLayout>)> {
        let rect = self.rects.get(&self.node)?;
        let placed = self.placed.borrow();
        let placed = placed.as_ref()?;
        let shift = rect.min - placed.rect.min + placed.origin.to_vec2();
        Some((shift, Rc::clone(&placed.layout)))
    }

    pub fn rect(&self) -> Option<Rect> {
        self.rects.get(&self.node)
    }

    pub fn caret_rect(&self, index: usize, width: f32) -> Option<Rect> {
        let (shift, layout) = self.placement()?;
        Some(layout.caret_rect(index, width).translate(shift))
    }

    pub fn index_at(&self, pos: Pos2) -> Option<usize> {
        let (shift, layout) = self.placement()?;
        Some(layout.index_at(pos - shift))
    }

    pub fn inline_at(&self, pos: Pos2) -> Option<usize> {
        let (shift, layout) = self.placement()?;
        layout.inline_at(pos - shift)
    }
}
