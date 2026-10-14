use block_editor_beui::be_block::PresentationContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{BlockQuery, Loaded};
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod slides;
pub mod templates;
mod ui;

use ui::{PresentationPreview, PresentationView};

pub struct PresentationApp;

impl block_editor_beui::BeuiApp for PresentationApp {
    fn view(editor: Editor) -> NodeId {
        let loaded = editor.loaded::<block_editor_beui::be_block::PresentationContent>(&[
            BlockQuery::References(editor.block_id()),
        ]);
        view! {
            <Loaded loaded={loaded}>
                <PresentationView editor={editor} />
            </Loaded>
        }
    }

    fn preview_view(editor: Editor) -> NodeId {
        let loaded = editor.loaded::<block_editor_beui::be_block::PresentationContent>(&[
            BlockQuery::References(editor.block_id()),
        ]);
        view! {
            <Loaded loaded={loaded}>
                <PresentationPreview editor={editor} />
            </Loaded>
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
