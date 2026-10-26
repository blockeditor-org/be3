use std::rc::Rc;

use beui_types::{Color32, Corners, Image, Rect};

pub struct Painter(());

impl Painter {
    pub fn rect_filled(&self, _rect: Rect, _corner_radius: impl Into<Corners>, _color: Color32) {}

    pub fn rect_stroke(
        &self,
        _rect: Rect,
        _corner_radius: impl Into<Corners>,
        _width: f32,
        _color: Color32,
    ) {
    }

    pub fn image(
        &self,
        _rect: Rect,
        _source: Rect,
        _image: &Image,
        _tint: Color32,
        _corner_radius: f32,
        _smooth: bool,
    ) {
    }
}

pub type Draw = Rc<dyn Fn(&Painter, Rect)>;
