use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{create_memo, view};
use block_editor_beui::{BlockQuery, Editor, Loaded};

mod export;
mod phone;
mod rows;
mod ui;

use ui::FileTreeEditor;

pub struct FileTreeApp;

impl block_editor_beui::BeuiApp for FileTreeApp {
    fn view(editor: Editor) -> NodeId {
        let roots = editor.watch_blocks(BlockQuery::Roots);
        let loaded = create_memo(move || roots.with(Option::is_some));
        view! {
            <Loaded loaded={loaded} label="Loading the files">
                <FileTreeEditor editor={editor} />
            </Loaded>
        }
    }
}
