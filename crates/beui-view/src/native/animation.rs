use std::cell::Cell;
use std::rc::Rc;

use crate::reactive::{Get, Memo, create_memo};
use crate::types::Motion;
use beui_core::current::with_document;

pub(crate) fn running() -> Rc<Cell<usize>> {
    with_document(|document| document.animations())
}

pub(crate) fn motion() -> Motion {
    with_document(|document| document.motion())
}

pub(crate) fn watch_motion() -> Memo<Motion> {
    let motion = with_document(|document| document.watch_motion());
    create_memo(move || motion.get())
}
