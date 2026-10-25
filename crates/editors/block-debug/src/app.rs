use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Editor, Loaded};

mod entries;
mod ui;

use ui::BlockDebug;

pub struct BlockDebugApp;

#[component]
impl block_editor_beui::BeuiApp for BlockDebugApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <BlockDebug editor={editor} />
                </Loaded>
            </Root>
        }
    }
}
