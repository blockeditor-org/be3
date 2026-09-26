use block_editor_beui::be_block::PresentationContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod slides;
pub mod templates;
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
        if creation.template() == Creation::MAIN {
            return Ok(creation.create(&PresentationContent::default()));
        }
        let template = templates::SlideTemplate::from_id(creation.template())
            .ok_or_else(|| format!("there is no slide template {}", creation.template()))?;
        Ok(creation.create(&templates::build_template_canvas(template)))
    }
}
