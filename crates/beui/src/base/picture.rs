use std::any::Any;

use beui_macros::component;

use crate::color::Color32;
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::image::{Image, ImageFit};
use crate::node::{Element, NodeId, NodeMap};
use crate::painter::Painter;
use crate::reactive::{Prop, create_effect, with_document};

pub(crate) struct PictureNode {
    image: Option<Image>,
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
    fn shown_size(&self, image: &Image) -> Vec2 {
        let size = image.size();
        match self.source {
            Some(source) => Vec2::new(size.x * source.width(), size.y * source.height()),
            None => size,
        }
    }
}

impl Element for PictureNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, available: Vec2) -> Vec2 {
        let Some(image) = self.image.as_ref() else {
            return Vec2::ZERO;
        };
        let size = self.shown_size(image);
        if !available.x.is_finite() || available.x <= 0.0 {
            return size;
        }
        match size.x > available.x {
            true => Vec2::new(available.x, size.y * available.x / size.x),
            false => size,
        }
    }

    fn layout(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _rect: Rect,
        _out: &mut NodeMap<Rect>,
    ) {
    }

    fn paint(&self, _doc: &Document, painter: &Painter, _rects: &NodeMap<Rect>, rect: Rect) {
        let Some(image) = self.image.as_ref() else {
            return;
        };
        painter.image(
            self.fit.place(rect, self.shown_size(image)),
            self.source.unwrap_or(WHOLE),
            image,
            self.tint,
            self.radius,
            self.smooth,
        );
    }

    fn children(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn kind(&self) -> &'static str {
        "picture"
    }

    fn detail(&self) -> Option<String> {
        self.image
            .as_ref()
            .map(|image| format!("{}x{}", image.width(), image.height()))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[component]
pub fn Picture(
    image: Prop<Option<Image>>,
    #[prop(default = None)] source: Prop<Option<Rect>>,
    #[prop(default = ImageFit::Contain)] fit: Prop<ImageFit>,
    #[prop(default = Color32::WHITE)] tint: Prop<Color32>,
    #[prop(default = 0.0)] radius: Prop<f32>,
    #[prop(default = true)] smooth: Prop<bool>,
) -> NodeId {
    let picture = with_document(Document::create_picture);
    create_effect(move || {
        let image = image.get();
        with_document(|document| document.set_picture_image(picture, image));
    });
    create_effect(move || {
        let source = source.get();
        with_document(|document| document.set_picture_source(picture, source));
    });
    create_effect(move || {
        let fit = fit.get();
        with_document(|document| document.set_picture_fit(picture, fit));
    });
    create_effect(move || {
        let tint = tint.get();
        with_document(|document| document.set_picture_tint(picture, tint));
    });
    create_effect(move || {
        let radius = radius.get();
        with_document(|document| document.set_picture_radius(picture, radius));
    });
    create_effect(move || {
        let smooth = smooth.get();
        with_document(|document| document.set_picture_smooth(picture, smooth));
    });
    picture
}

impl Document {
    pub(crate) fn create_picture(&mut self) -> NodeId {
        self.arena.insert(PictureNode {
            image: None,
            source: None,
            fit: ImageFit::Contain,
            tint: Color32::WHITE,
            radius: 0.0,
            smooth: true,
        })
    }

    pub(crate) fn set_picture_image(&mut self, picture: NodeId, image: Option<Image>) {
        if self.arena.get_as::<PictureNode>(picture).image != image {
            self.arena.get_mut_as::<PictureNode>(picture).image = image;
        }
    }

    pub(crate) fn set_picture_source(&mut self, picture: NodeId, source: Option<Rect>) {
        if self.arena.get_as::<PictureNode>(picture).source != source {
            self.arena.get_mut_as::<PictureNode>(picture).source = source;
        }
    }

    pub(crate) fn set_picture_fit(&mut self, picture: NodeId, fit: ImageFit) {
        if self.arena.get_as::<PictureNode>(picture).fit != fit {
            self.arena.get_mut_as::<PictureNode>(picture).fit = fit;
        }
    }

    pub(crate) fn set_picture_tint(&mut self, picture: NodeId, tint: Color32) {
        if self.arena.get_as::<PictureNode>(picture).tint != tint {
            self.arena.get_mut_as::<PictureNode>(picture).tint = tint;
        }
    }

    pub(crate) fn set_picture_radius(&mut self, picture: NodeId, radius: f32) {
        if self.arena.get_as::<PictureNode>(picture).radius != radius {
            self.arena.get_mut_as::<PictureNode>(picture).radius = radius;
        }
    }

    pub(crate) fn set_picture_smooth(&mut self, picture: NodeId, smooth: bool) {
        if self.arena.get_as::<PictureNode>(picture).smooth != smooth {
            self.arena.get_mut_as::<PictureNode>(picture).smooth = smooth;
        }
    }
}
