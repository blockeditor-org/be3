use crate::reactive::with_document;
use crate::types::files::{FileFilter, FilePick, FileSave, SavedFile};

pub fn pick_file(filter: FileFilter, picked: impl FnOnce(FilePick) + 'static) {
    with_document(|document| document.pick_file(filter, picked));
}

pub fn save_file(file: SavedFile, saved: impl FnOnce(FileSave) + 'static) {
    with_document(|document| document.save_file(file, saved));
}
