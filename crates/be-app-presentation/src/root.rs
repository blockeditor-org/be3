use be_sdk_beui::Loaded;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_presentation::PresentationContent;
use uuid::Uuid;

mod slides;
pub use slides::TEXT_TYPE;
pub mod templates;
mod ui;

use ui::{PresentationPreview, PresentationView};

pub struct PresentationRoot;

#[component]
impl be_sdk_beui::BeuiRoot for PresentationRoot {
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
