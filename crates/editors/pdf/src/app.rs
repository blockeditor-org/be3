use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};

mod chooser;
mod pages;
mod ui;

use ui::{PdfCreation, PdfEditor, PdfPreview};

pub struct PdfApp;

impl block_editor_plugin::BeuiApp for PdfApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <PdfEditor editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <PdfPreview editor={editor} />
        }
    }

    fn creation_view(creation: Creation) -> NodeId {
        view! {
            <PdfCreation creation={creation} />
        }
    }
}

pub(crate) const DEFAULT_PAGE_SIZE: block_editor_plugin::beui::Vec2 =
    block_editor_plugin::beui::Vec2::new(612.0, 792.0);
