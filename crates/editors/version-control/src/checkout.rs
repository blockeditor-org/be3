use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;

mod ui;

use ui::CheckoutView;

pub struct CheckoutApp;

impl block_editor_beui::BeuiApp for CheckoutApp {
    fn view(editor: Editor) -> NodeId {
        let content = editor.loaded::<block_editor_beui::be_block::CheckoutContent>(&[]);
        let status = editor.version_known();
        let loaded = create_memo(move || content.get() && status.get());
        view! {
            <Loaded loaded={loaded}>
                <CheckoutView editor={editor} />
            </Loaded>
        }
    }
}
