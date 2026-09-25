use block_editor_beui::be_block::Scene3dContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::Scene3DView;

pub struct Scene3DApp;

impl block_editor_beui::BeuiApp for Scene3DApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Scene3DView editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&Scene3dContent::default()))
    }
}
