use std::any::Any;

use crate::color::Color32;
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::image::{Image, ImageFit, Thumbhash};
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub struct PictureNode {
    image: Option<Image>,
    thumbhash: Option<Thumbhash>,
    placeholder: Option<Image>,
    source: Option<Rect>,
    fit: ImageFit,
    tint: Color32,
    radius: f32,
    smooth: bool,
}

const WHOLE: Rect = Rect {
    min: crate::geometry::Pos2 { x: 0.0, y: 0.0 },
    max: crate::geometry::Pos2 { x: 1.0, y: 1.0 },
};

impl PictureNode {
    fn shown(&self) -> Option<(&Image, Vec2, bool)> {
        match (&self.image, &self.placeholder, &self.thumbhash) {
            (Some(image), _, _) => Some((image, self.cropped(image.size()), self.smooth)),
            (None, Some(placeholder), Some(thumbhash)) => {
                Some((placeholder, self.cropped(thumbhash.size()), true))
            }
            _ => None,
        }
    }

    fn cropped(&self, size: Vec2) -> Vec2 {
        match self.source {
            Some(source) => Vec2::new(size.x * source.width(), size.y * source.height()),
            None => size,
        }
    }
}

impl Element for PictureNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, available: Vec2) -> Vec2 {
        let Some((_, size, _)) = self.shown() else {
            return Vec2::ZERO;
        };
        if !available.x.is_finite() || available.x <= 0.0 {
            return size;
        }
        match size.x > available.x {
            true => Vec2::new(available.x, size.y * available.x / size.x),
            false => size,
        }
    }

    fn layout(&mut self, _doc: &mut Document, _painter: &Painter, _rect: Rect, _out: &Rects) {}

    fn paint(&self, _doc: &Document, painter: &Painter, _rects: &Rects, rect: Rect) {
        let Some((image, size, smooth)) = self.shown() else {
            return;
        };
        painter.image(
            self.fit.place(rect, size),
            self.source.unwrap_or(WHOLE),
            image,
            self.tint,
            self.radius,
            smooth,
        );
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
        "picture"
    }

    fn detail(&self) -> Option<String> {
        match (&self.image, &self.placeholder) {
            (Some(image), _) => Some(format!("{}x{}", image.width(), image.height())),
            (None, Some(_)) => self
                .thumbhash
                .as_ref()
                .map(|thumbhash| format!("thumbhash {}x{}", thumbhash.width, thumbhash.height)),
            (None, None) => None,
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_picture(&mut self) -> NodeOf<PictureNode> {
        self.arena.insert(PictureNode {
            image: None,
            thumbhash: None,
            placeholder: None,
            source: None,
            fit: ImageFit::Contain,
            tint: Color32::WHITE,
            radius: 0.0,
            smooth: true,
        })
    }

    pub fn set_picture_image(&mut self, picture: NodeOf<PictureNode>, image: Option<Image>) {
        if self.arena.get_as::<PictureNode>(picture).image != image {
            self.arena.get_mut_as::<PictureNode>(picture).image = image;
        }
    }

    pub fn set_picture_thumbhash(
        &mut self,
        picture: NodeOf<PictureNode>,
        thumbhash: Option<Thumbhash>,
    ) {
        if self.arena.get_as::<PictureNode>(picture).thumbhash != thumbhash {
            let node = self.arena.get_mut_as::<PictureNode>(picture);
            node.placeholder = thumbhash.as_ref().and_then(Thumbhash::decode);
            node.thumbhash = thumbhash;
        }
    }

    pub fn set_picture_source(&mut self, picture: NodeOf<PictureNode>, source: Option<Rect>) {
        if self.arena.get_as::<PictureNode>(picture).source != source {
            self.arena.get_mut_as::<PictureNode>(picture).source = source;
        }
    }

    pub fn set_picture_fit(&mut self, picture: NodeOf<PictureNode>, fit: ImageFit) {
        if self.arena.get_as::<PictureNode>(picture).fit != fit {
            self.arena.paint_mut_as::<PictureNode>(picture).fit = fit;
        }
    }

    pub fn set_picture_tint(&mut self, picture: NodeOf<PictureNode>, tint: Color32) {
        if self.arena.get_as::<PictureNode>(picture).tint != tint {
            self.arena.paint_mut_as::<PictureNode>(picture).tint = tint;
        }
    }

    pub fn set_picture_radius(&mut self, picture: NodeOf<PictureNode>, radius: f32) {
        if self.arena.get_as::<PictureNode>(picture).radius != radius {
            self.arena.paint_mut_as::<PictureNode>(picture).radius = radius;
        }
    }

    pub fn set_picture_smooth(&mut self, picture: NodeOf<PictureNode>, smooth: bool) {
        if self.arena.get_as::<PictureNode>(picture).smooth != smooth {
            self.arena.paint_mut_as::<PictureNode>(picture).smooth = smooth;
        }
    }
}
