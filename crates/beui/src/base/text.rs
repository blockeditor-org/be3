use std::any::Any;
use std::cell::RefCell;

use crate::color::Color32;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::painter::Painter;
use crate::pixel_grid::PixelGrid;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, NodeMap};
use crate::reactive::{Prop, create_effect, with_document};

use beui_macros::component;

const UNDERLINE_OFFSET: f32 = 0.1;
const UNDERLINE_THICKNESS: f32 = 0.07;
const UNDERLINE_MINIMUM_THICKNESS: f32 = 1.0;
const DEFAULT_FONT_SIZE: f32 = 14.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

#[derive(Clone)]
struct Placed {
    rect: Rect,
    galley: Galley,
    origin: Pos2,
}

pub(crate) struct TextNode {
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
    placed: RefCell<Option<Placed>>,
}

impl TextNode {
    pub(crate) fn accessible_text(&self) -> Option<&str> {
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
        let galley = self.galley(painter, &self.content, rect.width());
        let origin = self.origin(painter.pixel_grid(), galley.size(), rect);
        let placed = Placed {
            rect,
            galley,
            origin,
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
    fn measure(&self, _doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.galley(painter, &self.content, available.x).size()
    }

    fn layout(
        &mut self,
        _doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        _out: &mut NodeMap<Rect>,
    ) {
        self.place(painter, rect);
    }

    fn paint(&self, _doc: &Document, painter: &Painter, _rects: &NodeMap<Rect>, rect: Rect) {
        let clipped = painter.with_clip_rect(if self.clip { rect } else { Rect::EVERYTHING });
        let placed = self.placed(&clipped, rect);
        clipped.galley(placed.origin, placed.galley.clone(), self.color);
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
        _children: &mut Vec<NodeId>,
    ) {
    }

    fn children(&self) -> Vec<NodeId> {
        Vec::new()
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
    pub(crate) fn create_text(
        &mut self,
        content: impl Into<String>,
        font_size: f32,
        color: Color32,
    ) -> NodeId {
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
            placed: RefCell::new(None),
        })
    }

    pub(crate) fn set_text(&mut self, id: NodeId, content: impl Into<String>) {
        let value = content.into();
        if self.arena.get_as::<TextNode>(id).content != value {
            self.arena.get_mut_as::<TextNode>(id).content = value;
        }
    }

    pub fn text(&self, id: NodeId) -> &str {
        &self.arena.get_as::<TextNode>(id).content
    }

    pub(crate) fn set_text_horizontal_align(&mut self, text: NodeId, horizontal: TextAlign) {
        if self.arena.get_as::<TextNode>(text).horizontal != horizontal {
            self.arena.get_mut_as::<TextNode>(text).horizontal = horizontal;
        }
    }

    pub(crate) fn set_text_vertical_align(&mut self, text: NodeId, vertical: TextAlign) {
        if self.arena.get_as::<TextNode>(text).vertical != vertical {
            self.arena.get_mut_as::<TextNode>(text).vertical = vertical;
        }
    }

    pub(crate) fn set_text_wrap(&mut self, text: NodeId, wrap: bool) {
        if self.arena.get_as::<TextNode>(text).wrap != wrap {
            self.arena.get_mut_as::<TextNode>(text).wrap = wrap;
        }
    }

    pub(crate) fn set_text_font_size(&mut self, text: NodeId, font_size: f32) {
        if self.arena.get_as::<TextNode>(text).font_size != font_size {
            self.arena.get_mut_as::<TextNode>(text).font_size = font_size;
        }
    }

    pub(crate) fn set_text_line_height(&mut self, text: NodeId, line_height: Option<f32>) {
        if self.arena.get_as::<TextNode>(text).line_height != line_height {
            self.arena.get_mut_as::<TextNode>(text).line_height = line_height;
        }
    }

    pub(crate) fn set_text_monospace(&mut self, text: NodeId, monospace: bool) {
        if self.arena.get_as::<TextNode>(text).monospace != monospace {
            self.arena.get_mut_as::<TextNode>(text).monospace = monospace;
        }
    }

    pub(crate) fn set_text_bold(&mut self, text: NodeId, bold: bool) {
        if self.arena.get_as::<TextNode>(text).bold != bold {
            self.arena.get_mut_as::<TextNode>(text).bold = bold;
        }
    }

    pub(crate) fn set_text_italic(&mut self, text: NodeId, italic: bool) {
        if self.arena.get_as::<TextNode>(text).italic != italic {
            self.arena.get_mut_as::<TextNode>(text).italic = italic;
        }
    }

    pub(crate) fn set_text_icon(&mut self, text: NodeId, icon: bool) {
        if self.arena.get_as::<TextNode>(text).icon != icon {
            self.arena.get_mut_as::<TextNode>(text).icon = icon;
        }
    }

    pub(crate) fn set_text_color(&mut self, text: NodeId, color: Color32) {
        if self.arena.get_as::<TextNode>(text).color != color {
            self.arena.get_mut_as::<TextNode>(text).color = color;
        }
    }

    pub(crate) fn set_text_clip(&mut self, text: NodeId, clip: bool) {
        if self.arena.get_as::<TextNode>(text).clip != clip {
            self.arena.get_mut_as::<TextNode>(text).clip = clip;
        }
    }

    pub(crate) fn set_text_underline(&mut self, text: NodeId, underline: bool) {
        if self.arena.get_as::<TextNode>(text).underline != underline {
            self.arena.get_mut_as::<TextNode>(text).underline = underline;
        }
    }
}

#[component]
pub fn Text(
    string: Prop<String>,
    #[prop(default = DEFAULT_FONT_SIZE)] font_size: Prop<f32>,
    line_height: Option<Prop<f32>>,
    #[prop(default = Color32::WHITE)] color: Prop<Color32>,
    align: Option<Prop<TextAlign>>,
    vertical_align: Option<Prop<TextAlign>>,
    #[prop(default = false)] wrap: Prop<bool>,
    #[prop(default = false)] monospace: Prop<bool>,
    #[prop(default = false)] bold: Prop<bool>,
    #[prop(default = false)] italic: Prop<bool>,
    #[prop(default = false)] icon: Prop<bool>,
    #[prop(default = false)] clip: Prop<bool>,
    #[prop(default = false)] underline: Prop<bool>,
) -> NodeId {
    let vertical_default = match align {
        Some(_) => TextAlign::Center,
        None => TextAlign::Start,
    };
    let align = align.unwrap_or(Prop::Static(TextAlign::Start));
    let vertical_align = vertical_align.unwrap_or(Prop::Static(vertical_default));
    let node = with_document(|document| {
        document.create_text(String::new(), DEFAULT_FONT_SIZE, Color32::WHITE)
    });
    create_effect(move || {
        with_document(|document| document.set_text_horizontal_align(node, align.get()))
    });
    create_effect(move || {
        with_document(|document| document.set_text_vertical_align(node, vertical_align.get()))
    });
    create_effect(move || with_document(|document| document.set_text_wrap(node, wrap.get())));
    create_effect(move || {
        with_document(|document| document.set_text_monospace(node, monospace.get()))
    });
    create_effect(move || with_document(|document| document.set_text_bold(node, bold.get())));
    create_effect(move || with_document(|document| document.set_text_italic(node, italic.get())));
    create_effect(move || with_document(|document| document.set_text_icon(node, icon.get())));
    create_effect(move || with_document(|document| document.set_text_clip(node, clip.get())));
    create_effect(move || {
        with_document(|document| document.set_text_underline(node, underline.get()))
    });
    create_effect(move || with_document(|document| document.set_text(node, string.get())));
    create_effect(move || {
        with_document(|document| document.set_text_font_size(node, font_size.get()))
    });
    if let Some(line_height) = line_height {
        create_effect(move || {
            with_document(|document| document.set_text_line_height(node, Some(line_height.get())))
        });
    }
    create_effect(move || with_document(|document| document.set_text_color(node, color.get())));
    node
}
