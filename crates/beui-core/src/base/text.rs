use std::any::Any;
use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::color::Color32;
use crate::font::TextAlign;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::painter::Painter;
use crate::pixel_grid::PixelGrid;

use crate::base::child_list::{ChildHost, ChildItem, ChildList};
use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::rich::{
    CaretHandle, RichLayout, RichOptions, SpanStyle, TextCaret, TextMark, TextSpan, handle_shape,
};

const UNDERLINE_OFFSET: f32 = 0.1;
const UNDERLINE_THICKNESS: f32 = 0.07;
const UNDERLINE_MINIMUM_THICKNESS: f32 = 1.0;
const ELLIPSIS: &str = "\u{2026}";
pub const DEFAULT_FONT_SIZE: f32 = 14.0;
pub const CARET_BLINK: Duration = Duration::from_millis(530);
const SPAN_UNDERLINE_OFFSET: f32 = 0.12;
const SPAN_STRIKE_OFFSET: f32 = 0.32;
const SPAN_LINE_THICKNESS: f32 = 1.0 / 16.0;
const CARET_FLAG: Vec2 = Vec2::new(6.0, 4.0);

#[derive(Clone)]
struct Placed {
    rect: Rect,
    galley: Galley,
    origin: Pos2,
    rich: Option<Rc<RichLayout>>,
}

struct Rich {
    spans: Vec<TextSpan>,
    padding: (f32, f32),
    marks: Vec<TextMark>,
    carets: Vec<TextCaret>,
    since: Cell<Instant>,
}

impl Default for Rich {
    fn default() -> Self {
        Self {
            spans: Vec::new(),
            padding: (0.0, 0.0),
            marks: Vec::new(),
            carets: Vec::new(),
            since: Cell::new(Instant::now()),
        }
    }
}

struct Shaped {
    strut: Galley,
    content: String,
    spans: Vec<TextSpan>,
    options: RichOptions,
    sizes: Vec<Vec2>,
    layout: Rc<RichLayout>,
}

pub struct TextItemNode {
    child: Option<NodeId>,
    at: Option<usize>,
}

pub struct TextNode {
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
    selection: Option<(Range<usize>, Color32)>,
    handles: Vec<(usize, CaretHandle, Color32)>,
    rich: Option<Box<Rich>>,
    items: ChildList<NodeId>,
    placed: Rc<RefCell<Option<Placed>>>,
    shaped: RefCell<Option<Shaped>>,
}

impl ChildHost for TextNode {
    type Stored = NodeId;

    fn children(&mut self) -> &mut ChildList<NodeId> {
        &mut self.items
    }
}

