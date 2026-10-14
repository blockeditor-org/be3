use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{create_memo, view};
use block_editor_beui::{BlockQuery, Editor, Loaded};

mod entries;
mod ui;

use ui::BlockDebug;

pub struct BlockDebugApp;

impl block_editor_beui::BeuiApp for BlockDebugApp {
    fn view(editor: Editor) -> NodeId {
        let raw = editor.raw_block_content();
        let listed = editor.watch_blocks(BlockQuery::Block(editor.block_id()));
        let catalog = editor.block_types_known();
        let loaded = create_memo(move || {
            raw.with(Option::is_some) && listed.with(Option::is_some) && catalog.get()
        });
        view! {
            <Loaded loaded={loaded}>
                <BlockDebug editor={editor} />
            </Loaded>
        }
    }
}
