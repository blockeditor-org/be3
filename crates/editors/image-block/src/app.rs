use block_editor_beui::be_block::ImageContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{Creation, Editor, FileFilter, PickedFile, content_file_creation};

mod picture;
mod ui;

use ui::{ImageEditor, ImagePreview};

pub struct ImageApp;

impl block_editor_beui::BeuiApp for ImageApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <ImageEditor editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <ImagePreview editor={editor} />
        }
    }

    fn creation_view(creation: Creation) -> NodeId {
        content_file_creation::<ImageContent>(&creation, "image", filter(), imported)
    }
}

pub(crate) fn filter() -> FileFilter {
    FileFilter::new(
        "Images",
        "Image",
        ImageContent::FILE_EXTENSIONS,
        ImageContent::MIME_TYPES,
    )
}

pub(crate) fn imported(file: PickedFile) -> Result<ImageContent, String> {
    let PickedFile { name, data } = file;
    crate::decode::decode(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(ImageContent::from_file(name, data))
}
