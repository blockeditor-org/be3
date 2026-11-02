use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_example_beui_gallery::BeuiDemoContent;
use beui_demo::Demo;

pub struct BeuiGalleryRoot;

#[component]
impl be_sdk_beui::BeuiApp for BeuiGalleryRoot {
    #[component]
    fn View(_editor: Editor) -> Root {
        view! {
            <Root>
                <Demo />
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<uuid::Uuid, String> {
        Ok(creation.create(&BeuiDemoContent::default()))
    }
}
