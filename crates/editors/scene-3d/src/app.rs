use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use scene_3d_types::Scene3dContent;
use uuid::Uuid;

mod ui;

use ui::Scene3DView;

pub struct Scene3DApp;

#[component]
impl block_editor_beui::BeuiApp for Scene3DApp {
    #[component]
    fn View(_editor: Editor) -> Root {
        view! {
            <Root>
                <Scene3DView />
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&Scene3dContent::default()))
    }
}
