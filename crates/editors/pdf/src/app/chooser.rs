use std::rc::Rc;

use block_client::blocks::pdf::Pdf;
use block_editor_plugin::{FileChooser, FileFilter, PickedFile};

pub(crate) fn chooser() -> Rc<FileChooser<Pdf>> {
    FileChooser::new(filter(), imported)
}

fn filter() -> FileFilter {
    FileFilter {
        name: "PDF".to_owned(),
        default_file_name: "Document.pdf".to_owned(),
        extensions: vec!["pdf".to_owned()],
        mime_types: vec!["application/pdf".to_owned()],
    }
}

fn imported(file: PickedFile) -> Result<Pdf, String> {
    let PickedFile { name, data } = file;
    Pdf::new(name.clone(), data).map_err(|error| format!("Could not import {name}: {error}"))
}
