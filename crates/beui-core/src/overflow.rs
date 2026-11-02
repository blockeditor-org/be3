use std::fmt;

use crate::base::canvas::CanvasNode;
use crate::base::offset::OffsetNode;
use crate::document::Document;
use crate::node::{Rects, SpaceId};
use beui_types::Direction;
use beui_types::NodeId;
use beui_types::Rect;

const TOLERANCE: f32 = 1.0;
const EXACT: f32 = 0.01;
const OUTLINE_SEARCH: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OverflowKind {
    CutOff,
    Spills,
    RingCut,
}

#[derive(Clone, Debug)]
pub struct Overflow {
    pub node: NodeId,
    pub kind: OverflowKind,
    pub rect: Rect,
    pub bounds: Rect,
    pub path: String,
}

impl fmt::Display for Overflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.kind {
            OverflowKind::CutOff => "is cut off by a clip nothing scrolls",
            OverflowKind::Spills => "draws outside its own box",
            OverflowKind::RingCut => "has its outline cut by a clip while all of its box shows",
        };
        let rect = self.rect;
        let bounds = self.bounds;
        write!(
            f,
            "{} {what}: [{:.1},{:.1}]-[{:.1},{:.1}] against [{:.1},{:.1}]-[{:.1},{:.1}]",
            self.path,
            rect.min.x,
            rect.min.y,
            rect.max.x,
            rect.max.y,
            bounds.min.x,
            bounds.min.y,
            bounds.max.x,
            bounds.max.y,
        )
    }
}

impl Document {
    pub fn overflows(&self) -> Vec<Overflow> {
        let mut found = Vec::new();
        let Some(root) = self.root() else {
            return found;
        };
        let mut pending = vec![(root, false)];
        while let Some((id, parent_cut)) = pending.pop() {
            if !self.arena.contains(id) {
                continue;
            }
            let Some(rect) = self.rects.get(&id) else {
                continue;
            };
            let bounds = self.reachable(id);
            let cut = rect.is_positive() && !within(rect, bounds);
            if cut && !parent_cut {
                found.push(self.overflow(id, OverflowKind::CutOff, rect, bounds));
            }
            if let Some(outlined) = self.arena.get(id).outline(self.pixel_grid(), rect) {
                let shown = self.shown_area(id);
                if contains(shown, outlined.inner) && !contains(shown, outlined.outer) {
                    found.push(self.overflow(id, OverflowKind::RingCut, outlined.outer, shown));
                }
            }
            if self.is_culled(id) {
                continue;
            }
            if let Some(ink) = self.arena.get(id).ink() {
                let ink = ink
                    .scaled(self.rects.scale_of(&id))
                    .translate(rect.min.to_vec2());
                if ink.is_positive() && !within(ink, rect) {
                    found.push(self.overflow(id, OverflowKind::Spills, ink, rect));
                }
            }
            for child in self.arena.get(id).live_children() {
                pending.push((child, cut));
            }
        }
        found
    }

    fn shown_area(&self, id: NodeId) -> Rect {
        self.rects
            .clips_above(&id)
            .into_iter()
            .fold(Rect::EVERYTHING, |shown, (_, clip)| shown.intersect(clip))
    }

    pub(crate) fn outlined_extent(&self, rects: &Rects, id: NodeId) -> Option<Rect> {
        let mut extent = rects.get(&id)?;
        let grid = self.pixel_grid();
        let mut pending = vec![id];
        let mut visited = 0;
        while let Some(node) = pending.pop() {
            visited += 1;
            if visited > OUTLINE_SEARCH || !self.arena.contains(node) {
                break;
            }
            let element = self.arena.get(node);
            if let Some(outlined) = rects
                .get(&node)
                .and_then(|rect| element.outline(grid, rect))
            {
                extent = extent.union(outlined.outer);
            }
            pending.extend(element.live_children());
        }
        Some(extent)
    }

    fn reachable(&self, id: NodeId) -> Rect {
        let mut bounds = Rect::EVERYTHING;
        let (mut free_x, mut free_y) = (false, false);
        for (space, clip) in self.rects.clips_above(&id) {
            let (x, y) = self.scrolls(space);
            free_x |= x;
            free_y |= y;
            let mut clip = clip;
            if free_x {
                clip.min.x = f32::NEG_INFINITY;
                clip.max.x = f32::INFINITY;
            }
            if free_y {
                clip.min.y = f32::NEG_INFINITY;
                clip.max.y = f32::INFINITY;
            }
            bounds = bounds.intersect(clip);
        }
        bounds
    }

    fn scrolls(&self, space: SpaceId) -> (bool, bool) {
        let host = space.node();
        if space.slot() == 0 || !self.arena.contains(host) {
            return (false, false);
        }
        let element = self.arena.get(host).as_any();
        if let Some(offset) = element.downcast_ref::<OffsetNode>() {
            return match offset.direction {
                Direction::Horizontal => (true, false),
                Direction::Vertical => (false, true),
            };
        }
        match element.downcast_ref::<CanvasNode>() {
            Some(canvas) => (canvas.pans() || canvas.scrolls_across(), canvas.pans()),
            None => (false, false),
        }
    }

    fn overflow(&self, id: NodeId, kind: OverflowKind, rect: Rect, bounds: Rect) -> Overflow {
        Overflow {
            node: id,
            kind,
            rect,
            bounds,
            path: self.node_path(id),
        }
    }

    pub fn node_path(&self, id: NodeId) -> String {
        let mut parts = Vec::new();
        let mut current = Some(id);
        while let Some(node) = current {
            if !self.arena.contains(node) || parts.len() > 64 {
                break;
            }
            let mut part = self.node_kind(node).to_string();
            if let Some(detail) = self
                .node_detail(node)
                .filter(|_| self.node_kind(node) != "portal")
            {
                let detail: String = detail.chars().take(40).collect();
                part.push_str(&format!(" {detail:?}"));
            }
            let ids = self.node_test_ids(node);
            if !ids.is_empty() {
                part.push_str(&format!(" #{}", ids.join(" #")));
            }
            parts.push(part);
            current = self.node_parent(node);
        }
        let mut shown: Vec<&str> = parts.iter().take(3).map(String::as_str).collect();
        let named = parts.iter().skip(3).find(|part| part.contains(" #"));
        if parts.len() > 3 {
            shown.push("…");
        }
        if let Some(named) = named {
            shown.push(named);
        }
        shown.join(" < ")
    }
}

fn within(inner: Rect, outer: Rect) -> bool {
    let outer = outer.expand(TOLERANCE);
    inner.min.x >= outer.min.x
        && inner.min.y >= outer.min.y
        && inner.max.x <= outer.max.x
        && inner.max.y <= outer.max.y
}

fn contains(outer: Rect, inner: Rect) -> bool {
    let outer = outer.expand(EXACT);
    inner.min.x >= outer.min.x
        && inner.min.y >= outer.min.y
        && inner.max.x <= outer.max.x
        && inner.max.y <= outer.max.y
}
