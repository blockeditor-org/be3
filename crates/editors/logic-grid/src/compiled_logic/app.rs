use block_editor_beui::Editor;
use block_editor_beui::Loaded;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::Root;

mod ui;

use ui::CompiledLogicView;

pub struct CompiledLogicApp;

impl block_editor_beui::BeuiApp for CompiledLogicApp {
    fn view(editor: Editor) -> Root {
        view! {
            <Root>
                <Loaded>
                    <CompiledLogicView editor={editor} />
                </Loaded>
            </Root>
        }
    }
}
