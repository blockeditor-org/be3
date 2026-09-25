use block_editor_plugin::be_block::Scene3dContent;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::Scene3DView;

pub struct Scene3DApp;

impl block_editor_plugin::BeuiApp for Scene3DApp {
    fn view(_editor: Editor) -> NodeId {
        view! {
            <Scene3DView />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&Scene3dContent::default()))
    }
}
