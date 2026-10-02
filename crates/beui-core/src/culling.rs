use crate::geometry::{Rect, Vec2};
use crate::painter::PainterState;

pub const CULLING_MARGIN: f32 = 256.0;
const BINDING: f32 = 0.01;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Culling {
    within: Rect,
    reach: Rect,
}

impl Culling {
    pub const ANY: Self = Self {
        within: Rect::EVERYTHING,
        reach: Rect::NOTHING,
    };

    pub fn region(state: PainterState) -> Rect {
        state
            .clip
            .intersect(state.space_clip)
            .expand(CULLING_MARGIN)
    }

    pub fn keep(&mut self, start: f32, end: f32, axis: Axis) {
        let (reach_start, reach_end) = axis.of_mut(&mut self.reach);
        *reach_start = reach_start.min(end);
        *reach_end = reach_end.max(start);
    }

    pub fn cull_before(&mut self, end: f32, axis: Axis) {
        let (within_start, _) = axis.of_mut(&mut self.within);
        *within_start = within_start.max(end);
    }

    pub fn cull_after(&mut self, start: f32, axis: Axis) {
        let (_, within_end) = axis.of_mut(&mut self.within);
        *within_end = within_end.min(start);
    }

    pub fn holds(self, region: Rect) -> bool {
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

    pub fn and(self, other: Self) -> Self {
        Self {
            within: self.within.intersect(other.within),
            reach: self.reach.union(other.reach),
        }
    }

    pub fn translate(self, by: Vec2) -> Self {
        Self {
            within: self.within.translate(by),
            reach: self.reach.translate(by),
        }
    }

    pub fn seen_through(mut self, inner: Rect, outer: Rect) -> Self {
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
pub enum Axis {
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
}
