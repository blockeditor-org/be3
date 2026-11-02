use be_sdk_beui::Loaded;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::Root;
use be_sdk_beui::{Creation, Editor};
use be_type_workspace::FolderContent;
use uuid::Uuid;

mod drop;
mod entries;
mod ui;

use ui::FolderEditor;

pub struct WorkspaceIndexRoot;

#[component]
impl be_sdk_beui::BeuiRoot for WorkspaceIndexRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <FolderEditor editor={editor} />
                </Loaded>
            </Root>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&FolderContent::default()))
    }
}
