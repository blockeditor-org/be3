use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Editor, Loaded};

mod export;
mod hits;
mod kinds;
mod menu;
mod rename;
mod rows;
mod toolbar;
mod ui;
mod zoom;

use ui::FileTreeEditor;

pub struct FileTreeRoot;

#[component]
impl be_sdk_beui::BeuiRoot for FileTreeRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded label="Loading the files">
                    <FileTreeEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }
}
