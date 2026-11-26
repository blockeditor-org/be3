use std::rc::Rc;

use beui_types::drag::{Driver, Space};
use beui_types::{NodeId, Rect, WindowId};

struct Measured;

impl Space for Measured {
    fn placed(&self, node: NodeId) -> Option<(Rect, f32)> {
        Some((beui_core_html::measure::rect(node)?, 1.0))
    }

    fn visible(&self, node: NodeId) -> Option<Rect> {
        beui_core_html::measure::rect(node)
    }

    /// The page is one window.
    fn window(&self, _: NodeId) -> Option<WindowId> {
        Some(WindowId::MAIN)
    }
}

pub(crate) fn space() -> Rc<dyn Space> {
    Rc::new(Measured)
}

pub(crate) fn driver() -> Option<Rc<dyn Driver>> {
    beui_core_html::drags::driver()
}

pub(crate) fn drive(driver: Rc<dyn Driver>) {
    beui_core_html::drags::drive(driver);
}
