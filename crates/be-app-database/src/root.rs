use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_sdk_database::create_database;
use uuid::Uuid;

mod ui;

use ui::DatabaseEditor;

pub struct DatabaseRoot;

#[component]
impl be_sdk_beui::BeuiApp for DatabaseRoot {
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
