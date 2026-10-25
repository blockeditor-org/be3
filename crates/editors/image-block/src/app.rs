use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};
use image_block_types::{ImageContent, ImageFile};

mod picture;
mod ui;

use ui::{ImageEditor, ImagePreview};

pub struct ImageApp;

#[component]
impl block_editor_beui::BeuiApp for ImageApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <ImageEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Preview(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <ImagePreview editor={editor} />
            </Loaded>
        }
    }

    #[component]
    fn Creation(creation: Creation) -> NodeId {
        view! {
            <ContentFileCreation creation id_prefix="image" filter={filter()} import={imported} />
        }
    }
}

pub(crate) fn filter() -> FileFilter {
    FileFilter::new("Images", ImageFile::FILE_EXTENSIONS, ImageFile::MIME_TYPES)
}

pub(crate) fn imported(file: PickedFile) -> Result<ImageContent, String> {
    let PickedFile { name, data } = file;
    crate::decode::decode(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(ImageFile::from_file(name, data))
}
