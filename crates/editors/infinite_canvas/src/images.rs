use block_client::blocks::image::Image;
use block_editor_plugin::{FileFilter, PickedFile};

pub(crate) fn image_filter() -> FileFilter {
    FileFilter::new("Images", "Image", Image::FILE_EXTENSIONS, Image::MIME_TYPES)
}

pub(crate) fn imported_image(file: PickedFile) -> Image {
    let PickedFile { name, data } = file;
    Image::new(name, data)
}
