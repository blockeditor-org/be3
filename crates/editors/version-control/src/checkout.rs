use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::*;

mod ui;

use ui::CheckoutView;

pub struct CheckoutApp;

impl block_editor_beui::BeuiApp for CheckoutApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Loaded>
                <CheckoutView editor={editor} />
            </Loaded>
        }
    }
}
