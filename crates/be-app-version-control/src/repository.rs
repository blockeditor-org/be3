use be_sdk_beui::Loaded;
use be_sdk_beui::be_block::RepositoryContent;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::RepositoryView;

pub struct RepositoryRoot;

#[component]
impl be_sdk_beui::BeuiApp for RepositoryRoot {
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
