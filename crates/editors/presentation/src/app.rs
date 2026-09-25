use block_editor_beui::be_block::PresentationContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod slides;
mod ui;

use ui::{PresentationPreview, PresentationView};

pub struct PresentationApp;

impl block_editor_beui::BeuiApp for PresentationApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <PresentationView editor={editor} />
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        view! {
            <PresentationPreview editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&PresentationContent::default()))
    }
}
