use std::any::Any;

use crate::base::list::Align;
use crate::color::{Color32, format_hex};
use crate::document::Document;
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::node::{Element, NodeId, NodeOf, Outlined, Rects};
use crate::painter::{Corners, Painter};
use crate::pixel_grid::PixelGrid;

pub use beui_types::layout::{Align2, Extent, Outline, Sides};

const TIP_RADIUS: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PointerSide {
    Top,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pointing {
    pub side: PointerSide,
    pub at: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FrameStyle {
    pub fill: Color32,
    pub radius: Corners,
    pub outline: Option<Outline>,
    pub pointer: f32,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            fill: Color32::TRANSPARENT,
            radius: Corners::all(0.0),
            outline: None,
            pointer: 0.0,
        }
    }
}

pub struct FrameNode {
    pub child: Option<NodeId>,
    pub width: Extent,
    pub height: Extent,
    pub aspect_ratio: Option<f32>,
    pub padding: Sides,
    pub align: Align2,
    pub style: FrameStyle,
    pub pointing: Option<Pointing>,
}

impl FrameNode {
    pub fn pointer_room(&self, rect: Rect) -> (f32, f32) {
        let radius = self.style.radius.largest();
        let reach = self.style.pointer + self.outline_width();
        (rect.left() + radius + reach, rect.right() - radius - reach)
    }

    fn outline_width(&self) -> f32 {
        self.style.outline.map_or(0.0, |outline| outline.width)
    }

    fn paint_pointer(&self, painter: &Painter, rect: Rect) {
        let Some(pointing) = self.pointing.filter(|_| self.style.pointer > 0.0) else {
            return;
        };
        let (lowest, highest) = self.pointer_room(rect);
        if lowest > highest {
            return;
        }
        let x = (rect.left() + pointing.at).clamp(lowest, highest);
        let width = self.outline_width();
        let half = (self.style.pointer + width) / std::f32::consts::SQRT_2;
        let around = rect.expand(self.style.pointer + 1.0);
        let (y, clip, tip) = match pointing.side {
            PointerSide::Top => (
                rect.top() + width,
                Rect::from_min_max(around.min, pos2(around.right(), rect.top() + width)),
                Corners::new(TIP_RADIUS, 0.0, 0.0, 0.0),
            ),
            PointerSide::Bottom => (
                rect.bottom() - width,
                Rect::from_min_max(pos2(around.left(), rect.bottom() - width), around.max),
                Corners::new(0.0, 0.0, TIP_RADIUS, 0.0),
            ),
        };
        let centre = pos2(x, y);
        let square = Rect::from_center_size(centre, Vec2::splat(2.0 * half));
        let turned = painter
            .with_clip_rect(clip)
            .rotated(centre, std::f32::consts::FRAC_PI_4);
        turned.rect_filled(square, tip, self.style.fill);
        if let Some(outline) = self.style.outline {
            turned.rect_stroke(square, tip, outline.width, outline.color);
        }
    }

    fn padding(&self, grid: PixelGrid) -> Sides {
        snapped(self.padding, grid)
    }

    fn size(&self, grid: PixelGrid, available: Vec2) -> Vec2 {
        let size = grid.snap_vec(vec2(
            self.width.offered(available.x),
            self.height.offered(available.y),
        ));
        match self.aspect_ratio {
            Some(ratio) if ratio > 0.0 => grid.snap_vec(contain(size, ratio)),
            _ => size,
        }
    }

    fn box_rect(&self, grid: PixelGrid, rect: Rect) -> Rect {
        let size = self.size(grid, rect.size());
        match self.aspect_ratio {
            Some(ratio) if ratio > 0.0 => centered(grid, rect, size),
            _ => Rect::from_min_size(rect.min, size),
        }
    }

    fn content_rect(
        &self,
        doc: &mut Document,
        painter: &Painter,
        child: NodeId,
        inner: Rect,
    ) -> Rect {
        let grid = doc.pixel_grid();
        if self.align == Align2::STRETCH {
            return inner;
        }
        let measured = crate::layout::measure(doc, painter, child, inner.size());
        let place = |align: Align, room: f32, length: f32| -> (f32, f32) {
            let length = length.min(room);
            match align {
                Align::Stretch => (0.0, room),
                Align::Start | Align::Baseline => (0.0, length),
                Align::Center => (grid.snap((room - length) / 2.0), length),
                Align::End => (room - length, length),
            }
        };
        let (x, width) = place(self.align.horizontal, inner.width(), measured.x);
        let (y, height) = place(self.align.vertical, inner.height(), measured.y);
        Rect::from_min_size(inner.min + vec2(x, y), vec2(width, height))
    }
}

fn contain(size: Vec2, ratio: f32) -> Vec2 {
    let width = size.x.min(size.y * ratio);
    vec2(width, width / ratio)
}

