use std::collections::HashMap;
use std::rc::Rc;

use beui::{CursorIcon, Vec2, vec2};

use crate::arrow;
use crate::gpu::{Gpu, Sprite};
use crate::theme::{Image, Theme};

pub fn legacy_names(icon: CursorIcon) -> &'static [&'static str] {
    match icon {
        CursorIcon::Default => &["left_ptr", "arrow", "top_left_arrow", "left_arrow"],
        CursorIcon::ContextMenu => &[],
        CursorIcon::Help => &["question_arrow", "whats_this", "left_ptr_help"],
        CursorIcon::PointingHand => &["hand2", "hand1", "hand", "pointing_hand"],
        CursorIcon::Progress => &["left_ptr_watch", "half-busy"],
        CursorIcon::Wait => &["watch"],
        CursorIcon::Cell => &["plus"],
        CursorIcon::Crosshair => &["cross", "tcross"],
        CursorIcon::Text => &["xterm", "ibeam"],
        CursorIcon::VerticalText => &[],
        CursorIcon::Alias => &["dnd-link", "link"],
        CursorIcon::Copy => &["dnd-copy"],
        CursorIcon::Move => &["fleur", "dnd-move", "size_all"],
        CursorIcon::NoDrop => &["dnd-no-drop", "dnd-none", "circle", "forbidden"],
        CursorIcon::NotAllowed => &["crossed_circle", "forbidden", "circle"],
        CursorIcon::Grab => &["openhand", "hand1"],
        CursorIcon::Grabbing => &["closedhand", "dnd-none"],
        CursorIcon::ResizeEast => &["right_side"],
        CursorIcon::ResizeNorth => &["top_side"],
        CursorIcon::ResizeNorthEast => &["top_right_corner"],
        CursorIcon::ResizeNorthWest => &["top_left_corner"],
        CursorIcon::ResizeSouth => &["bottom_side"],
        CursorIcon::ResizeSouthEast => &["bottom_right_corner"],
        CursorIcon::ResizeSouthWest => &["bottom_left_corner"],
        CursorIcon::ResizeWest => &["left_side"],
        CursorIcon::ResizeHorizontal => &["sb_h_double_arrow", "h_double_arrow", "size_hor"],
        CursorIcon::ResizeVertical => &["sb_v_double_arrow", "v_double_arrow", "size_ver"],
        CursorIcon::ResizeNeSw => &["fd_double_arrow", "size_bdiag"],
        CursorIcon::ResizeNwSe => &["bd_double_arrow", "size_fdiag"],
        CursorIcon::ResizeColumn => &["split_h", "sb_h_double_arrow"],
        CursorIcon::ResizeRow => &["split_v", "sb_v_double_arrow"],
        CursorIcon::AllScroll => &["fleur", "size_all"],
        CursorIcon::ZoomIn => &["zoom_in"],
        CursorIcon::ZoomOut => &["zoom_out"],
        CursorIcon::None | CursorIcon::Touch => &[],
    }
}

pub fn names(icon: CursorIcon) -> Vec<&'static str> {
    if icon == CursorIcon::None {
        return Vec::new();
    }
    let mut names = vec![icon.name()];
    names.extend_from_slice(legacy_names(icon));
    names
}

#[derive(Debug, PartialEq)]
pub enum Choice {
    Hidden,
    Arrow,
    Touch(beui::native::touch_cursor::CursorImage),
    Themed {
        image: Image,
        size: Vec2,
        hotspot: Vec2,
    },
}

pub fn choose(theme: &Theme, icon: CursorIcon, scale: f32) -> Choice {
    match icon {
        CursorIcon::None => return Choice::Hidden,
        CursorIcon::Touch => return Choice::Touch(beui::native::touch_cursor::image(scale)),
        _ => {}
    }
    let wanted = (theme.size() as f32 * scale).round().max(1.0) as u32;
    let image = theme
        .load(&names(icon), wanted)
        .or_else(|| theme.load(&names(CursorIcon::Default), wanted));
    let Some(image) = image else {
        return Choice::Arrow;
    };
    let points = theme.size() as f32 / image.nominal.max(1) as f32;
    Choice::Themed {
        size: vec2(image.width as f32, image.height as f32) * points,
        hotspot: vec2(image.hotspot.0 as f32, image.hotspot.1 as f32) * points,
        image,
    }
}

pub struct Shape {
    pub sprite: Sprite,
    pub size: Vec2,
    pub hotspot: Vec2,
}

pub struct Cursors {
    theme: Theme,
    scale: f32,
    loaded: HashMap<CursorIcon, Option<Rc<Shape>>>,
}

impl Cursors {
    pub fn new(theme: Theme, scale: f32) -> Self {
        Self {
            theme,
            scale,
            loaded: HashMap::new(),
        }
    }

    pub fn get(&mut self, gpu: &Gpu, icon: CursorIcon) -> Option<Rc<Shape>> {
        let (theme, scale) = (&self.theme, self.scale);
        self.loaded
            .entry(icon)
            .or_insert_with(|| {
                let shape = match choose(theme, icon, scale) {
                    Choice::Hidden => return None,
                    Choice::Arrow => Shape {
                        sprite: gpu.rgba(arrow::WIDTH, arrow::HEIGHT, &arrow::pixels()),
                        size: vec2(arrow::WIDTH as f32, arrow::HEIGHT as f32),
                        hotspot: Vec2::ZERO,
                    },
                    Choice::Touch(image) => Shape {
                        sprite: gpu.rgba(image.width, image.height, &image.rgba),
                        size: Vec2::splat(beui::native::touch_cursor::SIZE),
                        hotspot: Vec2::splat(beui::native::touch_cursor::SIZE / 2.0),
                    },
                    Choice::Themed {
                        image,
                        size,
                        hotspot,
                    } => Shape {
                        sprite: gpu.rgba(image.width, image.height, &image.rgba),
                        size,
                        hotspot,
                    },
                };
                Some(Rc::new(shape))
            })
            .clone()
    }
}

#[cfg(test)]
mod tests;
