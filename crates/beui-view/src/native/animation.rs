use std::cell::Cell;
use std::rc::Rc;

use crate::reactive::{Get, Memo, create_memo, with_document};
use crate::types::Motion;

pub fn running() -> Rc<Cell<usize>> {
    with_document(|document| document.animations())
}

pub fn motion() -> Motion {
    with_document(|document| document.motion())
}

pub fn watch_motion() -> Memo<Motion> {
    let motion = with_document(|document| document.watch_motion());
    create_memo(move || motion.get())
}
