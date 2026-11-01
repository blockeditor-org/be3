use std::rc::Rc;

use beui_types::drag::{Driver, Space};
use beui_types::{NodeId, Rect};

struct Measured;

impl Space for Measured {
    fn placed(&self, node: NodeId) -> Option<(Rect, f32)> {
        Some((beui_core_html::measure::rect(node)?, 1.0))
    }

    fn visible(&self, node: NodeId) -> Option<Rect> {
        beui_core_html::measure::rect(node)
    }
}

pub(crate) fn space() -> Rc<dyn Space> {
    Rc::new(Measured)
}

pub(crate) fn drive(_: Rc<dyn Driver>) {}
