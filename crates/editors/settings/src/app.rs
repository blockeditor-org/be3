use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{NodeId, Vec2};

mod devices;
mod setup;
mod ui;

use ui::{SettingsProfileView, SettingsView};

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 240.0;

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

pub struct SettingsProfileApp;

impl block_editor_beui::BeuiApp for SettingsProfileApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <SettingsProfileView editor={editor} />
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
