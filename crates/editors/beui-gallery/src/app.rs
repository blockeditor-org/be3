use beui_demo::Demo;
use beui_gallery_types::BeuiDemoContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};

pub struct BeuiGalleryApp;

#[component]
impl block_editor_beui::BeuiApp for BeuiGalleryApp {
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
