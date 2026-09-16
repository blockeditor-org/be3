use std::any::Any;
use std::collections::HashMap;

use beui_macros::component;

use crate::color::Color32;
use crate::document::Document;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, InteractInput, NodeId};
use crate::painter::Painter;
use crate::reactive::{Child, Prop, create_effect, with_document};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FrameStyle {
    pub(crate) fill: Color32,
    pub(crate) outline: Color32,
    pub(crate) outline_width: f32,
    pub(crate) radius: u8,
    pub(crate) outline_offset: f32,
    pub(crate) outline_visible: bool,
}

impl Default for FrameStyle {
    fn default() -> Self {
        Self {
            fill: Color32::TRANSPARENT,
            outline: Color32::TRANSPARENT,
            outline_width: 0.0,
            radius: 0,
            outline_offset: 0.0,
            outline_visible: false,
        }
    }
}

pub(crate) struct FrameNode {
    pub(crate) child: Option<NodeId>,
    pub(crate) width: Option<f32>,
    pub(crate) height: Option<f32>,
    pub(crate) aspect_ratio: Option<f32>,
    pub(crate) padding_horizontal: f32,
    pub(crate) padding_vertical: f32,
    pub(crate) style: FrameStyle,
    pub(crate) visible: bool,
}

impl FrameNode {
    fn amount(&self) -> Vec2 {
        vec2(self.padding_horizontal * 2.0, self.padding_vertical * 2.0)
    }

    fn shown(&self) -> Option<NodeId> {
        self.child.filter(|_| self.visible)
    }

    fn size(&self, available: Vec2) -> Vec2 {
        let size = vec2(
            self.width.unwrap_or(available.x),
            self.height.unwrap_or(available.y),
        );
        match self.aspect_ratio {
            Some(ratio) if ratio > 0.0 => contain(size, ratio),
            _ => size,
        }
    }

