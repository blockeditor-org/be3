use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Editor, Loaded};

mod entries;
mod ui;

use ui::BlockDebug;

pub struct BlockDebugRoot;

#[component]
impl be_sdk_beui::BeuiRoot for BlockDebugRoot {
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
