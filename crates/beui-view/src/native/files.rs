use crate::types::files::{FileFilter, FilePick, FileSave, SavedFile};
use beui_core::current::with_document;

pub(crate) fn pick_file(filter: FileFilter, picked: impl FnOnce(FilePick) + 'static) {
    with_document(|document| document.pick_file(filter, picked));
}

pub(crate) fn save_file(file: SavedFile, saved: impl FnOnce(FileSave) + 'static) {
    with_document(|document| document.save_file(file, saved));
}
