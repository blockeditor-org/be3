use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;

mod rows;
mod ui;

use ui::FileTreeEditor;

pub struct FileTreeApp;

impl block_editor_plugin::BeuiApp for FileTreeApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <FileTreeEditor editor={editor} />
        }
    }
}
