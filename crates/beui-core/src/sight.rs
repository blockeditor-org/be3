use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::painter::PainterState;

pub const CULLING_MARGIN: f32 = 256.0;
const BINDING: f32 = 0.01;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sight {
    exact: bool,
    within: Rect,
    reach: Rect,
}

impl Sight {
    pub const ANYWHERE: Self = Self {
        exact: false,
        within: Rect::EVERYTHING,
        reach: Rect::NOTHING,
    };

    pub fn region(state: PainterState) -> Rect {
        state
            .clip
            .intersect(state.space_clip)
            .expand(CULLING_MARGIN)
    }

    pub fn shows(size: Vec2, own: PainterState) -> bool {
        Self::region(own).intersects(Rect::from_min_size(Pos2::ZERO, size))
    }

    pub fn shown(size: Vec2) -> Self {
        let mut sight = Self::ANYWHERE;
        sight.reach.min = pos2(size.x, size.y);
        sight.reach.max = Pos2::ZERO;
        sight
    }

    pub fn hidden(size: Vec2, own: PainterState) -> Self {
        let region = Self::region(own);
        let mut sight = Self::ANYWHERE;
        for axis in [Axis::X, Axis::Y] {
            let (start, end) = axis.of(region);
            let length = axis.length(size);
            let (within_start, within_end) = axis.of_mut(&mut sight.within);
            if end <= 0.0 {
                *within_end = 0.0;
                return sight;
            }
            if start >= length {
                *within_start = length;
                return sight;
            }
        }
        sight
    }

    pub fn exact(self, exact: bool) -> Self {
        Self {
            exact: self.exact || exact,
            ..self
        }
    }

    pub fn holds(self, before: PainterState, now: PainterState) -> bool {
        (!self.exact || before.sees(now)) && self.covers(Self::region(now))
    }

    pub fn and(self, other: Self) -> Self {
        Self {
            exact: self.exact || other.exact,
            within: self.within.intersect(other.within),
            reach: self.reach.union(other.reach),
        }
    }

    pub fn beneath(self, own: PainterState, parent: PainterState) -> Self {
        if own.rotation.turns() {
            return Self::ANYWHERE.exact(true);
        }
        let offset = own.origin - parent.origin;
        self.translate(offset)
            .seen_through(Self::region(own).translate(offset), Self::region(parent))
    }

    fn covers(self, region: Rect) -> bool {
        [Axis::X, Axis::Y].into_iter().all(|axis| {
            let (start, end) = axis.of(region);
            let (within_start, within_end) = axis.of(self.within);
            let (reach_start, reach_end) = axis.of(self.reach);
            start >= within_start
                && end <= within_end
                && (start < reach_start || reach_start == f32::INFINITY)
                && (end > reach_end || reach_end == f32::NEG_INFINITY)
        })
    }

    fn translate(self, by: Vec2) -> Self {
        Self {
            within: self.within.translate(by),
            reach: self.reach.translate(by),
            ..self
        }
    }

    fn seen_through(mut self, inner: Rect, outer: Rect) -> Self {
        for axis in [Axis::X, Axis::Y] {
            let (inner_start, inner_end) = axis.of(inner);
            let (outer_start, outer_end) = axis.of(outer);
            let (within_start, within_end) = axis.of_mut(&mut self.within);
            let (reach_start, reach_end) = axis.of_mut(&mut self.reach);
            if inner_start > outer_start + BINDING {
                if inner_start >= *within_start {
                    *within_start = f32::NEG_INFINITY;
                }
                if inner_start >= *reach_start {
                    *within_start = f32::INFINITY;
                }
            }
            if inner_end < outer_end - BINDING {
                if inner_end <= *within_end {
                    *within_end = f32::INFINITY;
                }
                if inner_end <= *reach_end {
                    *within_end = f32::NEG_INFINITY;
                }
            }
        }
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Axis {
    X,
    Y,
}

impl Axis {
    fn of(self, rect: Rect) -> (f32, f32) {
        match self {
            Axis::X => (rect.min.x, rect.max.x),
            Axis::Y => (rect.min.y, rect.max.y),
        }
    }

    fn of_mut(self, rect: &mut Rect) -> (&mut f32, &mut f32) {
        match self {
            Axis::X => (&mut rect.min.x, &mut rect.max.x),
            Axis::Y => (&mut rect.min.y, &mut rect.max.y),
        }
    }

    fn length(self, size: Vec2) -> f32 {
        match self {
            Axis::X => size.x,
            Axis::Y => size.y,
        }
    }
}