impl TextNode {
    pub fn accessible_text(&self) -> Option<&str> {
        if self.icon { None } else { Some(&self.content) }
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
            color: self.color,
            underline: self.underline,
            strikethrough: false,
        }
    }

    fn inline_items(&self, doc: &Document) -> Vec<NodeId> {
        self.items
            .iter()
            .copied()
            .filter(|item| {
                doc.arena
                    .get_as::<TextItemNode>(NodeOf::assumed(*item))
                    .at
                    .is_none()
            })
            .collect()
    }

    fn rich_layout(
        &self,
        doc: &mut Document,
        painter: &Painter,
        rich: &Rich,
        available_width: f32,
    ) -> Rc<RichLayout> {
        let inline = self.inline_items(doc);
        let sizes: Vec<Vec2> = inline
            .iter()
            .map(|item| crate::layout::measure(doc, painter, *item, Vec2::splat(f32::INFINITY)))
            .collect();
        let options = RichOptions {
            wrap_width: self.wrap_width(available_width),
            padding: rich.padding,
            style: self.style(),
        };
        let strut = painter.layout_text(String::new(), options.style.font, TextLayout::DEFAULT);
        if let Some(shaped) = self.shaped.borrow().as_ref()
            && shaped.strut == strut
            && shaped.options == options
            && shaped.sizes == sizes
            && shaped.content == self.content
            && shaped.spans == rich.spans
        {
            return Rc::clone(&shaped.layout);
        }
        let size_of = |index: usize| sizes.get(index).copied().unwrap_or(Vec2::ZERO);
        let mut shaper = |text: &str, font: FontId| {
            painter.layout_text(text.to_owned(), font, TextLayout::DEFAULT)
        };
        let layout = Rc::new(RichLayout::new(
            &self.content,
            &rich.spans,
            &size_of,
            options,
            &mut shaper,
        ));
        *self.shaped.borrow_mut() = Some(Shaped {
            strut,
            content: self.content.clone(),
            spans: rich.spans.clone(),
            options,
            sizes,
            layout: Rc::clone(&layout),
        });
        layout
    }

    fn place_rich(
        &self,
        doc: &mut Document,
        painter: &Painter,
        rich: &Rich,
        rect: Rect,
        out: &Rects,
    ) {
        let layout = self.rich_layout(doc, painter, rich, rect.width());
        let origin = painter.pixel_grid().snap_pos(rect.min);
        let inline = self.inline_items(doc);
        for (index, placed) in layout.inline_rects() {
            if let Some(item) = inline.get(index) {
                crate::layout::layout(doc, painter, *item, placed.translate(origin.to_vec2()), out);
            }
        }
        for item in self.items.iter().copied() {
            let Some(at) = doc.arena.get_as::<TextItemNode>(NodeOf::assumed(item)).at else {
                continue;
            };
            let size = crate::layout::measure(doc, painter, item, Vec2::splat(f32::INFINITY));
            let caret = layout.caret_rect(at, size.x);
            crate::layout::layout(doc, painter, item, caret.translate(origin.to_vec2()), out);
        }
        *self.placed.borrow_mut() = Some(Placed {
            rect,
            galley: painter.layout_text(String::new(), self.font(), TextLayout::DEFAULT),
            origin,
            rich: Some(layout),
        });
    }

    fn paint_rich(
        &self,
        doc: &Document,
        painter: &Painter,
        rects: &Rects,
        rich: &Rich,
        rect: Rect,
    ) {
        let Some(placed) = self
            .placed
            .borrow()
            .clone()
            .filter(|placed| placed.rect == rect)
        else {
            return;
        };
        let Some(layout) = placed.rich else {
            return;
        };
        let origin = placed.origin.to_vec2();
        for mark in &rich.marks {
            for area in layout.selection_rects(mark.range.clone()) {
                painter.rect_filled(
                    area.expand2(mark.outset).translate(origin),
                    mark.radius,
                    mark.color,
                );
            }
        }
        for line in &layout.lines {
            for run in &line.runs {
                let Some(galley) = &run.galley else {
                    continue;
                };
                let left = origin.x + run.x;
                painter.galley(
                    pos2(left, origin.y + line.run_top(run)),
                    galley.clone(),
                    run.style.color,
                );
                let size = run.style.font.size;
                let thickness = (size * SPAN_LINE_THICKNESS).max(1.0);
                let baseline = origin.y + line.top + line.baseline;
                if run.style.underline {
                    let y = baseline + (size * SPAN_UNDERLINE_OFFSET).max(1.0);
                    painter.rect_filled(
                        Rect::from_min_size(pos2(left, y), Vec2::new(run.width, thickness)),
                        0.0,
                        run.style.color,
                    );
                }
                if run.style.strikethrough {
                    let y = baseline - size * SPAN_STRIKE_OFFSET;
                    painter.rect_filled(
                        Rect::from_min_size(pos2(left, y), Vec2::new(run.width, thickness)),
                        0.0,
                        run.style.color,
                    );
                }
            }
        }
        for item in self.items.iter() {
            if rects.contains_key(item) {
                crate::paint::paint(doc, painter, rects, *item);
            }
        }
        let blinking = rich.carets.iter().any(|caret| caret.blink);
        let shown = match blinking {
            true => {
                let elapsed = painter
                    .ctx()
                    .now()
                    .saturating_duration_since(rich.since.get())
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
        for caret in &rich.carets {
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
    }

    fn wrap_width(&self, available_width: f32) -> f32 {
        if self.wrap {
            available_width.max(0.0)
        } else {
            f32::INFINITY
        }
    }

    fn galley(&self, painter: &Painter, text: &str, available_width: f32) -> Galley {
        painter.layout_text(
            text.to_owned(),
            self.font(),
            TextLayout {
                wrap_width: self.wrap_width(available_width),
                line_height: self.line_height,
                ..TextLayout::DEFAULT
            },
        )
    }

    fn truncates(&self) -> bool {
        self.ellipsis && !self.wrap && !self.icon
    }

    fn fitted(&self, painter: &Painter, width: f32) -> Galley {
        let galley = self.galley(painter, &self.content, width);
        if !self.truncates() || galley.size().x <= width {
            return galley;
        }
        let ellipsis = self.galley(painter, ELLIPSIS, width).size().x;
        let budget = width - ellipsis;
        let kept = galley.lines().first().map_or(0, |line| {
            line.cursors
                .iter()
                .filter(|(at, x)| *x <= budget && *at <= line.range.end)
                .map(|(at, _)| *at)
                .max()
                .unwrap_or(0)
        });
        let kept = self.content.get(..kept).unwrap_or_default().trim_end();
        self.galley(painter, &format!("{kept}{ELLIPSIS}"), width)
    }

    fn origin(&self, grid: PixelGrid, size: Vec2, rect: Rect) -> Pos2 {
        let x = match self.horizontal {
            TextAlign::Start => rect.left(),
            TextAlign::Center => rect.center().x - size.x / 2.0,
            TextAlign::End => rect.right() - size.x,
        };
        let y = match self.vertical {
            TextAlign::Start => rect.top(),
            TextAlign::Center => rect.center().y - size.y / 2.0,
            TextAlign::End => rect.bottom() - size.y,
        };
        grid.snap_pos(pos2(x, y))
    }

    fn placed(&self, painter: &Painter, rect: Rect) -> Placed {
        if let Some(placed) = self
            .placed
            .borrow()
            .as_ref()
            .filter(|placed| placed.rect == rect)
        {
            return placed.clone();
        }
        self.place(painter, rect)
    }

    fn place(&self, painter: &Painter, rect: Rect) -> Placed {
        let galley = self.fitted(painter, rect.width());
        let origin = self.origin(painter.pixel_grid(), galley.size(), rect);
        let placed = Placed {
            rect,
            galley,
            origin,
            rich: None,
        };
        *self.placed.borrow_mut() = Some(placed.clone());
        placed
    }

    fn underline_rects(&self, galley: &Galley, origin: Pos2) -> Vec<Rect> {
        let thickness = (self.font_size * UNDERLINE_THICKNESS).max(UNDERLINE_MINIMUM_THICKNESS);
        let top = galley.baseline() + self.font_size * UNDERLINE_OFFSET;
        galley
            .line_rects(origin)
            .into_iter()
            .map(|line| {
                Rect::from_min_max(
                    pos2(line.left(), line.top() + top),
                    pos2(line.right(), line.top() + top + thickness),
                )
            })
            .collect()
    }
}

impl Element for TextNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match &self.rich {
            Some(rich) => self.rich_layout(doc, painter, rich, available.x).size,
            None => {
                let size = self.galley(painter, &self.content, available.x).size();
                match self.truncates() {
                    true => Vec2::new(size.x.min(available.x.max(0.0)), size.y),
                    false => size,
                }
            }
        }
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        match &self.rich {
            Some(rich) => {
                let layout = self.rich_layout(doc, painter, rich, available.x);
                layout.lines.first().map(|line| line.top + line.baseline)
            }
            None => {
                let galley = self.galley(painter, &self.content, available.x);
                let top = galley.lines().first().map_or(0.0, |line| line.top);
                Some(top + galley.baseline())
            }
        }
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        match self.rich.take() {
            Some(rich) => {
                self.place_rich(doc, painter, &rich, rect, out);
                self.rich = Some(rich);
            }
            None => {
                self.place(painter, rect);
            }
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        if let Some(rich) = &self.rich {
            let clipped = painter.with_clip_rect(if self.clip { rect } else { Rect::EVERYTHING });
            self.paint_rich(doc, &clipped, rects, rich, rect);
            return;
        }
        let clipped = painter.with_clip_rect(if self.clip { rect } else { Rect::EVERYTHING });
        let placed = self.placed(&clipped, rect);
        if let Some((range, color)) = &self.selection {
            let end = range.end.min(placed.galley.text().len());
            for selected in placed
                .galley
                .selection_rects(placed.origin, range.start.min(end)..end)
            {
                clipped.rect_filled(selected, 0.0, *color);
            }
        }
        clipped.galley(placed.origin, placed.galley.clone(), self.color);
        if !self.handles.is_empty() {
            let top = painter.on_top();
            for (index, handle, color) in &self.handles {
                let caret = plain_caret_rect(&placed.galley, placed.origin, *index);
                let (shape, corners, angle) = handle_shape(caret, *handle);
                top.rotated(shape.center(), angle)
                    .rect_filled(shape, corners, *color);
            }
        }
        if self.underline {
            for line in self.underline_rects(&placed.galley, placed.origin) {
                clipped.rect_filled(line, 0.0, self.color);
            }
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
        children.extend(self.items.iter().map(ChildItem::node));
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.nodes()
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
        self.arena.insert(TextNode {
            content: content.into(),
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
            selection: None,
            handles: Vec::new(),
            rich: None,
            items: ChildList::default(),
            placed: Rc::new(RefCell::new(None)),
            shaped: RefCell::new(None),
        })
    }

    pub fn set_text(&mut self, id: NodeOf<TextNode>, content: impl Into<String>) {
        let value = content.into();
        if self.arena.get_as::<TextNode>(id).content != value {
            self.arena.get_mut_as::<TextNode>(id).content = value;
        }
    }

    pub fn text(&self, id: NodeOf<TextNode>) -> &str {
        &self.arena.get_as::<TextNode>(id).content
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
    fn text_rich(&mut self, text: NodeOf<TextNode>) -> &mut Rich {
        self.arena
            .get_mut_as::<TextNode>(text)
            .rich
            .get_or_insert_with(Box::default)
    }

    pub fn set_text_spans(&mut self, text: NodeOf<TextNode>, spans: Vec<TextSpan>) {
        let node = self.arena.get_as::<TextNode>(text);
        if node.rich.as_ref().is_some_and(|rich| rich.spans == spans) {
            return;
        }
        self.text_rich(text).spans = spans;
    }

    pub fn set_text_line_padding(&mut self, text: NodeOf<TextNode>, padding: (f32, f32)) {
        let node = self.arena.get_as::<TextNode>(text);
        if node
            .rich
            .as_ref()
            .is_some_and(|rich| rich.padding == padding)
        {
            return;
        }
        self.text_rich(text).padding = padding;
    }

    pub fn set_text_marks(&mut self, text: NodeOf<TextNode>, marks: Vec<TextMark>) {
        let node = self.arena.get_as::<TextNode>(text);
        if node
            .rich
            .as_ref()
            .map_or(marks.is_empty(), |rich| rich.marks == marks)
        {
            return;
        }
        self.arena
            .paint_mut_as::<TextNode>(text)
            .rich
            .get_or_insert_with(Box::default)
            .marks = marks;
    }

    pub fn set_text_carets(&mut self, text: NodeOf<TextNode>, carets: Vec<TextCaret>) {
        let node = self.arena.get_as::<TextNode>(text);
        if node
            .rich
            .as_ref()
            .map_or(carets.is_empty(), |rich| rich.carets == carets)
        {
            return;
        }
        let now = self.now();
        let rich = self
            .arena
            .paint_mut_as::<TextNode>(text)
            .rich
            .get_or_insert_with(Box::default);
        rich.carets = carets;
        rich.since.set(now);
    }

    pub fn text_marks(&self, text: NodeOf<TextNode>) -> &[TextMark] {
        self.arena
            .get_as::<TextNode>(text)
            .rich
            .as_ref()
            .map_or(&[], |rich| rich.marks.as_slice())
    }

    pub fn text_carets(&self, text: NodeOf<TextNode>) -> &[TextCaret] {
        self.arena
            .get_as::<TextNode>(text)
            .rich
            .as_ref()
            .map_or(&[], |rich| rich.carets.as_slice())
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
        if let Some(rect) = self.text_geometry(text).caret_rect(index, width) {
            return Some(rect);
        }
        let rect = self.node_rect(text)?;
        let node = self.arena.get_as::<TextNode>(text);
        let placed = node.placed.borrow();
        let placed = placed.as_ref()?;
        let origin = rect.min + (placed.origin - placed.rect.min);
        let caret = plain_caret_rect(&placed.galley, origin, index);
        Some(Rect::from_min_size(
            caret.min,
            Vec2::new(width, caret.height()),
        ))
    }

    pub fn text_index_at(&self, text: NodeOf<TextNode>, pos: Pos2) -> Option<usize> {
        if let Some(index) = self.text_geometry(text).index_at(pos) {
            return Some(index);
        }
        let rect = self.node_rect(text)?;
        let node = self.arena.get_as::<TextNode>(text);
        let placed = node.placed.borrow();
        let placed = placed.as_ref()?;
        let origin = rect.min + (placed.origin - placed.rect.min);
        let index = placed.galley.cursor_at(origin, pos);
        Some(index.min(node.content.len()))
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
        Some((shift, placed.rich.clone()?))
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

fn plain_caret_rect(galley: &Galley, origin: Pos2, index: usize) -> Rect {
    let top = galley.cursor_pos(origin, index.min(galley.text().len()));
    Rect::from_min_size(top, Vec2::new(0.0, galley.line_height()))
}
