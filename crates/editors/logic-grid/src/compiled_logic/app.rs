use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;

mod ui;

use ui::CompiledLogicView;

pub struct CompiledLogicApp;

impl block_editor_beui::BeuiApp for CompiledLogicApp {
    fn view(editor: Editor) -> NodeId {
        let loaded = editor.loaded::<block_editor_beui::be_block::CompiledLogicContent>(&[]);
        view! {
            <Loaded loaded={loaded}>
                <CompiledLogicView editor={editor} />
            </Loaded>
        }
    }
}
