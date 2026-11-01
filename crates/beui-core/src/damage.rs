use crate::painter::Shape;
use crate::path::PathPaint;
use beui_types::WindowId;
use beui_types::geometry::Region;
use beui_types::{Pos2, Rect, Rotation, Vec2};

#[derive(Clone, Default, PartialEq, Debug)]
pub struct WindowRegions(Vec<(WindowId, Region)>);

impl WindowRegions {
    pub fn add(&mut self, window: WindowId, rect: Rect) {
        match self.0.iter_mut().find(|(held, _)| *held == window) {
            Some((_, region)) => region.add(rect),
            None => {
                let mut region = Region::NOTHING;
                region.add(rect);
                if !region.is_empty() {
                    self.0.push((window, region));
                }
            }
        }
    }

    pub fn add_region(&mut self, window: WindowId, region: Region) {
        for rect in region.rects() {
            self.add(window, *rect);
        }
    }

    pub fn union(&mut self, other: Self) {
        for (window, region) in other.0 {
            self.add_region(window, region);
        }
    }

    pub fn set(&mut self, window: WindowId, region: Region) {
        self.0.retain(|(held, _)| *held != window);
        if !region.is_empty() {
            self.0.push((window, region));
        }
    }

    pub fn of(&self, window: WindowId) -> Region {
        self.0
            .iter()
            .find(|(held, _)| *held == window)
            .map_or(Region::NOTHING, |(_, region)| *region)
    }
}

#[derive(Default)]
pub struct Damage {
    regions: WindowRegions,
    everything: bool,
}

pub struct Taken {
    regions: WindowRegions,
    everything: bool,
}

impl Taken {
    pub fn add(&mut self, window: WindowId, rect: Rect) {
        self.regions.add(window, rect);
    }

    pub fn set(&mut self, window: WindowId, region: Region) {
        self.regions.set(window, region);
    }

    pub fn is_everything(&self) -> bool {
        self.everything
    }

    pub fn of(&self, window: WindowId, viewport: Rect) -> Region {
        match self.everything {
            true => Region::from(viewport),
            false => self.regions.of(window).clipped(viewport),
        }
    }
}

impl Damage {
    pub fn add(&mut self, window: WindowId, rect: Rect) {
        self.regions.add(window, rect);
    }

    pub fn add_regions(&mut self, regions: WindowRegions) {
        self.regions.union(regions);
    }

    pub fn everything(&mut self) {
        self.everything = true;
    }

    pub fn take(&mut self) -> Taken {
        let taken = std::mem::take(self);
        Taken {
            regions: taken.regions,
            everything: taken.everything,
        }
    }
}

pub fn bounds(shape: &Shape) -> Rect {
    match shape {
        Shape::Rect {
            rect,
            stroke_width,
            rotation,
            clip,
            ..
        } => rotation.bounds(rect.expand(*stroke_width)).intersect(*clip),
        Shape::Text {
            origin,
            galley,
            scale,
            rotation,
            clip,
            ..
        } => rotation
            .bounds(Rect::from_min_size(*origin, galley.size() * *scale))
            .intersect(*clip),
        Shape::Line {
            from,
            to,
            width,
            clip,
            ..
        } => line_bounds(*from, *to, *width).intersect(*clip),
        Shape::Image {
            rect,
            rotation,
            clip,
            ..
        }
        | Shape::Punch {
            rect,
            rotation,
            clip,
            ..
        } => rotation.bounds(*rect).intersect(*clip),
        Shape::Drawing { rect, clip, .. } => rect.intersect(*clip),
        Shape::Path {
            path,
            scale,
            offset,
            rotation,
            clip,
            ..
        } => path_bounds(path, *scale, *offset, *rotation).intersect(*clip),
    }
}

pub fn path_bounds(path: &PathPaint, scale: f32, offset: Vec2, rotation: Rotation) -> Rect {
    let bounds = path.bounds();
    if !bounds.is_positive() {
        return Rect::NOTHING;
    }
    rotation.bounds(bounds.scaled(scale).translate(offset).expand(1.0))
}

pub fn line_bounds(from: Pos2, to: Pos2, width: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(from.x.min(to.x), from.y.min(to.y)),
        Pos2::new(from.x.max(to.x), from.y.max(to.y)),
    )
    .expand(width / 2.0 + 1.0)
}

#[cfg(test)]
mod tests;
