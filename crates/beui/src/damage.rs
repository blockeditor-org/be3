use crate::geometry::{Pos2, Rect};
use crate::painter::Shape;

const REGIONS: usize = 4;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Region {
    rects: [Rect; REGIONS],
    count: usize,
}

impl Region {
    pub(crate) const NOTHING: Self = Self {
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

    pub(crate) fn add(&mut self, rect: Rect) {
        if !rect.is_positive() {
            return;
        }
        for region in &mut self.rects[..self.count] {
            if region.intersects(rect) {
                *region = region.union(rect);
                return;
            }
        }
        if self.count < REGIONS {
            self.rects[self.count] = rect;
            self.count += 1;
            return;
        }
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

    fn clipped(&self, viewport: Rect) -> Self {
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

fn area(rect: Rect) -> f32 {
    rect.width() * rect.height()
}

pub(crate) struct Damage {
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
    pub(crate) fn add(&mut self, rect: Rect) {
        self.region.add(rect);
    }

    pub(crate) fn add_region(&mut self, region: Region) {
        self.region = self.region.union(region);
    }

    pub(crate) fn everything(&mut self) {
        self.everything = true;
    }

    pub(crate) fn take(&mut self, viewport: Rect) -> Region {
        let mut region = Region::NOTHING;
        match self.everything {
            true => region.add(viewport),
            false => region = self.region.clipped(viewport),
        }
        *self = Self::default();
        region
    }
}

pub(crate) fn bounds(shape: &Shape) -> Rect {
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
    }
}

pub(crate) fn line_bounds(from: Pos2, to: Pos2, width: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(from.x.min(to.x), from.y.min(to.y)),
        Pos2::new(from.x.max(to.x), from.y.max(to.y)),
    )
    .expand(width / 2.0 + 1.0)
}

#[cfg(test)]
mod tests;
