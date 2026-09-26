use block_editor_beui::Editor;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::view;

mod ui;

use ui::HotbarView;

pub struct HotbarApp;

impl block_editor_beui::BeuiApp for HotbarApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <HotbarView editor={editor} />
        }
    }
}

#[cfg(test)]
mod tests;
