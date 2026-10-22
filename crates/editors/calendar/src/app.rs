use block_editor_beui::Loaded;
use block_editor_beui::be_block::CalendarContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod model;
mod month;
mod timeline;
mod ui;

use ui::CalendarEditor;

pub struct CalendarApp;

impl block_editor_beui::BeuiApp for CalendarApp {
    fn view(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <CalendarEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&CalendarContent::default()))
    }
}
