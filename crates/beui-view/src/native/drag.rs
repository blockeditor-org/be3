use std::rc::Rc;

use beui_core::current::with_document;
pub(crate) use beui_core::drag_board::Board;

pub(crate) fn drag_board() -> Rc<Board> {
    with_document(|document| document.drag_board())
}
