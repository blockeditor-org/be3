use std::cell::Cell;
use std::rc::Rc;

use crate::reactive::Memo;
use crate::types::Motion;

pub(crate) fn running() -> Rc<Cell<usize>> {
    beui_core_html::frame::animations()
}

pub(crate) fn motion() -> Motion {
    beui_core_html::page::motion()
}

pub(crate) fn watch_motion() -> Memo<Motion> {
    beui_core_html::page::watch_motion()
}
