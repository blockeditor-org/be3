pub use crate::types::{FileFilter, FilePick, FileSave, PickedFile, SavedFile};

impl FileFilter {
    pub fn new(name: &str, extensions: &[&str], mime_types: &[&str]) -> Self {
        let owned = |values: &[&str]| values.iter().map(|value| (*value).to_owned()).collect();
        Self {
            name: name.to_owned(),
            extensions: owned(extensions),
            mime_types: owned(mime_types),
        }
    }
}
