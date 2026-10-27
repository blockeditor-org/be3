use std::rc::Rc;

use crate::reactive::with_document;
pub use beui_core::drag_board::{Board, Pending, Target};

pub fn drag_board() -> Rc<Board> {
    with_document(|document| document.drag_board())
}
