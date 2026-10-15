use block_editor_beui::be_block::ImageContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{BlockQuery, Loaded};
use block_editor_beui::{ContentFileCreation, Creation, Editor, FileFilter, PickedFile};

mod picture;
mod ui;

use ui::{ImageEditor, ImagePreview};

pub struct ImageApp;

impl block_editor_beui::BeuiApp for ImageApp {
    fn view(editor: Editor) -> NodeId {
        let loaded = placeholder_or_image(editor);
        view! {
            <Loaded loaded={loaded}>
                <ImageEditor editor={editor} />
            </Loaded>
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        let loaded = placeholder_or_image(editor);
        view! {
            <Loaded loaded={loaded}>
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

fn placeholder_or_image(editor: Editor) -> Memo<bool> {
    let image = editor.block_content::<ImageContent>().loaded();
    let info = editor.watch_blocks(BlockQuery::Block(editor.block_id()));
    create_memo(move || image.get() || info.with(Option::is_some))
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
