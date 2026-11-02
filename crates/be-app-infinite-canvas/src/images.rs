use be_sdk_beui::{FileFilter, PickedFile};
use be_type_image::{ImageContent, ImageFile};

pub(crate) fn image_filter() -> FileFilter {
    FileFilter::new("Images", ImageFile::FILE_EXTENSIONS, ImageFile::MIME_TYPES)
}

pub(crate) fn imported_image(file: PickedFile) -> ImageContent {
    let PickedFile { name, data } = file;
    ImageFile::from_file(name, data)
}
