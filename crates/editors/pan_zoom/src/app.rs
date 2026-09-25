use block_editor_plugin::be_block::PanZoomContent;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{Creation, Editor};

mod ui;

use ui::PanZoom;

pub struct PanZoomApp;

impl block_editor_plugin::BeuiApp for PanZoomApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <PanZoom editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<uuid::Uuid, String> {
        Ok(creation.create(&PanZoomContent::default()))
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(ui::WORLD)
    }
}
