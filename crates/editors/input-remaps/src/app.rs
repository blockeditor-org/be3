use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::Vec2;
use block_editor_beui::beui::reactive::*;

mod gui;
mod raw;
mod ui;

use ui::InputRemapsView;

const INTRINSIC_WIDTH: f32 = 520.0;
const INTRINSIC_HEIGHT: f32 = 520.0;

pub const PLUGIN: &str = "be3.input-remaps";
pub const MODULE: &str = "remapper.wasm";

pub struct InputRemapsApp;

impl block_editor_beui::BeuiApp for InputRemapsApp {
    fn view(editor: Editor) -> NodeId {
        let loaded = editor.loaded::<block_editor_beui::be_block::InputRemapperContent>(&[]);
        view! {
            <Loaded loaded={loaded}>
                <InputRemapsView editor={editor} />
            </Loaded>
        }
    }

    fn intrinsic_size() -> Option<Vec2> {
        Some(Vec2::new(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }
}
