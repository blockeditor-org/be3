use block_editor_beui::{FileFilter, PickedFile};
use image_block_types::{ImageContent, ImageFile};

pub(crate) fn image_filter() -> FileFilter {
    FileFilter::new("Images", ImageFile::FILE_EXTENSIONS, ImageFile::MIME_TYPES)
}

pub(crate) fn imported_image(file: PickedFile) -> ImageContent {
    let PickedFile { name, data } = file;
    ImageFile::from_file(name, data)
}
