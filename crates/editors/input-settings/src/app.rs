use block_editor_beui::Editor;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::view;

mod ui;

use ui::InputSettingsView;

const INTRINSIC_WIDTH: f32 = 420.0;
const INTRINSIC_HEIGHT: f32 = 520.0;

pub struct InputSettingsApp;

impl block_editor_beui::BeuiApp for InputSettingsApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <InputSettingsView editor={editor} />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
