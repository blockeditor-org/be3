use crate::geometry::Rect;
use crate::painter::Shape;

const REGIONS: usize = 4;

#[derive(Clone, Copy)]
pub(crate) struct Region {
    rects: [Rect; REGIONS],
    count: usize,
}

impl Region {
    pub(crate) const NOTHING: Self = Self {
        rects: [Rect::NOTHING; REGIONS],
        count: 0,
    };

    pub(crate) fn rects(&self) -> &[Rect] {
        &self.rects[..self.count]
    }

    pub(crate) fn intersects(&self, rect: Rect) -> bool {
        self.rects().iter().any(|region| region.intersects(rect))
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
            clip,
            ..
        } => rect.expand(*stroke_width).intersect(*clip),
        Shape::Text {
            origin,
            galley,
            clip,
            ..
        } => Rect::from_min_size(*origin, galley.size()).intersect(*clip),
        Shape::Punch { rect, clip, .. } => rect.intersect(*clip),
    }
}

#[cfg(test)]
mod tests;
