use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;

mod ui;

use ui::HotbarView;

pub struct HotbarApp;

impl block_editor_beui::BeuiApp for HotbarApp {
    fn view(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <HotbarView editor={editor} />
                </Loaded>
            </Root>
        }
    }
}

#[cfg(test)]
mod tests;
