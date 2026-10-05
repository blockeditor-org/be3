use std::time::Instant;

use crate::context::Moved;
use crate::damage::Region;
use crate::geometry::{Rect, pos2};

pub const SPEED: f32 = 240.0;
const MINIMUM_ROWS: f32 = 1.0;

#[derive(Default)]
pub struct SlowRepaint {
    enabled: bool,
    pending: Vec<Rect>,
    scanned: Option<Instant>,
}

impl SlowRepaint {
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn scanning(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn shift(&mut self, moved: Moved) {
        let to = moved.to();
        let mut shifted = Vec::new();
        for rect in &self.pending {
            shifted.extend(subtract(*rect, to));
            let carried = rect.intersect(moved.from).translate(moved.by);
            if carried.is_positive() {
                shifted.push(carried);
            }
        }
        self.pending.clear();
        for rect in shifted {
            self.add(rect);
        }
    }

    pub fn scan(&mut self, region: Region, viewport: Rect, now: Instant) -> Vec<Rect> {
        if !self.enabled {
            self.scanned = None;
            let mut everything = std::mem::take(&mut self.pending);
            everything.extend_from_slice(region.rects());
            return everything;
        }
        for rect in region.rects() {
            self.add(*rect);
        }
        self.pending.retain_mut(|rect| {
            *rect = rect.intersect(viewport);
            rect.is_positive()
        });
        let elapsed = self
            .scanned
            .map_or(0.0, |at| now.saturating_duration_since(at).as_secs_f32());
        self.scanned = self.scanning().then_some(now);
        let rows = (SPEED * elapsed).max(MINIMUM_ROWS);
        let mut revealed = Vec::new();
        self.pending.retain_mut(|rect| {
            let cut = (rect.top() + rows).min(rect.bottom());
            revealed.push(Rect::from_min_max(rect.min, pos2(rect.right(), cut)));
            rect.min.y = cut;
            rect.is_positive()
        });
        revealed
    }

    fn add(&mut self, rect: Rect) {
        if !rect.is_positive() {
            return;
        }
        let mut merged = rect;
        loop {
            let before = self.pending.len();
            self.pending.retain(|pending| {
                let overlaps = pending.intersects(merged);
                if overlaps {
                    merged = merged.union(*pending);
                }
                !overlaps
            });
            if self.pending.len() == before {
                break;
            }
        }
        self.pending.push(merged);
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
