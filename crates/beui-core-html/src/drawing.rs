use std::any::Any;
use std::rc::Rc;

use beui_types::{Color32, Corners, Image, PixelGrid, Rect};

pub struct Painter(());

impl Painter {
    pub fn pixel_grid(&self) -> PixelGrid {
        PixelGrid::default()
    }

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

    pub fn drawing(&self, _rect: Rect, _drawing: &Drawing) {}
}

#[derive(Clone)]
pub struct Drawing {
    _draw: Rc<dyn Any>,
}

impl Drawing {
    pub fn erased(draw: impl Any) -> Self {
        Self {
            _draw: Rc::new(draw),
        }
    }
}

pub type Draw = Rc<dyn Fn(&Painter, Rect)>;
