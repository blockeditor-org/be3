use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;

mod gui;
mod raw;
mod ui;

use ui::InputRemapsView;

const INTRINSIC_WIDTH: f32 = 520.0;
const INTRINSIC_HEIGHT: f32 = 520.0;

pub const PLUGIN: &str = "be3.settings";
pub const MODULE: &str = "remapper.wasm";

pub struct InputRemapsApp;

#[component]
impl block_editor_beui::BeuiApp for InputRemapsApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <InputRemapsView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}

#[cfg(test)]
mod tests;
