use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use block_editor_database::create_database;
use uuid::Uuid;

mod ui;

use ui::DatabaseEditor;

pub struct DatabaseApp;

#[component]
impl block_editor_beui::BeuiApp for DatabaseApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <DatabaseEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(create_database(creation))
    }
}
