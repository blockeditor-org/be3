use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::view;
use block_editor_beui::beui::{NodeId, Vec2};

mod ui;

use ui::SettingsView;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 120.0;

pub struct SettingsApp;

impl block_editor_beui::BeuiApp for SettingsApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <SettingsView editor={editor} />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
