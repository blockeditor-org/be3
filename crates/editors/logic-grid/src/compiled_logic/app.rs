use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;

mod ui;

use ui::CompiledLogicView;

pub struct CompiledLogicApp;

impl block_editor_beui::BeuiApp for CompiledLogicApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <CompiledLogicView editor={editor} />
            </Loaded>
        }
    }
}
