use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use pan_zoom_types::PanZoomContent;

mod ui;

use ui::PanZoom;

pub struct PanZoomApp;

#[component]
impl block_editor_beui::BeuiApp for PanZoomApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <PanZoom editor={editor} />
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<uuid::Uuid, String> {
        Ok(creation.create(&PanZoomContent::default()))
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(ui::WORLD)
    }
}
