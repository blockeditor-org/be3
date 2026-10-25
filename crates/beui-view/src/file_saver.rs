pub use beui_core::file_saver::{FileSave, SavedFile};

use crate::reactive::with_document;

pub fn save_file(file: SavedFile, saved: impl FnOnce(FileSave) + 'static) {
    with_document(|document| document.save_file(file, saved));
}
