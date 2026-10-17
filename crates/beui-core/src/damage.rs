use crate::geometry::{Pos2, Rect};
use crate::paint::uncovered;
use crate::painter::Shape;

const REGIONS: usize = 8;

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
        self.insert(rect);
        self.merge_close();
    }

    fn insert(&mut self, rect: Rect) {
        if !rect.is_positive() {
            return;
        }
        if let Some(index) = self.rects().iter().position(|region| joins(*region, rect)) {
            let union = self.rects[index].union(rect);
            self.rects[index] = self.rects[self.count - 1];
            self.count -= 1;
            self.insert(union);
            return;
        }
        if let Some(overlapped) = self.rects().iter().find(|region| region.intersects(rect)) {
            for piece in uncovered(rect, *overlapped) {
                self.insert(piece);
            }
            return;
        }
        if self.count < REGIONS {
            self.rects[self.count] = rect;
            self.count += 1;
            return;
        }
        let mut all = self.rects.to_vec();
        all.push(rect);
        let mut cheapest = None;
        for first in 0..all.len() {
            for second in first + 1..all.len() {
                let (merged, absorbed) = cascade(&all, first, second);
                let growth =
                    area(merged) - absorbed.iter().map(|index| area(all[*index])).sum::<f32>();
                if cheapest
                    .as_ref()
                    .is_none_or(|(least, _, _)| growth < *least)
                {
                    cheapest = Some((growth, merged, absorbed));
                }
            }
        }
        let (_, merged, absorbed) = cheapest.expect("a full region has a pair to merge");
        self.replace(all, merged, &absorbed);
    }

    fn merge_close(&mut self) {
        'merged: loop {
            let rects = self.rects().to_vec();
            for first in 0..rects.len() {
                for second in first + 1..rects.len() {
                    let (merged, absorbed) = cascade(&rects, first, second);
                    let covered: f32 = absorbed.iter().map(|index| area(rects[*index])).sum();
                    if area(merged) <= MERGE_WASTE * covered {
                        self.replace(rects, merged, &absorbed);
                        continue 'merged;
                    }
                }
            }
            return;
        }
    }

    fn replace(&mut self, rects: Vec<Rect>, merged: Rect, absorbed: &[usize]) {
        *self = Self::NOTHING;
        for (index, region) in rects.into_iter().enumerate() {
            if !absorbed.contains(&index) {
                self.rects[self.count] = region;
                self.count += 1;
            }
        }
        self.rects[self.count] = merged;
        self.count += 1;
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

fn joins(a: Rect, b: Rect) -> bool {
    let overlap = match a.intersects(b) {
        true => area(a.intersect(b)),
        false => 0.0,
    };
    let union = a.union(b);
    let touches =
        union.width() <= a.width() + b.width() && union.height() <= a.height() + b.height();
    touches && area(union) <= area(a) + area(b) - overlap
}

fn cascade(rects: &[Rect], first: usize, second: usize) -> (Rect, Vec<usize>) {
    let mut merged = rects[first].union(rects[second]);
    let mut absorbed = vec![first, second];
    while let Some(index) =
        (0..rects.len()).find(|index| !absorbed.contains(index) && rects[*index].intersects(merged))
    {
        merged = merged.union(rects[index]);
        absorbed.push(index);
    }
    (merged, absorbed)
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
    }
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
