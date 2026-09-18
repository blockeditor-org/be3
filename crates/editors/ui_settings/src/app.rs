use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::Vec2;
use block_editor_plugin::beui::reactive::view;

mod ui;

use ui::UiSettingsView;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 120.0;

pub struct UiSettingsApp;

impl block_editor_plugin::BeuiApp for UiSettingsApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <UiSettingsView editor={editor} />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
