use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_scene_3d::Scene3dContent;
use uuid::Uuid;

mod ui;

use ui::Scene3DView;

pub struct Scene3DRoot;

#[component]
impl be_sdk_beui::BeuiRoot for Scene3DRoot {
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
