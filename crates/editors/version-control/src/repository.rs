use block_editor_beui::Loaded;
use block_editor_beui::be_block::RepositoryContent;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::RepositoryView;

pub struct RepositoryApp;

#[component]
impl block_editor_beui::BeuiApp for RepositoryApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <RepositoryView editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&RepositoryContent::default()))
    }
}
