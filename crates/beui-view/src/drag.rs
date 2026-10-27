use std::rc::Rc;

use beui_core::current::with_document;
pub use beui_core::drag_board::{Board, DragPoint, Pending, Target};

pub fn drag_board() -> Rc<Board> {
    with_document(|document| document.drag_board())
}
