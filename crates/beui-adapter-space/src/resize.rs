use beui_core::geometry::{Pos2, Vec2, pos2, vec2};
use beui_core::window::{WindowEdge, WindowId};

use crate::panel::{Panel, Pose};

pub const SMALLEST: Vec2 = vec2(120.0, 80.0);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Resize {
    pub window: WindowId,
    pub edge: WindowEdge,
    pose: Pose,
    size: Vec2,
    grab: Pos2,
}

impl Resize {
    pub fn begin(panel: &Panel, edge: WindowEdge, grab: Pos2) -> Self {
        Self {
            window: panel.window,
            edge,
            pose: panel.pose,
            size: panel.size,
            grab,
        }
    }

    pub fn original(&self) -> Panel {
        Panel {
            window: self.window,
            pose: self.pose,
            size: self.size,
        }
    }

    pub fn to(&self, pos: Pos2) -> (Pose, Vec2) {
        let delta = pos - self.grab;
        let (left, right, top, bottom) = sides(self.edge);
        let mut size = self.size;
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
        let corner = pos2(
            if left { self.size.x - size.x } else { 0.0 },
            if top { self.size.y - size.y } else { 0.0 },
        );
        let centre = corner + size / 2.0;
        let pose = Pose {
            position: self.original().world(centre),
            rotation: self.pose.rotation,
        };
        (pose, size)
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
