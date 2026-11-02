use be_sdk_beui::beui::Vec2;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_example_pan_zoom::PanZoomContent;

mod ui;

use ui::PanZoom;

pub struct PanZoomRoot;

#[component]
impl be_sdk_beui::BeuiApp for PanZoomRoot {
    #[component]
    fn View(editor: Editor) -> Root {
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
