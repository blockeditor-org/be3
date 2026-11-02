use be_sdk_beui::Loaded;
use be_sdk_beui::beui::Vec2;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_browser_tab::BrowserTabContent;
use uuid::Uuid;

mod session;
mod ui;

const INTRINSIC_WIDTH: f32 = 1024.0;
const INTRINSIC_HEIGHT: f32 = 768.0;

use ui::BrowserTab;

pub struct BrowserTabRoot;

#[component]
impl be_sdk_beui::BeuiApp for BrowserTabRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <BrowserTab editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&BrowserTabContent::default()))
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
