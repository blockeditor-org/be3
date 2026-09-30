use block_editor_beui::be_block::ImageContent;
use block_editor_beui::{FileFilter, PickedFile};

pub(crate) fn image_filter() -> FileFilter {
    FileFilter::new(
        "Images",
        "Image",
        ImageContent::FILE_EXTENSIONS,
        ImageContent::MIME_TYPES,
    )
}

pub(crate) fn imported_image(file: PickedFile) -> ImageContent {
    let PickedFile { name, data } = file;
    ImageContent::from_file(name, data)
}
