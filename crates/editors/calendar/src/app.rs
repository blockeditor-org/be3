use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use calendar_types::CalendarContent;
use uuid::Uuid;

mod model;
mod month;
mod timeline;
mod ui;

use ui::CalendarEditor;

pub struct CalendarApp;

#[component]
impl block_editor_beui::BeuiApp for CalendarApp {
    #[component]
    fn View(editor: Editor) -> Root {
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
