use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;

mod ui;

use ui::HotbarView;

pub struct HotbarApp;

#[component]
impl block_editor_beui::BeuiApp for HotbarApp {
    #[component]
    fn View(editor: Editor) -> Root {
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
