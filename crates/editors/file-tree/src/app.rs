use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Editor, Loaded};

mod export;
mod kinds;
mod menu;
mod rename;
mod rows;
mod ui;
mod zoom;

use ui::FileTreeEditor;

pub struct FileTreeApp;

#[component]
impl block_editor_beui::BeuiApp for FileTreeApp {
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
