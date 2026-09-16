use block_client::blocks::presentation::Presentation;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod slides;
mod ui;

use ui::{PresentationPreview, PresentationView};

pub struct PresentationApp;

impl block_editor_plugin::BeuiApp for PresentationApp {
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
        Ok(creation.client().create_block(Presentation::new()).id())
    }
}
