use std::rc::Rc;

use beui_core::current::with_document;
use beui_types::drag::{Driver, Space};

pub(crate) fn space() -> Rc<dyn Space> {
    with_document(|document| Rc::clone(&document.rects) as Rc<dyn Space>)
}

/// None also with no document installed: a node torn down with its document asks too.
pub(crate) fn driver() -> Option<Rc<dyn Driver>> {
    beui_core::current::try_with_document(|document| document.drags.clone()).flatten()
}

pub(crate) fn drive(driver: Rc<dyn Driver>) {
    with_document(|document| document.drags = Some(driver));
}
