pub use crate::types::files::{FileSave, SavedFile};

pub fn save_file(file: SavedFile, saved: impl FnOnce(FileSave) + 'static) {
    crate::core::files::save_file(file, saved);
}
