use block_client::blocks::web_browser_tab::WebBrowserTab;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::beui::{NodeId, Vec2};
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod session;
mod ui;

const INTRINSIC_WIDTH: f32 = 1024.0;
const INTRINSIC_HEIGHT: f32 = 768.0;

use ui::BrowserTab;

pub struct BrowserTabApp;

impl block_editor_plugin::BeuiApp for BrowserTabApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <BrowserTab editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.client().create_block(WebBrowserTab::new()).id())
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
