use be_sdk_beui::Loaded;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};
use be_type_image::{ImageContent, ImageFile};

mod picture;
mod ui;

use ui::{ImageEditor, ImagePreview};

pub struct ImageApp;

#[component]
impl be_sdk_beui::BeuiApp for ImageApp {
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
            <ContentFileCreation
                creation
                title="New Image"
                id_prefix="image"
                filter={filter()}
                import={imported}
            />
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
