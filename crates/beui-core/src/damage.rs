use crate::geometry::{Pos2, Rect, Rotation, Vec2};
use crate::painter::Shape;
use crate::path::PathPaint;

const REGIONS: usize = 4;

const MERGE_WASTE: f32 = 2.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Region {
    rects: [Rect; REGIONS],
    count: usize,
}

impl Region {
    pub const NOTHING: Self = Self {
        rects: [Rect::NOTHING; REGIONS],
        count: 0,
    };

    pub fn rects(&self) -> &[Rect] {
        &self.rects[..self.count]
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn bounds(&self) -> Rect {
        self.rects()
            .iter()
            .fold(Rect::NOTHING, |bounds, rect| bounds.union(*rect))
    }

    pub fn union(mut self, other: Self) -> Self {
        for rect in other.rects() {
            self.add(*rect);
        }
        self
    }

    pub fn add(&mut self, rect: Rect) {
        if !rect.is_positive() {
            return;
        }
        if let Some(region) = self.rects[..self.count]
            .iter_mut()
            .find(|region| region.intersects(rect) && !wasteful(**region, rect))
        {
            *region = region.union(rect);
        } else if self.count < REGIONS {
            self.rects[self.count] = rect;
            self.count += 1;
        } else {
            let mut chosen = 0;
            let mut cheapest = f32::INFINITY;
            for (index, region) in self.rects.iter().enumerate() {
                let growth = area(region.union(rect)) - area(*region);
                if growth < cheapest {
                    cheapest = growth;
                    chosen = index;
                }
            }
            self.rects[chosen] = self.rects[chosen].union(rect);
        }
        self.merge_close();
    }

    fn merge_close(&mut self) {
        'merged: loop {
            for first in 0..self.count {
                for second in first + 1..self.count {
                    let (a, b) = (self.rects[first], self.rects[second]);
                    if !wasteful(a, b) {
                        let union = a.union(b);
                        self.rects[first] = union;
                        self.rects.copy_within(second + 1..self.count, second);
                        self.count -= 1;
                        self.rects[self.count] = Rect::NOTHING;
                        continue 'merged;
                    }
                }
            }
            return;
        }
    }

    pub fn clipped(&self, viewport: Rect) -> Self {
        let mut clipped = Self::NOTHING;
        for rect in self.rects() {
            clipped.add(rect.intersect(viewport));
        }
        clipped
    }
}

impl From<Rect> for Region {
    fn from(rect: Rect) -> Self {
        let mut region = Self::NOTHING;
        region.add(rect);
        region
    }
}

impl Default for Region {
    fn default() -> Self {
        Self::NOTHING
    }
}

fn wasteful(a: Rect, b: Rect) -> bool {
    area(a.union(b)) > MERGE_WASTE * (area(a) + area(b))
}

fn area(rect: Rect) -> f32 {
    rect.width() * rect.height()
}

pub struct Damage {
    region: Region,
    everything: bool,
}

impl Default for Damage {
    fn default() -> Self {
        Self {
            region: Region::NOTHING,
            everything: false,
        }
    }
}

impl Damage {
    pub fn add(&mut self, rect: Rect) {
        self.region.add(rect);
    }

    pub fn add_region(&mut self, region: Region) {
        self.region = self.region.union(region);
    }

    pub fn everything(&mut self) {
        self.everything = true;
    }

    pub fn is_everything(&self) -> bool {
        self.everything
    }

    pub fn take(&mut self, viewport: Rect) -> Region {
        let mut region = Region::NOTHING;
        match self.everything {
            true => region.add(viewport),
            false => region = self.region.clipped(viewport),
        }
        *self = Self::default();
        region
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
            rotation,
            clip,
            ..
        } => rotation
            .bounds(Rect::from_min_size(*origin, galley.size()))
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
