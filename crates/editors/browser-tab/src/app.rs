use block_editor_beui::Loaded;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use browser_tab_types::BrowserTabContent;
use uuid::Uuid;

mod session;
mod ui;

const INTRINSIC_WIDTH: f32 = 1024.0;
const INTRINSIC_HEIGHT: f32 = 768.0;

use ui::BrowserTab;

pub struct BrowserTabApp;

#[component]
impl block_editor_beui::BeuiApp for BrowserTabApp {
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
