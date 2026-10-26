use beui_core::geometry::{Pos2, Vec2, pos2, vec2};
use beui_core::window::{WindowEdge, WindowId};

use crate::panel::{Panel, Pose};

pub const SMALLEST: Vec2 = vec2(120.0, 80.0);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Resize {
    pub window: WindowId,
    pub edge: WindowEdge,
    panel: Panel,
    grab: Pos2,
}

impl Resize {
    pub fn begin(panel: &Panel, edge: WindowEdge, grab: Pos2) -> Self {
        Self {
            window: panel.window,
            edge,
            panel: *panel,
            grab,
        }
    }

    pub fn original(&self) -> Panel {
        self.panel
    }

    pub fn to(&self, pos: Pos2) -> (Pose, Vec2) {
        let delta = pos - self.grab;
        let (left, right, top, bottom) = sides(self.edge);
        let was = self.panel.size;
        let mut size = was;
        if right {
            size.x += delta.x;
        }
        if left {
            size.x -= delta.x;
        }
        if bottom {
            size.y += delta.y;
        }
        if top {
            size.y -= delta.y;
        }
        let size = vec2(
            size.x.max(SMALLEST.x).round(),
            size.y.max(SMALLEST.y).round(),
        );
        let kept = |size: Vec2| {
            pos2(
                match (left, right) {
                    (true, _) => size.x,
                    (false, true) => 0.0,
                    (false, false) => size.x / 2.0,
                },
                if top { size.y } else { 0.0 },
            )
        };
        let mut panel = Panel { size, ..self.panel };
        panel.anchor(kept(size), self.panel.frame_at(kept(was)));
        (panel.pose, size)
    }
}

fn sides(edge: WindowEdge) -> (bool, bool, bool, bool) {
    use WindowEdge::*;
    (
        matches!(edge, Left | TopLeft | BottomLeft),
        matches!(edge, Right | TopRight | BottomRight),
        matches!(edge, Top | TopLeft | TopRight),
        matches!(edge, Bottom | BottomLeft | BottomRight),
    )
}
