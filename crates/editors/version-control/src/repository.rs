use block_editor_beui::Loaded;
use block_editor_beui::be_block::RepositoryContent;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::RepositoryView;

pub struct RepositoryApp;

impl block_editor_beui::BeuiApp for RepositoryApp {
    fn view(editor: Editor) -> NodeId {
        let content = editor.loaded::<block_editor_beui::be_block::RepositoryContent>(&[]);
        let status = editor.version_known();
        let loaded = create_memo(move || content.get() && status.get());
        view! {
            <Loaded loaded={loaded}>
                <RepositoryView editor={editor} />
            </Loaded>
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&RepositoryContent::default()))
    }
}
