use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use presentation_types::PresentationContent;
use uuid::Uuid;

mod slides;
pub mod templates;
mod ui;

use ui::{PresentationPreview, PresentationView};

pub struct PresentationApp;

#[component]
impl block_editor_beui::BeuiApp for PresentationApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <PresentationView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    #[component]
    fn Preview(editor: Editor) -> NodeId {
        view! {
            <Loaded>
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
