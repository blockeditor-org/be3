use std::any::Any;

use crate::base::list::Align;
use crate::color::Color32;
use crate::document::Document;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::painter::{Corners, Painter};
use crate::pixel_grid::PixelGrid;

#[derive(Clone, Copy, PartialEq)]
pub struct FrameStyle {
    pub fill: Color32,
    pub outline: Color32,
    pub outline_width: f32,
    pub radius: Corners,
    pub outline_offset: f32,
    pub outline_visible: bool,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            fill: Color32::TRANSPARENT,
            outline: Color32::TRANSPARENT,
            outline_width: 0.0,
            radius: Corners::all(0.0),
            outline_offset: 0.0,
            outline_visible: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Sides {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Sides {
    pub fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    fn snapped(self, grid: PixelGrid) -> Self {
        Self {
            left: grid.snap(self.left),
            top: grid.snap(self.top),
            right: grid.snap(self.right),
            bottom: grid.snap(self.bottom),
        }
    }

    fn start(self) -> Vec2 {
        vec2(self.left, self.top)
    }

    fn amount(self) -> Vec2 {
        vec2(self.left + self.right, self.top + self.bottom)
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Extent {
    pub exact: Option<f32>,
    pub fraction: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

impl Extent {
    fn within(self, available: f32, intrinsic: f32) -> f32 {
        let fraction = self
            .fraction
            .filter(|_| available.is_finite())
            .map(|fraction| fraction * available);
        let length = match (self.exact.or(fraction), self.max) {
            (Some(length), _) => length,
            (None, Some(_)) => available,
            (None, None) => intrinsic,
        };
        let length = self.max.map_or(length, |max| length.min(max));
        self.min.map_or(length, |min| length.max(min))
    }

    fn offered(self, available: f32) -> f32 {
        let length = self.exact.unwrap_or(available);
        self.max.map_or(length, |max| length.min(max))
    }

    fn offered_to_content(self, available: f32) -> f32 {
        let fraction = self
            .fraction
            .filter(|_| available.is_finite())
            .map(|fraction| fraction * available);
        let length = self.exact.or(fraction).unwrap_or(available);
        self.max.map_or(length, |max| length.min(max))
    }
}

pub struct FrameNode {
    pub child: Option<NodeId>,
    pub width: Extent,
    pub height: Extent,
    pub aspect_ratio: Option<f32>,
    pub padding: Sides,
    pub align_horizontal: Align,
    pub align_vertical: Align,
    pub style: FrameStyle,
    pub visible: bool,
}

impl FrameNode {
    fn padding(&self, grid: PixelGrid) -> Sides {
        self.padding.snapped(grid)
    }

    fn shown(&self) -> Option<NodeId> {
        self.child.filter(|_| self.visible)
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
        if self.align_horizontal == Align::Stretch && self.align_vertical == Align::Stretch {
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
        let (x, width) = place(self.align_horizontal, inner.width(), measured.x);
        let (y, height) = place(self.align_vertical, inner.height(), measured.y);
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
            align_horizontal: Align::Stretch,
            align_vertical: Align::Stretch,
            style: FrameStyle::default(),
            visible: true,
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
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        if !self.visible {
            return Vec2::ZERO;
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
        let padded = inner + padding;
        let size = grid.snap_vec(vec2(
            self.width.within(available.x, padded.x),
            self.height.within(available.y, padded.y),
        ));
        match self.aspect_ratio {
            Some(ratio) if ratio > 0.0 => grid.snap_vec(cover(size, ratio)),
            _ => size,
        }
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        let child = self.shown()?;
        let grid = doc.pixel_grid();
        let padding = self.padding(grid);
        let offered = self.child_available(available, padding.amount());
        let baseline = crate::layout::baseline(doc, painter, child, offered)?;
        let offset = match self.align_vertical {
            Align::Center | Align::End => {
                let size = crate::layout::measure(doc, painter, child, offered);
                let room = self.measure(doc, painter, available).y - padding.amount().y;
                let spare = (room - size.y).max(0.0);
                match self.align_vertical {
                    Align::Center => grid.snap(spare / 2.0),
                    _ => spare,
                }
            }
            _ => 0.0,
        };
        Some(padding.top + offset + baseline)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        if let Some(child) = self.shown() {
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
        if !self.visible {
            return;
        }
        let rect = self.box_rect(doc.pixel_grid(), rect);
        if self.style.fill.to_array()[3] > 0 {
            painter.rect_filled(rect, self.style.radius, self.style.fill);
        }
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
        if self.style.outline_visible {
            painter.rect_stroke(
                rect.expand(self.style.outline_offset),
                self.style.radius,
                self.style.outline_width,
                self.style.outline,
            );
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
        children.extend(self.shown());
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "frame"
    }

    fn detail(&self) -> Option<String> {
        if !self.visible {
            return Some("hidden".to_owned());
        }
        let [red, green, blue, alpha] = self.style.fill.to_array();
        if alpha == 0 {
            return None;
        }
        Some(format!("#{red:02x}{green:02x}{blue:02x}"))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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

    pub fn set_frame_width(&mut self, frame: NodeOf<FrameNode>, width: Option<f32>) {
        self.update_frame_width(frame, |extent| extent.exact = width);
    }

    pub fn set_frame_min_width(&mut self, frame: NodeOf<FrameNode>, min_width: Option<f32>) {
        self.update_frame_width(frame, |extent| extent.min = min_width);
    }

    pub fn set_frame_max_width(&mut self, frame: NodeOf<FrameNode>, max_width: Option<f32>) {
        self.update_frame_width(frame, |extent| extent.max = max_width);
    }

    pub fn set_frame_width_fraction(&mut self, frame: NodeOf<FrameNode>, fraction: Option<f32>) {
        self.update_frame_width(frame, |extent| extent.fraction = fraction);
    }

    pub fn set_frame_height(&mut self, frame: NodeOf<FrameNode>, height: Option<f32>) {
        self.update_frame_height(frame, |extent| extent.exact = height);
    }

    pub fn set_frame_min_height(&mut self, frame: NodeOf<FrameNode>, min_height: Option<f32>) {
        self.update_frame_height(frame, |extent| extent.min = min_height);
    }

    pub fn set_frame_max_height(&mut self, frame: NodeOf<FrameNode>, max_height: Option<f32>) {
        self.update_frame_height(frame, |extent| extent.max = max_height);
    }

    pub fn set_frame_height_fraction(&mut self, frame: NodeOf<FrameNode>, fraction: Option<f32>) {
        self.update_frame_height(frame, |extent| extent.fraction = fraction);
    }

    fn update_frame_width(&mut self, frame: NodeOf<FrameNode>, change: impl FnOnce(&mut Extent)) {
        let mut width = self.arena.get_as::<FrameNode>(frame).width;
        change(&mut width);
        if self.arena.get_as::<FrameNode>(frame).width != width {
            self.arena.get_mut_as::<FrameNode>(frame).width = width;
        }
    }

    fn update_frame_height(&mut self, frame: NodeOf<FrameNode>, change: impl FnOnce(&mut Extent)) {
        let mut height = self.arena.get_as::<FrameNode>(frame).height;
        change(&mut height);
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

    pub fn set_frame_content_align(
        &mut self,
        frame: NodeOf<FrameNode>,
        horizontal: Align,
        vertical: Align,
    ) {
        let node = self.arena.get_as::<FrameNode>(frame);
        if node.align_horizontal == horizontal && node.align_vertical == vertical {
            return;
        }
        let node = self.arena.get_mut_as::<FrameNode>(frame);
        node.align_horizontal = horizontal;
        node.align_vertical = vertical;
    }

    pub fn set_frame_style(&mut self, frame: NodeOf<FrameNode>, style: FrameStyle) {
        if self.arena.get_as::<FrameNode>(frame).style != style {
            self.arena.paint_mut_as::<FrameNode>(frame).style = style;
        }
    }

    pub fn set_frame_color(&mut self, frame: NodeOf<FrameNode>, color: Color32) {
        if self.arena.get_as::<FrameNode>(frame).style.fill != color {
            self.arena.paint_mut_as::<FrameNode>(frame).style.fill = color;
        }
    }

    pub fn is_visible(&self, frame: NodeOf<FrameNode>) -> bool {
        self.arena.get_as::<FrameNode>(frame).visible
    }

    pub fn set_visible(&mut self, frame: NodeOf<FrameNode>, visible: bool) {
        if self.arena.get_as::<FrameNode>(frame).visible != visible {
            self.arena.get_mut_as::<FrameNode>(frame).visible = visible;
        }
    }
}
