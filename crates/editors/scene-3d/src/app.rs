use block_editor_beui::be_block::Scene3dContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::Scene3DView;

pub struct Scene3DApp;

impl block_editor_beui::BeuiApp for Scene3DApp {
    fn view(_editor: Editor) -> Root {
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
