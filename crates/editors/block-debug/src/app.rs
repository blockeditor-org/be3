use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{Editor, Loaded};

mod entries;
mod ui;

use ui::BlockDebug;

pub struct BlockDebugApp;

impl block_editor_beui::BeuiApp for BlockDebugApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <BlockDebug editor={editor} />
            </Loaded>
        }
    }
}
