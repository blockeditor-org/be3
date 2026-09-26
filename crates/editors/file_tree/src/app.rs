use block_editor_beui::Editor;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;

mod rows;
mod ui;

use ui::FileTreeEditor;

pub struct FileTreeApp;

impl block_editor_beui::BeuiApp for FileTreeApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <FileTreeEditor editor={editor} />
        }
    }
}
