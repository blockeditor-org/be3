use beui::{Pos2, Rect};

use crate::layout::{absolute, bounds};
use crate::rotation::Rotation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Touch,
    Pen,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pointer {
    pub kind: Kind,
    pub size: Option<(f64, f64)>,
    pub output: Option<String>,
}

impl Pointer {
    pub fn pen() -> Self {
        Self {
            kind: Kind::Pen,
            size: None,
            output: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub id: String,
    pub connector: String,
    pub size: Option<(u32, u32)>,
    pub rect: Rect,
    pub rotation: Rotation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Output(usize),
    All,
}

pub fn target(pointer: &Pointer, screens: &[Screen], chosen: Option<&str>) -> Target {
    let named = |wanted: &str| {
        screens
            .iter()
            .position(|screen| screen.id == wanted || screen.connector == wanted)
    };
    if let Some(index) = chosen.and_then(named) {
        return Target::Output(index);
    }
    automatic(pointer, screens)
}

pub fn automatic(pointer: &Pointer, screens: &[Screen]) -> Target {
    if let Some(index) = pointer.output.as_deref().and_then(|wanted| {
        screens
            .iter()
            .position(|screen| screen.connector == wanted || screen.id == wanted)
    }) {
        return Target::Output(index);
    }
    if screens.len() == 1 {
        return Target::Output(0);
    }
    let fitting: Vec<usize> = (0..screens.len())
        .filter(|&index| fits(pointer.size, screens[index].size))
        .collect();
    let prefer_built_in =
        |among: &[usize]| among.iter().copied().find(|&index| built_in(&screens[index]));
    if let Some(&first) = fitting.first() {
        return Target::Output(prefer_built_in(&fitting).unwrap_or(first));
    }
    match pointer.kind {
        Kind::Pen => Target::All,
        Kind::Touch if screens.is_empty() => Target::All,
        Kind::Touch => {
            let every: Vec<usize> = (0..screens.len()).collect();
            Target::Output(prefer_built_in(&every).unwrap_or(0))
        }
    }
}

pub fn area(target: Target, areas: &[(Rect, Rotation)]) -> (Rect, Rotation) {
    match target {
        Target::Output(index) if let Some(&area) = areas.get(index) => area,
        _ => {
            let rects: Vec<Rect> = areas.iter().map(|(rect, _)| *rect).collect();
            (bounds(&rects), Rotation::Normal)
        }
    }
}

pub fn place(normalized: Pos2, target: Target, areas: &[(Rect, Rotation)]) -> Pos2 {
    let (rect, rotation) = area(target, areas);
    absolute(normalized, rect, rotation)
}

pub fn shown(target: Target, screens: &[Screen]) -> Option<String> {
    match target {
        Target::Output(index) => screens.get(index).map(|screen| screen.id.clone()),
        Target::All => None,
    }
}

pub fn built_in(screen: &Screen) -> bool {
    ["eDP", "LVDS", "DSI"]
        .iter()
        .any(|prefix| screen.connector.starts_with(prefix))
}

const SLACK_MM: f64 = 10.0;
const SLACK_SHARE: f64 = 0.05;

fn fits(pointer: Option<(f64, f64)>, screen: Option<(u32, u32)>) -> bool {
    let (Some((width, height)), Some((screen_width, screen_height))) = (pointer, screen) else {
        return false;
    };
    if width <= 0.0 || height <= 0.0 || screen_width == 0 || screen_height == 0 {
        return false;
    }
    let close = |a: f64, b: u32| {
        let b = f64::from(b);
        (a - b).abs() <= SLACK_MM.max(b * SLACK_SHARE)
    };
    (close(width, screen_width) && close(height, screen_height))
        || (close(width, screen_height) && close(height, screen_width))
}

#[cfg(test)]
mod tests;
