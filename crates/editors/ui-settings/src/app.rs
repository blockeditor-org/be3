use block_editor_beui::Editor;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::view;

mod editors;
mod ui;

use editors::DefaultEditorsView;
use ui::UiSettingsView;

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 120.0;
const EDITORS_HEIGHT: f32 = 360.0;

pub struct UiSettingsApp;

impl block_editor_beui::BeuiApp for UiSettingsApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <UiSettingsView editor={editor} />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

pub struct DefaultEditorsApp;

impl block_editor_beui::BeuiApp for DefaultEditorsApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <DefaultEditorsView editor={editor} />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, EDITORS_HEIGHT))
    }
}
