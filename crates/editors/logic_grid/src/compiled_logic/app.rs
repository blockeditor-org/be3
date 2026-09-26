use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;

mod ui;

use ui::CompiledLogicView;

pub struct CompiledLogicApp;

impl block_editor_plugin::BeuiApp for CompiledLogicApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <CompiledLogicView editor={editor} />
        }
    }
}
