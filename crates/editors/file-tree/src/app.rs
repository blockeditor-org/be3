use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{Editor, Loaded};

mod export;
mod menu;
mod rename;
mod rows;
mod ui;
mod zoom;

use ui::FileTreeEditor;

pub struct FileTreeApp;

impl block_editor_beui::BeuiApp for FileTreeApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Loaded label="Loading the files">
                <FileTreeEditor editor={editor} />
            </Loaded>
        }
    }
}
