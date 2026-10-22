use block_editor_beui::Loaded;
use block_editor_beui::be_block::ImageContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};

mod picture;
mod ui;

use ui::{ImageEditor, ImagePreview};

pub struct ImageApp;

impl block_editor_beui::BeuiApp for ImageApp {
    fn view(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <ImageEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <ImagePreview editor={editor} />
            </Loaded>
        }
    }

    fn creation_view(creation: Creation) -> NodeId {
        view! {
            <ContentFileCreation creation id_prefix="image" filter={filter()} import={imported} />
        }
    }
}

pub(crate) fn filter() -> FileFilter {
    FileFilter::new(
        "Images",
        ImageContent::FILE_EXTENSIONS,
        ImageContent::MIME_TYPES,
    )
}

pub(crate) fn imported(file: PickedFile) -> Result<ImageContent, String> {
    let PickedFile { name, data } = file;
    crate::decode::decode(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(ImageContent::from_file(name, data))
}
