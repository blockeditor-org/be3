use block_client::blocks::scene_3d::Scene3D;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::Scene3DView;

pub struct Scene3DApp;

impl block_editor_plugin::BeuiApp for Scene3DApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Scene3DView editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(Scene3D::new()).id())
    }
}
