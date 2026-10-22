use block_editor_beui::be_block::PanZoomContent;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};

mod ui;

use ui::PanZoom;

pub struct PanZoomApp;

impl block_editor_beui::BeuiApp for PanZoomApp {
    fn view(editor: Editor) -> Root {
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