    fn box_rect(&self, rect: Rect) -> Rect {
        let size = self.size(rect.size());
        match self.aspect_ratio {
            Some(ratio) if ratio > 0.0 => centered(rect, size),
            _ => Rect::from_min_size(rect.min, size),
        }
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

fn centered(rect: Rect, size: Vec2) -> Rect {
    let offset = (rect.size() - size) * 0.5;
    Rect::from_min_size(rect.min + offset, size)
}

impl Default for FrameNode {
    fn default() -> Self {
        Self {
            child: None,
            width: None,
            height: None,
            aspect_ratio: None,
            padding_horizontal: 0.0,
            padding_vertical: 0.0,
            style: FrameStyle::default(),
            visible: true,
        }
    }
}

impl Element for FrameNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        if !self.visible {
            return Vec2::ZERO;
        }
        let constrained = vec2(
            self.width.unwrap_or(available.x),
            self.height.unwrap_or(available.y),
        );
        let inner = match self.child {
            Some(child) => {
                let available = (constrained - self.amount()).max(Vec2::ZERO);
                crate::layout::measure(doc, painter, child, available)
            }
            None => Vec2::ZERO,
        };
        let padded = inner + self.amount();
        let size = vec2(
            self.width.unwrap_or(padded.x),
            self.height.unwrap_or(padded.y),
        );
        match self.aspect_ratio {
            Some(ratio) if ratio > 0.0 => cover(size, ratio),
            _ => size,
        }
    }

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut HashMap<NodeId, Rect>,
    ) {
        if let Some(child) = self.shown() {
            let outer = self.box_rect(rect);
            let inner = Rect::from_min_max(
                outer.min + vec2(self.padding_horizontal, self.padding_vertical),
                outer.max - vec2(self.padding_horizontal, self.padding_vertical),
            );
            crate::layout::layout(doc, painter, child, inner, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &HashMap<NodeId, Rect>, rect: Rect) {
        if !self.visible {
            return;
        }
        let rect = self.box_rect(rect);
        if self.style.fill.to_array()[3] > 0 {
            painter.rect_filled(rect, f32::from(self.style.radius), self.style.fill);
        }
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
        if self.style.outline_visible {
            painter.rect_stroke(
                rect.expand(self.style.outline_offset),
                f32::from(self.style.radius),
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
    ) -> Vec<NodeId> {
        self.shown().into_iter().collect()
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
    pub(crate) fn create_frame(&mut self) -> NodeId {
        self.arena.insert(FrameNode::default())
    }

    pub(crate) fn set_frame_child(&mut self, frame: NodeId, child: NodeId) {
        if self.arena.get_as::<FrameNode>(frame).child != Some(child) {
            self.arena.get_mut_as::<FrameNode>(frame).child = Some(child);
        }
    }

    pub(crate) fn set_frame_width(&mut self, frame: NodeId, width: Option<f32>) {
        if self.arena.get_as::<FrameNode>(frame).width != width {
            self.arena.get_mut_as::<FrameNode>(frame).width = width;
        }
    }

    pub(crate) fn set_frame_height(&mut self, frame: NodeId, height: Option<f32>) {
        if self.arena.get_as::<FrameNode>(frame).height != height {
            self.arena.get_mut_as::<FrameNode>(frame).height = height;
        }
    }

    pub(crate) fn set_frame_aspect_ratio(&mut self, frame: NodeId, ratio: Option<f32>) {
        if self.arena.get_as::<FrameNode>(frame).aspect_ratio != ratio {
            self.arena.get_mut_as::<FrameNode>(frame).aspect_ratio = ratio;
        }
    }

    pub(crate) fn set_frame_padding(&mut self, frame: NodeId, horizontal: f32, vertical: f32) {
        let node = self.arena.get_as::<FrameNode>(frame);
        if node.padding_horizontal == horizontal && node.padding_vertical == vertical {
            return;
        }
        let node = self.arena.get_mut_as::<FrameNode>(frame);
        node.padding_horizontal = horizontal;
        node.padding_vertical = vertical;
    }

    pub(crate) fn set_frame_style(&mut self, frame: NodeId, style: FrameStyle) {
        if self.arena.get_as::<FrameNode>(frame).style != style {
            self.arena.get_mut_as::<FrameNode>(frame).style = style;
        }
    }

    #[cfg(test)]
    pub(crate) fn set_frame_color(&mut self, frame: NodeId, color: Color32) {
        if self.arena.get_as::<FrameNode>(frame).style.fill != color {
            self.arena.get_mut_as::<FrameNode>(frame).style.fill = color;
        }
    }

    pub(crate) fn is_hidden_frame(&self, id: NodeId) -> bool {
        self.arena
            .get(id)
            .as_any()
            .downcast_ref::<FrameNode>()
            .is_some_and(|frame| !frame.visible)
    }

    pub fn is_visible(&self, frame: NodeId) -> bool {
        self.arena.get_as::<FrameNode>(frame).visible
    }

    pub(crate) fn set_visible(&mut self, frame: NodeId, visible: bool) {
        if self.arena.get_as::<FrameNode>(frame).visible != visible {
            self.arena.get_mut_as::<FrameNode>(frame).visible = visible;
        }
    }
}

#[component]
pub fn Frame(
    width: Option<Prop<f32>>,
    height: Option<Prop<f32>>,
    aspect_ratio: Option<Prop<f32>>,
    #[prop(default = 0.0)] padding_horizontal: Prop<f32>,
    #[prop(default = 0.0)] padding_vertical: Prop<f32>,
    #[prop(default = Color32::TRANSPARENT)] color: Prop<Color32>,
    #[prop(default = Color32::TRANSPARENT)] outline: Prop<Color32>,
    #[prop(default = 0.0)] outline_width: Prop<f32>,
    #[prop(default = 0)] radius: Prop<u8>,
    #[prop(default = 0.0)] outline_offset: Prop<f32>,
    #[prop(default = false)] outline_visible: Prop<bool>,
    #[prop(default = true)] visible: Prop<bool>,
    children: Option<Child>,
) -> NodeId {
    let frame = with_document(|document| {
        let frame = document.create_frame();
        if let Some(child) = children {
            document.set_frame_child(frame, child);
        }
        frame
    });
    if let Some(width) = width {
        create_effect(move || {
            with_document(|document| document.set_frame_width(frame, Some(width.get())))
        });
    }
    if let Some(height) = height {
        create_effect(move || {
            with_document(|document| document.set_frame_height(frame, Some(height.get())))
        });
    }
    if let Some(aspect_ratio) = aspect_ratio {
        create_effect(move || {
            with_document(|document| {
                document.set_frame_aspect_ratio(frame, Some(aspect_ratio.get()));
            })
        });
    }
    create_effect(move || {
        with_document(|document| {
            document.set_frame_padding(frame, padding_horizontal.get(), padding_vertical.get());
        })
    });
    create_effect(move || {
        with_document(|document| {
            document.set_frame_style(
                frame,
                FrameStyle {
                    fill: color.get(),
                    outline: outline.get(),
                    outline_width: outline_width.get(),
                    radius: radius.get(),
                    outline_offset: outline_offset.get(),
                    outline_visible: outline_visible.get(),
                },
            );
        })
    });
    create_effect(move || with_document(|document| document.set_visible(frame, visible.get())));
    frame
}
