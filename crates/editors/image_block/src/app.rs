use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};

mod chooser;
mod picture;
mod ui;

use ui::{ImageCreation, ImageEditor, ImagePreview};

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
        view! {
            <ImageCreation creation={creation} />
        }
    }
}
