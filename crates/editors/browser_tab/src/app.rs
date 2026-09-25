use block_editor_beui::be_block::BrowserTabContent;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::beui::{NodeId, Vec2};
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod session;
mod ui;

const INTRINSIC_WIDTH: f32 = 1024.0;
const INTRINSIC_HEIGHT: f32 = 768.0;

use ui::BrowserTab;

pub struct BrowserTabApp;

impl block_editor_beui::BeuiApp for BrowserTabApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <BrowserTab editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&BrowserTabContent::default()))
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
