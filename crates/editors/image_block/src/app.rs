use block_client::blocks::image::Image as ImageBlock;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor, FileFilter, PickedFile, file_creation};

mod picture;
mod ui;

use ui::{ImageEditor, ImagePreview};

pub struct ImageApp;

impl block_editor_plugin::BeuiApp for ImageApp {
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
        file_creation(&creation, "image", filter(), imported)
    }
}

pub(crate) fn filter() -> FileFilter {
    FileFilter::new(
        "Images",
        "Image",
        ImageBlock::FILE_EXTENSIONS,
        ImageBlock::MIME_TYPES,
    )
}

pub(crate) fn imported(file: PickedFile) -> Result<ImageBlock, String> {
    let PickedFile { name, data } = file;
    crate::decode::decode(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(ImageBlock::new(name, data))
}
