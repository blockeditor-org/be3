use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;

mod ui;

use ui::CheckoutView;

pub struct CheckoutApp;

impl block_editor_plugin::BeuiApp for CheckoutApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <CheckoutView editor={editor} />
        }
    }
}
