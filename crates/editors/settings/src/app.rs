use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Editor, Loaded};

mod devices;
mod setup;
mod ui;

use ui::{SettingsProfileView, SettingsView};

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 240.0;

pub struct SettingsApp;

#[component]
impl block_editor_beui::BeuiApp for SettingsApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <SettingsView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

pub struct SettingsProfileApp;

#[component]
impl block_editor_beui::BeuiApp for SettingsProfileApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <SettingsProfileView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
