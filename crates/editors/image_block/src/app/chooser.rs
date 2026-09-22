use std::rc::Rc;

use block_client::blocks::image::Image as ImageBlock;
use block_editor_plugin::{FileChooser, FileFilter, PickedFile};

pub(crate) fn chooser() -> Rc<FileChooser<ImageBlock>> {
    FileChooser::new(filter(), imported)
}

fn filter() -> FileFilter {
    FileFilter {
        name: "Images".to_owned(),
        default_file_name: "Image".to_owned(),
        extensions: ImageBlock::FILE_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
        mime_types: ImageBlock::MIME_TYPES
            .iter()
            .map(|mime_type| (*mime_type).to_owned())
            .collect(),
    }
}

fn imported(file: PickedFile) -> Result<ImageBlock, String> {
    let PickedFile { name, data } = file;
    crate::decode::decode(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(ImageBlock::new(name, data))
}
