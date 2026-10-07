use std::time::Instant;

use crate::context::Moved;
use crate::damage::Region;
use crate::geometry::{Rect, pos2};

pub const SPEED: f32 = 240.0;
const MINIMUM_ROWS: f32 = 1.0;

#[derive(Default)]
pub struct SlowRepaint {
    enabled: bool,
    sweeps: Vec<Sweep>,
    scanned: Option<Instant>,
}

struct Sweep {
    area: Rect,
    remaining: Rect,
    next: Option<Rect>,
}

impl Sweep {
    fn new(rect: Rect) -> Self {
        Self {
            area: rect,
            remaining: rect,
            next: None,
        }
    }

    fn started(&self) -> bool {
        self.remaining.top() > self.area.top()
    }
}

impl SlowRepaint {
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn scanning(&self) -> bool {
        !self.sweeps.is_empty()
    }

    pub fn shift(&mut self, moved: Moved) {
        let to = moved.to();
        let shifted = |rect: Rect| {
            let mut pieces = subtract(rect, to);
            pieces.push(rect.intersect(moved.from).translate(moved.by));
            pieces.retain(Rect::is_positive);
            pieces
        };
        let sweeps = std::mem::take(&mut self.sweeps);
        let mut queued = Vec::new();
        for sweep in &sweeps {
            for piece in shifted(sweep.remaining) {
                self.sweeps.push(Sweep::new(piece));
            }
            queued.extend(sweep.next.into_iter().flat_map(shifted));
        }
        for rect in queued {
            self.add(rect);
        }
    }

    pub fn scan(&mut self, region: Region, viewport: Rect, now: Instant) -> Vec<Rect> {
        if !self.enabled {
            self.scanned = None;
            let mut everything: Vec<Rect> = std::mem::take(&mut self.sweeps)
                .into_iter()
                .flat_map(|sweep| [Some(sweep.remaining), sweep.next])
                .flatten()
                .collect();
            everything.extend_from_slice(region.rects());
            return everything;
        }
        for rect in region.rects() {
            self.add(rect.intersect(viewport));
        }
        let elapsed = self
            .scanned
            .map_or(0.0, |at| now.saturating_duration_since(at).as_secs_f32());
        let rows = (SPEED * elapsed).max(MINIMUM_ROWS);
        let mut revealed = Vec::new();
        self.sweeps.retain_mut(|sweep| {
            let rect = sweep.remaining.intersect(viewport);
            sweep.remaining = Rect::NOTHING;
            if rect.is_positive() {
                let cut = (rect.top() + rows).min(rect.bottom());
                revealed.push(Rect::from_min_max(rect.min, pos2(rect.right(), cut)));
                sweep.remaining = Rect::from_min_max(pos2(rect.left(), cut), rect.max);
            }
            if sweep.remaining.is_positive() {
                return true;
            }
            match sweep.next.take() {
                Some(next) => {
                    *sweep = Sweep::new(next);
                    true
                }
                None => false,
            }
        });
        self.scanned = self.scanning().then_some(now);
        revealed
    }

    fn add(&mut self, rect: Rect) {
        if !rect.is_positive()
            || self
                .sweeps
                .iter()
                .any(|sweep| sweep.remaining.contains_rect(rect))
        {
            return;
        }
        if let Some(sweep) = self
            .sweeps
            .iter_mut()
            .find(|sweep| !sweep.started() && sweep.area.intersects(rect))
        {
            sweep.area = sweep.area.union(rect);
            sweep.remaining = sweep.area;
            return;
        }
        match self
            .sweeps
            .iter_mut()
            .find(|sweep| sweep.area.intersects(rect))
        {
            Some(sweep) => {
                sweep.next = Some(sweep.next.map_or(rect, |next| next.union(rect)));
            }
            None => self.sweeps.push(Sweep::new(rect)),
        }
    }
}

fn subtract(rect: Rect, cut: Rect) -> Vec<Rect> {
    let overlap = rect.intersect(cut);
    if !overlap.is_positive() {
        return vec![rect];
    }
    [
        Rect::from_min_max(rect.min, pos2(rect.right(), overlap.top())),
        Rect::from_min_max(pos2(rect.left(), overlap.bottom()), rect.max),
        Rect::from_min_max(
            pos2(rect.left(), overlap.top()),
            pos2(overlap.left(), overlap.bottom()),
        ),
        Rect::from_min_max(
            pos2(overlap.right(), overlap.top()),
            pos2(rect.right(), overlap.bottom()),
        ),
    ]
    .into_iter()
    .filter(Rect::is_positive)
    .collect()
}
