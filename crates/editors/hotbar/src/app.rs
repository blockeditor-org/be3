use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;

mod ui;

use ui::HotbarView;

pub struct HotbarApp;

impl block_editor_plugin::BeuiApp for HotbarApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <HotbarView editor={editor} />
        }
    }
}

#[cfg(test)]
mod tests;
