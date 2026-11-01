use std::rc::Rc;

use beui_core::current::with_document;
use beui_types::drag::{Driver, Space};

pub(crate) fn space() -> Rc<dyn Space> {
    with_document(|document| Rc::clone(&document.rects) as Rc<dyn Space>)
}

pub(crate) fn drive(driver: Rc<dyn Driver>) {
    with_document(|document| document.drags = Some(driver));
}