fn cover(size: Vec2, ratio: f32) -> Vec2 {
    let width = size.x.max(size.y * ratio);
    vec2(width, width / ratio)
}

fn centered(grid: PixelGrid, rect: Rect, size: Vec2) -> Rect {
    let offset = grid.snap_vec((rect.size() - size) * 0.5);
    Rect::from_min_size(rect.min + offset, size)
}

impl Default for FrameNode {
    fn default() -> Self {
        Self {
            child: None,
            width: Extent::default(),
            height: Extent::default(),
            aspect_ratio: None,
            padding: Sides::default(),
            align: Align2::STRETCH,
            style: FrameStyle::default(),
            pointing: None,
        }
    }
}

impl FrameNode {
    fn child_available(&self, available: Vec2, padding: Vec2) -> Vec2 {
        let constrained = vec2(
            self.width.offered_to_content(available.x),
            self.height.offered_to_content(available.y),
        );
        (constrained - padding).max(Vec2::ZERO)
    }
}

impl Element for FrameNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        if self.width.scales_with(available.x) || self.height.scales_with(available.y) {
            doc.note_offer_dependent();
        }
        let grid = doc.pixel_grid();
        let padding = self.padding(grid).amount();
        let inner = match self.child {
            Some(child) => {
                let available = self.child_available(available, padding);
                crate::layout::measure(doc, painter, child, available)
            }
            None => Vec2::ZERO,
        };
        let padded = (inner + padding).max(Vec2::ZERO);
        let size = grid.snap_vec(vec2(
            self.width.within(available.x, padded.x),
            self.height.within(available.y, padded.y),
        ));
        match self.aspect_ratio {
            Some(ratio) if ratio > 0.0 => grid.snap_vec(cover(size, ratio)),
            _ => size,
        }
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        let width = self.width;
        let padding = self.padding(doc.pixel_grid()).amount().x;
        let inner = self
            .child
            .map_or(0.0, |child| crate::layout::min_width(doc, painter, child));
        let length = (inner + padding).max(0.0);
        let length = match (width.exact, width.fraction) {
            (Some(exact), _) => length.min(exact),
            (None, Some(_)) => 0.0,
            (None, None) => length,
        };
        let length = width.max.map_or(length, |max| length.min(max));
        width.min.map_or(length, |min| length.max(min))
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        let child = self.child?;
        let grid = doc.pixel_grid();
        let padding = self.padding(grid);
        let offered = self.child_available(available, padding.amount());
        let baseline = crate::layout::baseline(doc, painter, child, offered)?;
        let offset = match self.align.vertical {
            Align::Center | Align::End => {
                let size = crate::layout::measure(doc, painter, child, offered);
                let room = self.measure(doc, painter, available).y - padding.amount().y;
                let spare = (room - size.y).max(0.0);
                match self.align.vertical {
                    Align::Center => grid.snap(spare / 2.0),
                    _ => spare,
                }
            }
            _ => 0.0,
        };
        Some(padding.top + offset + baseline)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        if let Some(child) = self.child {
            let grid = doc.pixel_grid();
            let padding = self.padding(grid);
            let outer = self.box_rect(grid, rect);
            let inner = Rect::from_min_max(
                outer.min + padding.start(),
                outer.max - vec2(padding.right, padding.bottom),
            );
            let content = self.content_rect(doc, painter, child, inner);
            crate::layout::layout(doc, painter, child, content, out);
        }
    }

    fn relayout_boundary(&self) -> bool {
        self.width.exact.is_some() && self.height.exact.is_some()
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        let rect = self.box_rect(doc.pixel_grid(), rect);
        if self.style.fill.to_array()[3] > 0 {
            painter.rect_filled(rect, self.style.radius, self.style.fill);
        }
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
        if let Some(outline) = self.style.outline {
            painter.rect_stroke(
                rect.expand(outline.offset),
                self.style.radius,
                outline.width,
                outline.color,
            );
        }
        self.paint_pointer(painter, rect);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn for_each_child(&self, visit: &mut dyn FnMut(NodeId)) {
        if let Some(child) = self.child {
            visit(child);
        }
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
    }

    fn pointer_children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn passes_scroll_anchor(&self) -> bool {
        true
    }

    fn kind(&self) -> &'static str {
        "frame"
    }

    fn outline(&self, grid: PixelGrid, rect: Rect) -> Option<Outlined> {
        let outline = self.style.outline.filter(|outline| {
            outline.offset > 0.0 && outline.width > 0.0 && outline.color.alpha() > 0
        })?;
        let inner = self.box_rect(grid, rect);
        Some(Outlined {
            inner,
            outer: inner.expand(outline.offset),
        })
    }

    fn detail(&self) -> Option<String> {
        let [red, green, blue, alpha] = self.style.fill.to_array();
        if alpha == 0 {
            return None;
        }
        Some(format!("#{red:02x}{green:02x}{blue:02x}"))
    }

    fn properties(&self) -> Vec<(&'static str, String)> {
        let padding = self.padding;
        let radius = self.style.radius;
        vec![
            ("width", extent_label(self.width)),
            ("height", extent_label(self.height)),
            (
                "aspect ratio",
                self.aspect_ratio
                    .map_or_else(|| "none".to_owned(), |ratio| ratio.to_string()),
            ),
            (
                "padding",
                format!(
                    "{} {} {} {}",
                    padding.left, padding.top, padding.right, padding.bottom
                ),
            ),
            (
                "align",
                format!("{:?} {:?}", self.align.horizontal, self.align.vertical),
            ),
            ("fill", format_hex(self.style.fill, true)),
            (
                "outline",
                self.style.outline.map_or_else(
                    || "none".to_owned(),
                    |outline| {
                        format!(
                            "{} {} offset {}",
                            outline.width,
                            format_hex(outline.color, true),
                            outline.offset
                        )
                    },
                ),
            ),
            (
                "radius",
                format!(
                    "{} {} {} {}",
                    radius.top_left, radius.top_right, radius.bottom_right, radius.bottom_left
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

fn extent_label(extent: Extent) -> String {
    let parts: Vec<String> = [
        extent.exact.map(|exact| exact.to_string()),
        extent
            .fraction
            .map(|fraction| format!("{}%", fraction * 100.0)),
        extent.min.map(|min| format!("min {min}")),
        extent.max.map(|max| format!("max {max}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    match parts.is_empty() {
        true => "intrinsic".to_owned(),
        false => parts.join(" "),
    }
}

impl Document {
    pub fn create_frame(&mut self) -> NodeOf<FrameNode> {
        self.arena.insert(FrameNode::default())
    }

    pub fn set_frame_child(&mut self, frame: NodeOf<FrameNode>, child: NodeId) {
        if self.arena.get_as::<FrameNode>(frame).child != Some(child) {
            self.arena.get_mut_as::<FrameNode>(frame).child = Some(child);
        }
    }

    pub fn set_frame_width(&mut self, frame: NodeOf<FrameNode>, width: Extent) {
        if self.arena.get_as::<FrameNode>(frame).width != width {
            self.arena.get_mut_as::<FrameNode>(frame).width = width;
        }
    }

    pub fn set_frame_height(&mut self, frame: NodeOf<FrameNode>, height: Extent) {
        if self.arena.get_as::<FrameNode>(frame).height != height {
            self.arena.get_mut_as::<FrameNode>(frame).height = height;
        }
    }

    pub fn set_frame_aspect_ratio(&mut self, frame: NodeOf<FrameNode>, ratio: Option<f32>) {
        if self.arena.get_as::<FrameNode>(frame).aspect_ratio != ratio {
            self.arena.get_mut_as::<FrameNode>(frame).aspect_ratio = ratio;
        }
    }

    pub fn set_frame_padding(&mut self, frame: NodeOf<FrameNode>, padding: Sides) {
        if self.arena.get_as::<FrameNode>(frame).padding != padding {
            self.arena.get_mut_as::<FrameNode>(frame).padding = padding;
        }
    }

    pub fn set_frame_align(&mut self, frame: NodeOf<FrameNode>, align: Align2) {
        if self.arena.get_as::<FrameNode>(frame).align != align {
            self.arena.get_mut_as::<FrameNode>(frame).align = align;
        }
    }

    pub fn set_frame_style(&mut self, frame: NodeOf<FrameNode>, style: FrameStyle) {
        if self.arena.get_as::<FrameNode>(frame).style != style {
            self.arena.paint_mut_as::<FrameNode>(frame).style = style;
        }
    }

    pub fn set_frame_pointing(&mut self, frame: NodeOf<FrameNode>, pointing: Option<Pointing>) {
        if self.arena.get_as::<FrameNode>(frame).pointing != pointing {
            self.arena.paint_mut_as::<FrameNode>(frame).pointing = pointing;
        }
    }

    pub fn set_frame_color(&mut self, frame: NodeOf<FrameNode>, color: Color32) {
        if self.arena.get_as::<FrameNode>(frame).style.fill != color {
            self.arena.paint_mut_as::<FrameNode>(frame).style.fill = color;
        }
    }
}

fn snapped(sides: Sides, grid: PixelGrid) -> Sides {
    Sides {
        left: grid.snap(sides.left),
        top: grid.snap(sides.top),
        right: grid.snap(sides.right),
        bottom: grid.snap(sides.bottom),
    }
}
